//! Voice/sounds: fact detection from session outcomes, selection from voice packs
//! and the priority queue. All pure (no audio) and testable.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::events::CharacterId;
use crate::session::Outcome;
use crate::voicepack::{Conditions, VoicePack, default_priority};

/// A fact (base event) or – after trigger evaluation – a sound event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trigger {
    pub event: String,
    /// multikill/streak/nemesis/repeat_victim/death_streak/headshot_streak: count.
    pub count: u32,
    pub headshot: bool,
    /// Kill from a vehicle without a weapon (attacker_weapon_id 0).
    pub roadkill: bool,
    pub weapon_id: u32,
    pub vehicle_id: u32,
    /// Filled by the caller from the name cache.
    pub weapon_category: Option<String>,
    pub vehicle: Option<String>,
}

impl Trigger {
    pub fn new(event: &str) -> Self {
        Self {
            event: event.into(),
            count: 0,
            headshot: false,
            roadkill: false,
            weapon_id: 0,
            vehicle_id: 0,
            weapon_category: None,
            vehicle: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DetectorConfig {
    pub multikill_window_secs: i64,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            multikill_window_secs: 4,
        }
    }
}

/// Window for a "mutual kill" (seconds).
pub const MUTUAL_WINDOW_SECS: i64 = 3;

/// Derives facts from session outcomes (one instance per session).
#[derive(Debug, Clone, Default)]
pub struct VoiceDetector {
    cfg: DetectorConfig,
    chain: u32,
    last_kill_at: Option<i64>,
    kills: u32,
    hs_streak: u32,
    death_streak: u32,
    deaths_by: HashMap<CharacterId, u32>,
    kills_of: HashMap<CharacterId, u32>,
    last_death: Option<(CharacterId, i64)>,
    last_kill: Option<(CharacterId, i64)>,
}

impl VoiceDetector {
    pub fn new(cfg: DetectorConfig) -> Self {
        Self {
            cfg,
            ..Default::default()
        }
    }

    /// `previous_killer`: `Session::last_killer` before the event.
    pub fn on_outcomes(
        &mut self,
        outcomes: &[Outcome],
        timestamp: i64,
        previous_killer: Option<CharacterId>,
    ) -> Vec<Trigger> {
        let mut out = Vec::new();
        for o in outcomes {
            match o {
                Outcome::Kill {
                    victim,
                    weapon_id,
                    vehicle_id,
                    headshot,
                } => {
                    let within = self
                        .last_kill_at
                        .is_some_and(|t| timestamp - t <= self.cfg.multikill_window_secs);
                    self.chain = if within { self.chain + 1 } else { 1 };
                    self.last_kill_at = Some(timestamp);
                    self.kills += 1;
                    self.death_streak = 0;
                    self.hs_streak = if *headshot { self.hs_streak + 1 } else { 0 };
                    let n = self.kills_of.entry(*victim).or_default();
                    *n += 1;
                    let victim_kills = *n;
                    let mut t = Trigger::new("kill");
                    t.headshot = *headshot;
                    t.weapon_id = *weapon_id;
                    t.vehicle_id = *vehicle_id;
                    t.roadkill = *weapon_id == 0 && *vehicle_id != 0;
                    t.count = victim_kills;
                    out.push(t.clone());
                    let with = |event: &str, count: u32| Trigger {
                        event: event.into(),
                        count,
                        ..t.clone()
                    };
                    if self.kills == 1 {
                        out.push(with("first_kill", 1));
                    }
                    if self.chain >= 2 {
                        out.push(with("multikill", self.chain));
                    }
                    if self.hs_streak >= 2 {
                        out.push(with("headshot_streak", self.hs_streak));
                    }
                    if victim_kills >= 2 {
                        out.push(with("repeat_victim", victim_kills));
                    }
                    if previous_killer == Some(*victim) {
                        out.push(with("revenge", 0));
                    }
                    if self
                        .last_death
                        .is_some_and(|(k, at)| k == *victim && timestamp - at <= MUTUAL_WINDOW_SECS)
                    {
                        out.push(with("mutual", 0));
                    }
                    self.last_kill = Some((*victim, timestamp));
                }
                Outcome::Death {
                    killer,
                    weapon_id,
                    vehicle_id,
                    headshot,
                } => {
                    self.on_death();
                    let n = self.deaths_by.entry(*killer).or_default();
                    *n += 1;
                    let n = *n;
                    let mut t = Trigger::new("death");
                    t.headshot = *headshot;
                    t.weapon_id = *weapon_id;
                    t.vehicle_id = *vehicle_id;
                    out.push(t.clone());
                    if n >= 2 {
                        out.push(Trigger {
                            event: "nemesis".into(),
                            count: n,
                            ..t.clone()
                        });
                    }
                    out.push(Trigger {
                        event: "death_streak".into(),
                        count: self.death_streak,
                        ..t.clone()
                    });
                    if self
                        .last_kill
                        .is_some_and(|(v, at)| v == *killer && timestamp - at <= MUTUAL_WINDOW_SECS)
                    {
                        out.push(Trigger {
                            event: "mutual".into(),
                            ..t
                        });
                    }
                    self.last_death = Some((*killer, timestamp));
                }
                Outcome::Suicide { weapon_id } => {
                    self.on_death();
                    let mut t = Trigger::new("suicide");
                    t.weapon_id = *weapon_id;
                    out.push(t);
                }
                Outcome::Teamkill { weapon_id, .. } => {
                    let mut t = Trigger::new("teamkill");
                    t.weapon_id = *weapon_id;
                    out.push(t);
                }
                Outcome::Teamkilled { weapon_id, .. } => {
                    self.on_death();
                    let mut t = Trigger::new("teamkilled");
                    t.weapon_id = *weapon_id;
                    out.push(t);
                }
                Outcome::Revive { .. } => out.push(Trigger::new("revive")),
                Outcome::Revived { .. } => out.push(Trigger::new("revived")),
                Outcome::VehicleKill {
                    vehicle_id,
                    weapon_id,
                    ..
                } => {
                    let mut t = Trigger::new("vehicle_kill");
                    t.vehicle_id = *vehicle_id;
                    t.weapon_id = *weapon_id;
                    out.push(t);
                }
                Outcome::Streak { count } => {
                    let mut t = Trigger::new("streak");
                    t.count = *count;
                    out.push(t);
                }
                Outcome::StreakEnded { count } => {
                    let mut t = Trigger::new("streak_ended");
                    t.count = *count;
                    out.push(t);
                }
                Outcome::Login => out.push(Trigger::new("login")),
                _ => {}
            }
        }
        out
    }

    fn on_death(&mut self) {
        self.chain = 0;
        self.last_kill_at = None;
        self.hs_streak = 0;
        self.death_streak += 1;
    }
}

pub fn conditions_match(c: &Conditions, t: &Trigger) -> bool {
    if c.min_count.is_some_and(|m| t.count < m) || c.max_count.is_some_and(|m| t.count > m) {
        return false;
    }
    if c.headshot.is_some_and(|h| h != t.headshot) {
        return false;
    }
    let any_eq = |list: &[String], v: &Option<String>| {
        v.as_deref()
            .is_some_and(|v| list.iter().any(|x| x.eq_ignore_ascii_case(v)))
    };
    if !c.weapon_category.is_empty() && !any_eq(&c.weapon_category, &t.weapon_category) {
        return false;
    }
    if !c.vehicle.is_empty() && !any_eq(&c.vehicle, &t.vehicle) {
        return false;
    }
    true
}

/// Result of the selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    pub event: String,
    /// `None`: no sound (muted, or no pack has a file).
    pub file: Option<PathBuf>,
    pub priority: u8,
    pub volume: f32,
    pub pack: String,
    /// Folder name of the pack ("custom" for a custom assignment, empty = none).
    #[serde(default)]
    pub pack_id: String,
    /// Display in the overlay or the OBS source.
    pub overlay: bool,
    pub stream: bool,
}

/// Selects per event: custom assignment first, then the active packs in order.
/// Within a pack the most specific matching entry wins.
/// - Entry with `silent`: no sound, no fallback either.
/// - Entry without files (Recursion "Default"): sound from the next pack, display
///   flags from this entry.
#[derive(Debug, Clone)]
pub struct VoiceSelector {
    rng: u64,
    last_file: HashMap<String, PathBuf>,
    last_played: HashMap<String, u64>,
}

impl Default for VoiceSelector {
    fn default() -> Self {
        Self::new(0x9E37_79B9_7F4A_7C15)
    }
}

impl VoiceSelector {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: seed.max(1),
            last_file: HashMap::new(),
            last_played: HashMap::new(),
        }
    }

    fn next(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }

    /// Random choice without immediate repetition.
    fn pick(&mut self, key: &str, files: &[PathBuf]) -> Option<PathBuf> {
        let last = self.last_file.get(key);
        let candidates: Vec<&PathBuf> = if files.len() > 1 {
            files.iter().filter(|f| Some(*f) != last).collect()
        } else {
            files.iter().collect()
        };
        if candidates.is_empty() {
            return None;
        }
        let i = (self.next() % candidates.len() as u64) as usize;
        let f = candidates[i].clone();
        self.last_file.insert(key.to_owned(), f.clone());
        Some(f)
    }

    pub fn select(
        &mut self,
        trigger: &Trigger,
        overrides: &BTreeMap<String, Vec<PathBuf>>,
        packs: &[VoicePack],
        now_ms: u64,
    ) -> Option<Selection> {
        let base_priority = default_priority(&trigger.event);
        if let Some(files) = overrides.get(&trigger.event).filter(|f| !f.is_empty()) {
            let key = format!("override:{}", trigger.event);
            let file = self.pick(&key, files)?;
            return Some(Selection {
                event: trigger.event.clone(),
                file: Some(file),
                priority: base_priority,
                volume: 1.0,
                pack: "custom assignment".into(),
                pack_id: "custom".into(),
                overlay: true,
                stream: true,
            });
        }
        let mut display: Option<(bool, bool)> = None;
        let key_event = crate::voicepack::is_recursion_style_key(&trigger.event);
        for pack in packs {
            // the built-in standard tones serve only our own base events; pack
            // keys (MUTUAL, FIRST_BLOOD …) come from real packs
            if key_event && pack.id == crate::voicepack::STANDARD_PACK_ID {
                continue;
            }
            let best = pack
                .manifest
                .events
                .iter()
                .enumerate()
                .filter(|(_, e)| e.event == trigger.event && conditions_match(&e.when, trigger))
                .max_by(|(ia, a), (ib, b)| {
                    a.when
                        .specificity()
                        .cmp(&b.when.specificity())
                        .then(ib.cmp(ia))
                });
            let Some((idx, entry)) = best else { continue };
            let (overlay, stream) = *display.get_or_insert((entry.overlay, entry.stream));
            let sel = |file: Option<PathBuf>| Selection {
                event: trigger.event.clone(),
                file,
                priority: entry.priority.unwrap_or(base_priority),
                volume: entry.volume,
                pack: pack.manifest.pack.name.clone(),
                pack_id: pack.id.clone(),
                overlay,
                stream,
            };
            if entry.silent {
                return Some(sel(None));
            }
            if entry.files.is_empty() {
                continue; // "Default": sound from the next pack
            }
            let key = format!("{}#{idx}", pack.id);
            if entry.cooldown_ms > 0
                && self
                    .last_played
                    .get(&key)
                    .is_some_and(|t| now_ms.saturating_sub(*t) < entry.cooldown_ms)
            {
                return None; // cooldown: deliberately no fallback
            }
            let files: Vec<PathBuf> = entry
                .files
                .iter()
                .filter_map(|f| pack.file_path(f))
                .collect();
            let file = self.pick(&key, &files)?;
            self.last_played.insert(key, now_ms);
            return Some(sel(Some(file)));
        }
        display.map(|(overlay, stream)| Selection {
            event: trigger.event.clone(),
            file: None,
            priority: base_priority,
            volume: 1.0,
            pack: String::new(),
            pack_id: String::new(),
            overlay,
            stream,
        })
    }
}

/// Packs used for selection: with at least one real pack active, the built-in
/// standard tones are left out unless `builtin_fallback` is on (an event
/// without a pack sound then stays silent). Without a real pack they stay.
pub fn effective_packs(packs: &[VoicePack], builtin_fallback: bool) -> Vec<VoicePack> {
    let std_id = crate::voicepack::STANDARD_PACK_ID;
    if builtin_fallback || !packs.iter().any(|p| p.id != std_id) {
        return packs.to_vec();
    }
    packs.iter().filter(|p| p.id != std_id).cloned().collect()
}

/// Kill chain style for the active packs ("auto": classic if one has EXCELLENT).
pub fn chain_style(setting: &str, packs: &[VoicePack]) -> crate::triggers::ChainStyle {
    let has_excellent = packs.iter().any(|p| {
        p.id != crate::voicepack::STANDARD_PACK_ID
            && p.manifest
                .events
                .iter()
                .any(|e| e.event == "EXCELLENT" && !e.files.is_empty())
    });
    crate::triggers::ChainStyle::resolve(setting, has_excellent)
}

/// Outcome of [`choose`] for one ESS event.
#[derive(Debug, Clone, Default)]
pub struct Choice {
    /// What to play.
    pub best: Option<Selection>,
    /// What to show (Recursion key with overlay/stream display).
    pub display: Option<Selection>,
    /// Every key that was checked, most important first, with the result:
    /// `DOUBLE_KILL(miss) HEAD_SHOT(no file, X) PER_KILL(hit, Deadpool v2)`.
    pub tried: Vec<String>,
}

/// Generic tier of a hit: the built-in standard tones are only the last resort;
/// a hit in an imported/own pack (or a custom assignment) always beats them,
/// whatever their rank. Within a tier the priority decides, then the order.
fn tier(s: &Selection) -> u8 {
    u8::from(s.pack_id != crate::voicepack::STANDARD_PACK_ID)
}

/// One event system: all sound events triggered by one ESS event (base event
/// and Recursion keys, each with its priority) are checked in priority order
/// across all active packs; the most specific hit wins (see [`tier`]).
pub fn choose(
    selector: &mut VoiceSelector,
    events: &[(Trigger, u8)],
    overrides: &BTreeMap<String, Vec<PathBuf>>,
    packs: &[VoicePack],
    muted: &[String],
    now_ms: u64,
) -> Choice {
    let mut order: Vec<&(Trigger, u8)> = events.iter().collect();
    // most important first; with equal priority the pack key before the base event
    let key = |t: &(Trigger, u8)| (t.1, crate::voicepack::is_recursion_style_key(&t.0.event));
    order.sort_by_key(|t| std::cmp::Reverse(key(t)));
    let mut c = Choice::default();
    for (t, prio) in order {
        if muted.contains(&t.event) {
            c.tried.push(format!("{}(muted)", t.event));
            continue;
        }
        let Some(mut sel) = selector.select(t, overrides, packs, now_ms) else {
            c.tried.push(format!("{}(miss)", t.event));
            continue;
        };
        sel.priority = sel.priority.max(*prio);
        match &sel.file {
            Some(f) => c.tried.push(format!(
                "{}(hit, {}: {})",
                t.event,
                sel.pack,
                f.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            )),
            None if sel.pack.is_empty() => c.tried.push(format!("{}(no file)", t.event)),
            None => c.tried.push(format!("{}(no file, {})", t.event, sel.pack)),
        }
        if crate::voicepack::is_recursion_style_key(&sel.event)
            && (sel.overlay || sel.stream)
            && c.display.as_ref().is_none_or(|d| d.priority < sel.priority)
        {
            c.display = Some(sel.clone());
        }
        if sel.file.is_some()
            && c.best
                .as_ref()
                .is_none_or(|b| (tier(&sel), sel.priority) > (tier(b), b.priority))
        {
            c.best = Some(sel);
        }
    }
    c
}

// ---------------------------------------------------------------- Warteschlange

#[derive(Debug, Clone, PartialEq)]
pub struct QueueItem {
    pub id: u64,
    pub priority: u8,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueAction {
    Play(u64),
    Stop(u64),
    Drop(u64),
}

/// Priority queue: at most `max_concurrent` at once; more important sounds
/// interrupt less important ones; waiting items expire after `pending_ttl_ms`.
#[derive(Debug, Clone)]
pub struct PlaybackQueue {
    pub max_concurrent: usize,
    pub max_pending: usize,
    pub pending_ttl_ms: u64,
    playing: Vec<(QueueItem, u64)>,
    pending: Vec<(QueueItem, u64)>,
}

impl PlaybackQueue {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            max_concurrent: max_concurrent.max(1),
            max_pending: 2,
            pending_ttl_ms: 1500,
            playing: Vec::new(),
            pending: Vec::new(),
        }
    }

    pub fn playing(&self) -> Vec<u64> {
        self.playing.iter().map(|(i, _)| i.id).collect()
    }

    pub fn pending(&self) -> Vec<u64> {
        self.pending.iter().map(|(i, _)| i.id).collect()
    }

    pub fn push(&mut self, item: QueueItem, now: u64) -> Vec<QueueAction> {
        let mut out = self.tick(now);
        if self.playing.len() < self.max_concurrent {
            out.push(QueueAction::Play(item.id));
            self.playing.push((item.clone(), now + item.duration_ms));
            return out;
        }
        // lowest running priority (oldest on a tie)
        let (low_idx, low_prio) = self
            .playing
            .iter()
            .enumerate()
            .min_by_key(|(_, (i, _))| i.priority)
            .map(|(idx, (i, _))| (idx, i.priority))
            .expect("playing is full");
        if item.priority > low_prio {
            let (stopped, _) = self.playing.remove(low_idx);
            out.push(QueueAction::Stop(stopped.id));
            out.push(QueueAction::Play(item.id));
            self.playing.push((item.clone(), now + item.duration_ms));
            // pending items with lower priority are now obsolete
            let p = item.priority;
            self.pending.retain(|(i, _)| {
                if i.priority < p {
                    out.push(QueueAction::Drop(i.id));
                    false
                } else {
                    true
                }
            });
            return out;
        }
        // enqueue sorted by priority; drop the excess
        self.pending.push((item, now));
        self.pending
            .sort_by(|a, b| b.0.priority.cmp(&a.0.priority).then(a.1.cmp(&b.1)));
        while self.pending.len() > self.max_pending {
            let (d, _) = self.pending.pop().expect("not empty");
            out.push(QueueAction::Drop(d.id));
        }
        out
    }

    /// Called by the output when a sound has finished (or could not be played).
    pub fn finished(&mut self, id: u64, now: u64) -> Vec<QueueAction> {
        self.playing.retain(|(i, _)| i.id != id);
        self.tick(now)
    }

    pub fn tick(&mut self, now: u64) -> Vec<QueueAction> {
        let mut out = Vec::new();
        self.playing.retain(|(_, end)| *end > now);
        let ttl = self.pending_ttl_ms;
        self.pending.retain(|(i, at)| {
            if now.saturating_sub(*at) > ttl {
                out.push(QueueAction::Drop(i.id));
                false
            } else {
                true
            }
        });
        while self.playing.len() < self.max_concurrent && !self.pending.is_empty() {
            let (i, _) = self.pending.remove(0);
            out.push(QueueAction::Play(i.id));
            self.playing.push((i.clone(), now + i.duration_ms));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voicepack::{Manifest, PackEntry, PackMeta};

    fn kill(victim: u64, hs: bool) -> Outcome {
        Outcome::Kill {
            victim,
            weapon_id: 1,
            vehicle_id: 0,
            headshot: hs,
        }
    }

    fn death(killer: u64) -> Outcome {
        Outcome::Death {
            killer,
            weapon_id: 1,
            vehicle_id: 0,
            headshot: false,
        }
    }

    fn facts(d: &mut VoiceDetector, o: Outcome, t: i64, prev: Option<u64>) -> Vec<(String, u32)> {
        d.on_outcomes(&[o], t, prev)
            .into_iter()
            .map(|x| (x.event, x.count))
            .collect()
    }

    #[test]
    fn multikill_chain_first_kill_and_hs_streak() {
        let mut d = VoiceDetector::default();
        assert_eq!(
            facts(&mut d, kill(1, true), 10, None),
            vec![("kill".into(), 1), ("first_kill".into(), 1)]
        );
        assert_eq!(
            facts(&mut d, kill(2, true), 12, None),
            vec![
                ("kill".into(), 1),
                ("multikill".into(), 2),
                ("headshot_streak".into(), 2)
            ]
        );
        assert_eq!(
            facts(&mut d, kill(3, false), 15, None),
            vec![("kill".into(), 1), ("multikill".into(), 3)]
        );
        // outside the window: new chain
        assert_eq!(
            facts(&mut d, kill(4, false), 30, None),
            vec![("kill".into(), 1)]
        );
    }

    #[test]
    fn death_resets_and_nemesis_death_streak() {
        let mut d = VoiceDetector::default();
        facts(&mut d, kill(1, false), 10, None);
        let f = facts(&mut d, death(9), 11, None);
        assert_eq!(f, vec![("death".into(), 0), ("death_streak".into(), 1)]);
        let f = facts(&mut d, death(9), 20, None);
        assert!(f.contains(&("nemesis".into(), 2)));
        let f = facts(&mut d, death(9), 30, None);
        assert!(f.contains(&("nemesis".into(), 3)));
        assert!(f.contains(&("death_streak".into(), 3)));
        // no chain after a death, revenge on the killer
        let f = facts(&mut d, kill(9, false), 31, Some(9));
        assert!(f.contains(&("revenge".into(), 0)));
        assert!(!f.iter().any(|(e, _)| e == "multikill"));
        // kill right after dying to the same player → mutual
        assert!(f.contains(&("mutual".into(), 0)));
    }

    #[test]
    fn repeat_victim_and_roadkill() {
        let mut d = VoiceDetector::default();
        facts(&mut d, kill(5, false), 0, None);
        let f = facts(&mut d, kill(5, false), 100, None);
        assert!(f.contains(&("repeat_victim".into(), 2)));
        let rk = d.on_outcomes(
            &[Outcome::Kill {
                victim: 6,
                weapon_id: 0,
                vehicle_id: 4,
                headshot: false,
            }],
            200,
            None,
        );
        assert!(rk[0].roadkill);
    }

    #[test]
    fn streak_facts() {
        let mut d = VoiceDetector::default();
        let f = facts(&mut d, Outcome::Streak { count: 5 }, 10, None);
        assert_eq!(f, vec![("streak".into(), 5)]);
        let f = facts(&mut d, Outcome::StreakEnded { count: 7 }, 20, None);
        assert_eq!(f, vec![("streak_ended".into(), 7)]);
    }

    fn pack(id: &str, entries: Vec<PackEntry>) -> VoicePack {
        VoicePack {
            id: id.into(),
            dir: PathBuf::from(format!("/packs/{id}")),
            manifest: Manifest {
                pack: PackMeta {
                    source: String::new(),
                    name: id.into(),
                    author: String::new(),
                    version: "1".into(),
                    license: String::new(),
                    description: String::new(),
                },
                events: entries,
            },
            warnings: vec![],
            stats: Default::default(),
        }
    }

    fn entry(event: &str, files: &[&str], when: Conditions) -> PackEntry {
        PackEntry {
            when,
            ..PackEntry::new(event, files.iter().map(|s| (*s).to_owned()).collect())
        }
    }

    #[test]
    fn standard_tones_only_without_real_pack_or_when_enabled() {
        use crate::voicepack::STANDARD_PACK_ID;
        let std = pack(
            STANDARD_PACK_ID,
            vec![entry("death", &["d.wav"], Conditions::default())],
        );
        let real = pack(
            "deadpool",
            vec![entry("PER_KILL", &["k.ogg"], Conditions::default())],
        );
        let both = [std.clone(), real.clone()];
        assert_eq!(effective_packs(&both, false).len(), 1);
        assert_eq!(effective_packs(&both, true).len(), 2);
        assert_eq!(effective_packs(std::slice::from_ref(&std), false).len(), 1);
        // death with only PER_KILL in the real pack: silent
        let mut s = VoiceSelector::new(1);
        let ev = [(Trigger::new("death"), 40u8)];
        let c = choose(
            &mut s,
            &ev,
            &BTreeMap::new(),
            &effective_packs(&both, false),
            &[],
            0,
        );
        assert!(c.best.is_none());
    }

    /// One event system: pack keys serve the base events, most specific first,
    /// the standard tones only as the last resort (even at rank 1).
    #[test]
    fn pack_keys_serve_base_events() {
        use crate::triggers::{TriggerEngine, TriggerSet};
        use crate::voicepack::STANDARD_PACK_ID;
        let set = TriggerSet::default_set();
        let std = pack(
            STANDARD_PACK_ID,
            ["kill", "headshot", "multikill", "death", "session_start"]
                .iter()
                .map(|e| entry(e, &[&format!("{e}.wav")], Conditions::default()))
                .collect(),
        );
        let all = [
            "DOUBLE_KILL",
            "HEAD_SHOT",
            "PER_KILL",
            "DEATH_INSULT",
            "SESSION_STARTED",
        ];
        let deadpool = |keys: &[&str]| {
            pack(
                "deadpool",
                keys.iter()
                    .map(|k| entry(k, &[&format!("{k}.ogg")], Conditions::default()))
                    .collect(),
            )
        };
        let run = |packs: &[VoicePack], facts: &[Trigger]| -> (String, Vec<String>) {
            let mut eng = TriggerEngine::default();
            let events: Vec<(Trigger, u8)> =
                facts.iter().flat_map(|f| eng.evaluate(&set, f)).collect();
            let mut sel = VoiceSelector::new(7);
            let c = choose(&mut sel, &events, &BTreeMap::new(), packs, &[], 0);
            let b = c.best.unwrap();
            (format!("{}@{}", b.event, b.pack_id), c.tried)
        };
        let mut kill = Trigger::new("kill");
        kill.headshot = true;
        let mut chain = kill.clone();
        chain.event = "multikill".into();
        chain.count = 2;
        let facts = [kill, chain];
        // standard tones at rank 1 on purpose
        let (hit, tried) = run(&[std.clone(), deadpool(&all)], &facts);
        assert_eq!(hit, "DOUBLE_KILL@deadpool", "{tried:?}");
        assert!(tried[0].starts_with("DOUBLE_KILL(hit"), "{tried:?}");
        let (hit, _) = run(&[std.clone(), deadpool(&all[1..])], &facts);
        assert_eq!(hit, "HEAD_SHOT@deadpool");
        let (hit, tried) = run(&[std.clone(), deadpool(&all[2..])], &facts);
        assert_eq!(hit, "PER_KILL@deadpool");
        assert!(tried.iter().any(|t| t == "HEAD_SHOT(miss)"), "{tried:?}");
        let (hit, _) = run(&[std.clone(), deadpool(&all[3..])], &facts);
        assert_eq!(hit, format!("multikill@{STANDARD_PACK_ID}"));
        let (hit, _) = run(&[std.clone(), deadpool(&all)], &[Trigger::new("death")]);
        assert_eq!(hit, "DEATH_INSULT@deadpool");
        let (hit, _) = run(
            &[std.clone(), deadpool(&all)],
            &[Trigger::new("session_start")],
        );
        assert_eq!(hit, "SESSION_STARTED@deadpool");
        let (hit, _) = run(&[std.clone(), deadpool(&[])], &[Trigger::new("death")]);
        assert_eq!(hit, format!("death@{STANDARD_PACK_ID}"));
    }

    #[test]
    fn selection_specificity_fallback_and_no_repeat() {
        let a = pack(
            "a",
            vec![
                entry("multikill", &["mk.ogg"], Conditions::default()),
                entry(
                    "multikill",
                    &["triple.ogg"],
                    Conditions {
                        min_count: Some(3),
                        max_count: Some(3),
                        ..Default::default()
                    },
                ),
                entry(
                    "kill",
                    &["k1.ogg", "k2.ogg", "k3.ogg"],
                    Conditions::default(),
                ),
            ],
        );
        let b = pack("b", vec![entry("death", &["d.ogg"], Conditions::default())]);
        let packs = [a, b];
        let none = BTreeMap::new();
        let mut s = VoiceSelector::new(42);
        let mut t = Trigger::new("multikill");
        t.count = 3;
        let f = |s: Option<Selection>| s.and_then(|x| x.file).unwrap();
        assert!(f(s.select(&t, &none, &packs, 0)).ends_with("triple.ogg"));
        t.count = 2;
        assert!(f(s.select(&t, &none, &packs, 0)).ends_with("mk.ogg"));
        let d = s.select(&Trigger::new("death"), &none, &packs, 0).unwrap();
        assert_eq!(d.pack, "b");
        assert_eq!(d.priority, 40);
        let mut last = PathBuf::new();
        for i in 0..50 {
            let x = f(s.select(&Trigger::new("kill"), &none, &packs, i));
            assert_ne!(x, last);
            last = x;
        }
        let ov = BTreeMap::from([("death".to_string(), vec![PathBuf::from("/me/d.wav")])]);
        assert_eq!(
            f(s.select(&Trigger::new("death"), &ov, &packs, 0)),
            PathBuf::from("/me/d.wav")
        );
        assert!(s.select(&Trigger::new("login"), &none, &packs, 0).is_none());
    }

    #[test]
    fn silent_default_and_display_flags() {
        let mut silent = entry("TEAM_KILLER", &["tk.ogg"], Conditions::default());
        silent.silent = true;
        let mut default = entry("HEAD_SHOT", &[], Conditions::default());
        default.stream = false;
        let rec = pack("rec", vec![silent, default]);
        let std = pack(
            "std",
            vec![
                entry("TEAM_KILLER", &["std_tk.wav"], Conditions::default()),
                entry("HEAD_SHOT", &["std_hs.wav"], Conditions::default()),
            ],
        );
        let packs = [rec, std];
        let mut s = VoiceSelector::default();
        let none = BTreeMap::new();
        // SOUND=false: no sound, no fallback
        let tk = s
            .select(&Trigger::new("TEAM_KILLER"), &none, &packs, 0)
            .unwrap();
        assert_eq!(tk.file, None);
        // Default: sound from the next pack, display flags from the Recursion entry
        let hs = s
            .select(&Trigger::new("HEAD_SHOT"), &none, &packs, 0)
            .unwrap();
        assert!(hs.file.unwrap().ends_with("std_hs.wav"));
        assert!(hs.overlay);
        assert!(!hs.stream);
    }

    #[test]
    fn conditions_and_cooldown() {
        let mut e = entry(
            "kill",
            &["sniper.ogg"],
            Conditions {
                headshot: Some(true),
                weapon_category: vec!["Sniper Rifle".into()],
                ..Default::default()
            },
        );
        e.cooldown_ms = 1000;
        let p = [pack("a", vec![e])];
        let mut s = VoiceSelector::default();
        let mut t = Trigger::new("kill");
        t.headshot = true;
        t.weapon_category = Some("sniper rifle".into());
        let none = BTreeMap::new();
        assert!(s.select(&t, &none, &p, 0).is_some());
        assert!(s.select(&t, &none, &p, 500).is_none());
        assert!(s.select(&t, &none, &p, 1500).is_some());
        t.headshot = false;
        assert!(s.select(&t, &none, &p, 5000).is_none());
    }

    fn item(id: u64, priority: u8) -> QueueItem {
        QueueItem {
            id,
            priority,
            duration_ms: 1000,
        }
    }

    #[test]
    fn queue_interrupts_and_limits() {
        let mut q = PlaybackQueue::new(1);
        assert_eq!(q.push(item(1, 10), 0), vec![QueueAction::Play(1)]);
        assert_eq!(q.push(item(2, 10), 10), vec![]);
        assert_eq!(
            q.push(item(3, 70), 20),
            vec![
                QueueAction::Stop(1),
                QueueAction::Play(3),
                QueueAction::Drop(2)
            ]
        );
        let mut acts = vec![];
        for id in 10..20 {
            acts.extend(q.push(item(id, 10), 30));
        }
        assert_eq!(q.pending().len(), 2);
        assert_eq!(
            acts.iter()
                .filter(|a| matches!(a, QueueAction::Drop(_)))
                .count(),
            8
        );
        assert_eq!(q.playing(), vec![3]);
        assert_eq!(q.finished(3, 500), vec![QueueAction::Play(10)]);
    }

    #[test]
    fn queue_pending_expires() {
        let mut q = PlaybackQueue::new(1);
        q.push(item(1, 50), 0);
        q.push(item(2, 10), 0);
        let a = q.tick(1600);
        assert!(a.contains(&QueueAction::Drop(2)));
        assert!(q.pending().is_empty());
    }

    #[test]
    fn queue_two_concurrent() {
        let mut q = PlaybackQueue::new(2);
        q.push(item(1, 10), 0);
        q.push(item(2, 20), 0);
        let a = q.push(item(3, 15), 0);
        assert_eq!(a, vec![QueueAction::Stop(1), QueueAction::Play(3)]);
    }
}
