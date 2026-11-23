//! Manages the sessions of several characters and builds display snapshots.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::achievements::{AchievementEngine, RuleSet, Unlocked};
use crate::census::{LifetimeStats, NameCache};
use crate::classes::{Class, ClassStats, class_of_opt, class_stats};
use crate::events::{CharacterId, GameEvent};
use crate::session::{FeedEntry, Outcome, Rates, Session, SessionConfig, SessionStats};

#[derive(Debug, Clone, PartialEq)]
pub enum TrackerOutput {
    SessionStarted(CharacterId),
    Outcome {
        character_id: CharacterId,
        timestamp: i64,
        outcome: Outcome,
    },
    Achievement {
        character_id: CharacterId,
        unlocked: Unlocked,
    },
    SessionEnded(Box<Session>),
}

#[derive(Debug, Clone)]
pub struct TrackedCharacter {
    pub name: String,
    pub faction_id: Option<u32>,
    /// One of the user's own characters (false = squad/outfit member: tracked
    /// and stored, but never the focus, no sounds, no crosshair feedback).
    pub own: bool,
}

pub struct Tracker {
    config: SessionConfig,
    idle_timeout_secs: i64,
    rules: RuleSet,
    characters: HashMap<CharacterId, TrackedCharacter>,
    active: HashMap<CharacterId, (Session, AchievementEngine)>,
    focus: Option<CharacterId>,
}

/// Checks whether an event concerns a character.
pub fn involves(ev: &GameEvent, id: CharacterId) -> bool {
    match ev {
        GameEvent::Death(d) => d.character_id == id || d.attacker_character_id == id,
        GameEvent::VehicleDestroy(v) => v.character_id == id || v.attacker_character_id == id,
        GameEvent::GainExperience(g) => g.character_id == id || g.other_id == id,
        GameEvent::PlayerLogin(p) | GameEvent::PlayerLogout(p) => p.character_id == id,
        GameEvent::PlayerFacilityCapture(p) | GameEvent::PlayerFacilityDefend(p) => {
            p.character_id == id
        }
        GameEvent::ItemAdded(i) => i.character_id == id,
        GameEvent::AchievementEarned(a) => a.character_id == id,
        GameEvent::BattleRankUp(b) => b.character_id == id,
        GameEvent::FacilityControl(_) | GameEvent::Other { .. } => false,
    }
}

impl Tracker {
    pub fn new(config: SessionConfig, idle_timeout_secs: i64, rules: RuleSet) -> Self {
        Self {
            config,
            idle_timeout_secs,
            rules,
            characters: HashMap::new(),
            active: HashMap::new(),
            focus: None,
        }
    }

    pub fn set_characters(&mut self, chars: HashMap<CharacterId, TrackedCharacter>) {
        self.characters = chars;
    }

    /// IDs of the tracked characters.
    pub fn character_ids(&self) -> impl Iterator<Item = CharacterId> + '_ {
        self.characters.keys().copied()
    }

    /// IDs of the user's own characters.
    pub fn own_ids(&self) -> impl Iterator<Item = CharacterId> + '_ {
        self.characters
            .iter()
            .filter(|(_, c)| c.own)
            .map(|(id, _)| *id)
    }

    pub fn is_own(&self, id: CharacterId) -> bool {
        self.characters.get(&id).is_some_and(|c| c.own)
    }

    /// Tracked character (name, faction, own).
    pub fn character(&self, id: CharacterId) -> Option<&TrackedCharacter> {
        self.characters.get(&id)
    }

    pub fn set_rules(&mut self, rules: RuleSet) {
        self.rules = rules;
    }

    pub fn focus(&self) -> Option<CharacterId> {
        self.focus
    }

    pub fn set_focus(&mut self, id: CharacterId) {
        self.focus = Some(id);
    }

    pub fn session(&self, id: CharacterId) -> Option<&Session> {
        self.active.get(&id).map(|(s, _)| s)
    }

    pub fn active_sessions(&self) -> impl Iterator<Item = &Session> {
        self.active.values().map(|(s, _)| s)
    }

    pub fn session_mut(&mut self, id: CharacterId) -> Option<&mut Session> {
        self.active.get_mut(&id).map(|(s, _)| s)
    }

    fn start(&mut self, id: CharacterId, at: i64) -> bool {
        if self.active.contains_key(&id) {
            return false;
        }
        let Some(c) = self.characters.get(&id) else {
            return false;
        };
        let s = Session::new(id, c.name.clone(), c.faction_id, at, self.config);
        let own = c.own;
        self.active
            .insert(id, (s, AchievementEngine::new(&self.rules)));
        if own {
            self.focus = Some(id);
        }
        true
    }

    /// Manual start (e.g. a button in the UI).
    pub fn start_manual(&mut self, id: CharacterId, now: i64) -> Vec<TrackerOutput> {
        if self.start(id, now) {
            vec![TrackerOutput::SessionStarted(id)]
        } else {
            vec![]
        }
    }

    pub fn stop(&mut self, id: CharacterId, at: i64) -> Option<Session> {
        let (mut s, _) = self.active.remove(&id)?;
        s.end(at.max(s.last_event_at));
        if self.focus == Some(id) {
            self.focus = self.active.keys().next().copied();
        }
        Some(s)
    }

    pub fn handle(&mut self, ev: &GameEvent) -> Vec<TrackerOutput> {
        let mut out = Vec::new();
        let ts = ev.timestamp();
        let ids: Vec<CharacterId> = self
            .characters
            .keys()
            .copied()
            .filter(|id| involves(ev, *id))
            .collect();
        for id in ids {
            let is_logout = matches!(ev, GameEvent::PlayerLogout(p) if p.character_id == id);
            if !self.active.contains_key(&id) {
                if is_logout {
                    continue;
                }
                if self.start(id, ts) {
                    out.push(TrackerOutput::SessionStarted(id));
                }
            }
            let Some((s, engine)) = self.active.get_mut(&id) else {
                continue;
            };
            let prev_killer = s.last_killer;
            let outcomes = s.apply(ev);
            if !outcomes.is_empty() && self.characters.get(&id).is_some_and(|c| c.own) {
                self.focus = Some(id);
            }
            let unlocked = engine.on_outcomes(s, &outcomes, ts, prev_killer);
            s.achievements.extend(unlocked.iter().cloned());
            for o in outcomes {
                out.push(TrackerOutput::Outcome {
                    character_id: id,
                    timestamp: ts,
                    outcome: o,
                });
            }
            for u in unlocked {
                out.push(TrackerOutput::Achievement {
                    character_id: id,
                    unlocked: u,
                });
            }
            if is_logout && let Some(s) = self.stop(id, ts) {
                out.push(TrackerOutput::SessionEnded(Box::new(s)));
            }
        }
        out
    }

    /// Ends sessions without an event for `idle_timeout_secs`.
    pub fn tick(&mut self, now: i64) -> Vec<TrackerOutput> {
        let stale: Vec<CharacterId> = self
            .active
            .iter()
            .filter(|(_, (s, _))| now - s.last_event_at > self.idle_timeout_secs)
            .map(|(id, _)| *id)
            .collect();
        stale
            .into_iter()
            .filter_map(|id| {
                let last = self.active.get(&id)?.0.last_event_at;
                self.stop(id, last)
            })
            .map(|s| TrackerOutput::SessionEnded(Box::new(s)))
            .collect()
    }

    pub fn achievements(&self, id: CharacterId) -> &[Unlocked] {
        self.active
            .get(&id)
            .map(|(_, e)| e.unlocked.as_slice())
            .unwrap_or(&[])
    }
}

// ---------------------------------------------------------------- Snapshot

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedRow {
    pub timestamp: i64,
    pub kind: String,
    /// As a string because character IDs exceed `Number.MAX_SAFE_INTEGER`.
    pub other_id: String,
    pub other_name: String,
    /// Why there is no name: "" (name present), "self" (suicide), "environment"
    /// (no attacker, e.g. fall damage or a crash), "pending" (Census lookup not
    /// answered yet) or "unknown" (Census does not know the character).
    #[serde(default)]
    pub other_label: String,
    pub weapon_name: String,
    pub headshot: bool,
    pub vehicle_name: String,
    /// Class of the other party (kill: victim, death: attacker), if known.
    #[serde(default)]
    pub other_class: Option<Class>,
    /// Own class at the time, if known.
    #[serde(default)]
    pub my_class: Option<Class>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaponRow {
    pub weapon_id: u32,
    pub name: String,
    pub kills: u32,
    pub headshots: u32,
    pub hsr: f64,
    pub deaths_by: u32,
    pub lifetime_kills: Option<u64>,
    pub lifetime_hsr: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    pub session: f64,
    pub lifetime: f64,
    /// Deviation from the lifetime value in percent (positive = better).
    pub delta_pct: Option<f64>,
}

impl Metric {
    pub fn new(session: f64, lifetime: f64) -> Self {
        let delta_pct = (lifetime > 0.0).then(|| (session - lifetime) / lifetime * 100.0);
        Self {
            session,
            lifetime,
            delta_pct,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub kd: Metric,
    pub kpm: Metric,
    pub hsr: Metric,
    pub xp_per_hour: Metric,
    pub lifetime_kills: u64,
    pub lifetime_deaths: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: Option<i64>,
    pub character_id: String,
    pub character_name: String,
    pub faction_id: Option<u32>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub active: bool,
    pub stats: SessionStats,
    pub rates: Rates,
    pub streak: u32,
    pub best_streak: u32,
    pub killfeed: Vec<FeedRow>,
    pub weapons: Vec<WeaponRow>,
    pub vehicle_kills: Vec<(String, u32)>,
    pub achievements: Vec<Unlocked>,
    pub comparison: Option<Comparison>,
    /// Overlay block "Squad top 3" (empty unless the block is switched on).
    #[serde(default)]
    pub squad_top: Vec<SquadTopRow>,
    /// Last server/continent of the session and their names (Census, cached;
    /// `None` while unknown).
    #[serde(default)]
    pub world_id: Option<u32>,
    #[serde(default)]
    pub zone_id: Option<u32>,
    #[serde(default)]
    pub world_name: Option<String>,
    #[serde(default)]
    pub zone_name: Option<String>,
    /// Current own class (last loadout seen), if known.
    #[serde(default)]
    pub class: Option<Class>,
    /// Kills/deaths by enemy class and own class use (data version 4).
    #[serde(default)]
    pub classes: ClassStats,
}

/// Ranking of the overlay block "Squad top 3".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquadMetric {
    Kd,
    Kills,
    Kpm,
}

impl SquadMetric {
    pub fn parse(s: &str) -> Self {
        match s {
            "kills" => Self::Kills,
            "kpm" => Self::Kpm,
            _ => Self::Kd,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SquadTopRow {
    pub character_id: String,
    pub name: String,
    pub faction_id: Option<u32>,
    /// One of the user's own characters (marked in the overlay).
    pub own: bool,
    pub kills: u32,
    pub deaths: u32,
    /// Value of the chosen metric.
    pub value: f64,
    /// Enough kills for the ratio metric (K/D, KPM); otherwise the row is
    /// listed after the qualified ones and shown dimmed.
    #[serde(default = "yes")]
    pub qualified: bool,
    /// A session is running (false: added, but no event yet).
    #[serde(default)]
    pub session: bool,
    /// Current class (last loadout seen in a Death/GainExperience event).
    #[serde(default)]
    pub class: Option<Class>,
}

fn yes() -> bool {
    true
}

/// The three best of the squad by `metric`: every squad member (also without
/// a session yet, e.g. just added) and the user's own characters that have a
/// session. For the ratio metrics (K/D, KPM) a character needs at least
/// `min_kills` kills to be ranked by the value, so a 1/0 start is not first;
/// below that it still appears, after the qualified ones, by kills. Empty
/// slots only remain when there are fewer than three characters. Ties: more
/// kills first, then the name.
pub fn squad_top(t: &Tracker, metric: SquadMetric, min_kills: u32, now: i64) -> Vec<SquadTopRow> {
    let needed = match metric {
        SquadMetric::Kills => 0,
        SquadMetric::Kd | SquadMetric::Kpm => min_kills.max(1),
    };
    let mut rows: Vec<SquadTopRow> = t
        .characters
        .iter()
        .filter_map(|(id, c)| {
            let s = t.session(*id);
            if c.own && s.is_none() {
                return None;
            }
            let (kills, deaths, value) = match s {
                Some(s) => {
                    let r = s.rates(now);
                    let v = match metric {
                        SquadMetric::Kills => f64::from(s.stats.kills),
                        SquadMetric::Kd => r.kd,
                        SquadMetric::Kpm => r.kpm,
                    };
                    (s.stats.kills, s.stats.deaths, v)
                }
                None => (0, 0, 0.0),
            };
            Some(SquadTopRow {
                character_id: id.to_string(),
                name: c.name.clone(),
                faction_id: c.faction_id.or(s.and_then(|s| s.faction_id)),
                own: c.own,
                kills,
                deaths,
                value,
                qualified: kills >= needed,
                session: s.is_some(),
                class: s.and_then(|s| class_of_opt(s.loadout_id)),
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        b.qualified
            .cmp(&a.qualified)
            .then_with(|| {
                if a.qualified && b.qualified {
                    b.value.total_cmp(&a.value)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then(b.kills.cmp(&a.kills))
            .then(a.name.cmp(&b.name))
    });
    rows.truncate(3);
    rows
}

fn feed_row(e: &FeedEntry, me: CharacterId, names: &NameCache) -> Option<FeedRow> {
    let (kind, other, weapon, headshot, vehicle) = match &e.outcome {
        Outcome::Kill {
            victim,
            weapon_id,
            vehicle_id,
            headshot,
        } => ("kill", *victim, *weapon_id, *headshot, *vehicle_id),
        Outcome::Death {
            killer,
            weapon_id,
            vehicle_id,
            headshot,
        } => ("death", *killer, *weapon_id, *headshot, *vehicle_id),
        Outcome::Teamkill { victim, weapon_id } => ("teamkill", *victim, *weapon_id, false, 0),
        Outcome::Teamkilled { killer, weapon_id } => ("teamkilled", *killer, *weapon_id, false, 0),
        Outcome::Suicide { weapon_id } => ("suicide", me, *weapon_id, false, 0),
        Outcome::VehicleKill {
            vehicle_id,
            owner,
            weapon_id,
        } => ("vehicle_kill", *owner, *weapon_id, false, *vehicle_id),
        Outcome::VehicleLost {
            vehicle_id,
            attacker,
        } => ("vehicle_lost", *attacker, 0, false, *vehicle_id),
        _ => return None,
    };
    let other_name = names
        .characters
        .get(&other)
        .map(|(n, _)| n.clone())
        .unwrap_or_default();
    let other_label = if kind == "suicide" {
        "self"
    } else if other == 0 {
        "environment"
    } else if !other_name.is_empty() {
        ""
    } else if names.not_found.contains_key(&other) {
        "unknown"
    } else {
        "pending"
    };
    Some(FeedRow {
        timestamp: e.timestamp,
        kind: kind.into(),
        other_id: other.to_string(),
        other_name,
        other_label: other_label.into(),
        weapon_name: names.items.get(&weapon).cloned().unwrap_or_default(),
        headshot,
        vehicle_name: if vehicle == 0 {
            String::new()
        } else {
            names.vehicles.get(&vehicle).cloned().unwrap_or_default()
        },
        other_class: class_of_opt(e.other_loadout),
        my_class: class_of_opt(e.my_loadout),
    })
}

pub fn snapshot(
    s: &Session,
    active: bool,
    now: i64,
    names: &NameCache,
    lifetime: Option<&LifetimeStats>,
    achievements: &[Unlocked],
) -> SessionSnapshot {
    let rates = s.rates(now);
    let mut weapons: Vec<WeaponRow> = s
        .weapons
        .iter()
        .filter(|(_, w)| w.kills > 0)
        .map(|(id, w)| {
            let lt = lifetime.and_then(|l| l.weapons.get(id));
            WeaponRow {
                weapon_id: *id,
                name: names
                    .items
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| format!("#{id}")),
                kills: w.kills,
                headshots: w.headshots,
                hsr: if w.kills > 0 {
                    w.headshots as f64 / w.kills as f64
                } else {
                    0.0
                },
                deaths_by: w.deaths_by,
                lifetime_kills: lt.map(|l| l.kills),
                lifetime_hsr: lt
                    .and_then(|l| (l.kills > 0).then(|| l.headshots as f64 / l.kills as f64)),
            }
        })
        .collect();
    weapons.sort_by(|a, b| b.kills.cmp(&a.kills).then(a.weapon_id.cmp(&b.weapon_id)));
    let killfeed = s
        .killfeed()
        .rev()
        .filter_map(|e| feed_row(e, s.character_id, names))
        .collect();
    let vehicle_kills = s
        .vehicle_kills_by_type
        .iter()
        .map(|(id, n)| {
            (
                names
                    .vehicles
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| format!("#{id}")),
                *n,
            )
        })
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect();
    let comparison = lifetime.map(|l| Comparison {
        kd: Metric::new(rates.kd, l.kd()),
        kpm: Metric::new(rates.kpm, l.kpm()),
        hsr: Metric::new(rates.hsr, l.hsr()),
        xp_per_hour: Metric::new(rates.xp_per_hour, l.score_per_hour()),
        lifetime_kills: l.kills,
        lifetime_deaths: l.deaths,
    });
    SessionSnapshot {
        session_id: s.id,
        character_id: s.character_id.to_string(),
        character_name: s.character_name.clone(),
        faction_id: s.faction_id,
        started_at: s.started_at,
        ended_at: s.ended_at,
        active,
        stats: s.stats.clone(),
        rates,
        streak: s.streak,
        best_streak: s.best_streak,
        killfeed,
        weapons,
        vehicle_kills,
        achievements: achievements.to_vec(),
        comparison,
        squad_top: Vec::new(),
        world_id: s.world_id,
        zone_id: s.zone_id,
        world_name: s.world_id.and_then(|i| names.worlds.get(&i).cloned()),
        zone_name: s.zone_id.and_then(|i| names.zones.get(&i).cloned()),
        class: class_of_opt(s.loadout_id),
        classes: class_stats(&s.timeline),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Death, PlayerSession};

    const ME: u64 = 5428000000000000042;

    fn tracker() -> Tracker {
        let mut t = Tracker::new(SessionConfig::default(), 1800, RuleSet::default_rules());
        t.set_characters(HashMap::from([(
            ME,
            TrackedCharacter {
                name: "SampleSoldier".into(),
                faction_id: Some(3),
                own: true,
            },
        )]));
        t
    }

    #[test]
    fn squad_members_are_tracked_but_never_the_focus() {
        const MATE: u64 = 5428000000000000077;
        let mut t = tracker();
        t.set_characters(HashMap::from([
            (
                ME,
                TrackedCharacter {
                    name: "SampleSoldier".into(),
                    faction_id: Some(3),
                    own: true,
                },
            ),
            (
                MATE,
                TrackedCharacter {
                    name: "SquadMate".into(),
                    faction_id: Some(3),
                    own: false,
                },
            ),
        ]));
        let kill = |attacker: u64, ts: i64| {
            GameEvent::Death(Death {
                timestamp: ts,
                world_id: 10,
                zone_id: 2,
                character_id: 5429000000000000001,
                attacker_character_id: attacker,
                attacker_weapon_id: 80,
                attacker_vehicle_id: 0,
                attacker_loadout_id: 0,
                character_loadout_id: 0,
                is_headshot: false,
                team_id: Some(1),
                attacker_team_id: Some(3),
            })
        };
        t.handle(&kill(MATE, 100));
        assert!(t.session(MATE).is_some(), "squad member gets a session");
        assert_eq!(t.focus(), None, "but not the focus");
        t.handle(&kill(ME, 101));
        t.handle(&kill(MATE, 102));
        assert_eq!(t.focus(), Some(ME));
        assert_eq!(t.own_ids().collect::<Vec<_>>(), vec![ME]);
        assert!(!t.is_own(MATE));
    }

    fn kill(ts: i64) -> GameEvent {
        GameEvent::Death(Death {
            timestamp: ts,
            world_id: 10,
            zone_id: 2,
            character_id: 99,
            attacker_character_id: ME,
            attacker_weapon_id: 80,
            attacker_vehicle_id: 0,
            attacker_loadout_id: 0,
            character_loadout_id: 0,
            is_headshot: false,
            team_id: Some(1),
            attacker_team_id: Some(3),
        })
    }

    #[test]
    fn squad_top_ranks_with_minimum_kills() {
        let ids: Vec<u64> = (1..=5).map(|i| 5428000000000000100 + i).collect();
        let mut t = Tracker::new(SessionConfig::default(), 1800, RuleSet::default_rules());
        let mut chars = HashMap::new();
        for (i, id) in ids.iter().enumerate() {
            chars.insert(
                *id,
                TrackedCharacter {
                    name: format!("Mate{i}"),
                    faction_id: Some(1),
                    own: i == 0,
                },
            );
        }
        t.set_characters(chars);
        let ev = |attacker: u64, victim: u64, ts: i64| {
            GameEvent::Death(Death {
                timestamp: ts,
                world_id: 10,
                zone_id: 2,
                character_id: victim,
                attacker_character_id: attacker,
                attacker_weapon_id: 80,
                attacker_vehicle_id: 0,
                attacker_loadout_id: 0,
                character_loadout_id: 0,
                is_headshot: false,
                team_id: Some(2),
                attacker_team_id: Some(1),
            })
        };
        let foe = 5429000000000000001;
        // Mate0 (own): 6 kills 2 deaths, Mate1: 3 kills 0 deaths (K/D 3.0 but
        // under the minimum), Mate2: 10 kills 5 deaths, Mate3: 5 kills 1 death,
        // Mate4: no session at all.
        let mut ts = 1000;
        let mut kills = |who: u64, n: u32, t: &mut Tracker| {
            for _ in 0..n {
                ts += 1;
                t.handle(&ev(who, foe, ts));
            }
        };
        kills(ids[0], 6, &mut t);
        kills(ids[1], 3, &mut t);
        kills(ids[2], 10, &mut t);
        kills(ids[3], 5, &mut t);
        for (who, n) in [(ids[0], 2), (ids[2], 5), (ids[3], 1)] {
            for k in 0..n {
                t.handle(&ev(foe, who, 2000 + k));
            }
        }
        let names = |v: Vec<SquadTopRow>| v.into_iter().map(|r| r.name).collect::<Vec<_>>();
        let kd = squad_top(&t, SquadMetric::Kd, 5, 3000);
        assert_eq!(names(kd.clone()), ["Mate3", "Mate0", "Mate2"]);
        assert!(kd[1].own && !kd[0].own);
        // without a minimum the 1/0 start would be in the top 3
        assert!(names(squad_top(&t, SquadMetric::Kd, 0, 3000)).contains(&"Mate1".to_string()));
        assert_eq!(
            names(squad_top(&t, SquadMetric::Kills, 5, 3000)),
            ["Mate2", "Mate0", "Mate3"]
        );
        // KPM: same duration for everyone here, so the kills decide; Mate1 is
        // filtered by the minimum
        let kpm = squad_top(&t, SquadMetric::Kpm, 5, 3000);
        assert_eq!(kpm.len(), 3);
        assert!(kpm.iter().all(|r| r.kills >= 5));
        assert_eq!(SquadMetric::parse("bogus"), SquadMetric::Kd);
        // nobody has 20 kills: all listed anyway, by kills, not qualified
        let all = squad_top(&t, SquadMetric::Kd, 20, 3000);
        assert_eq!(names(all.clone()), ["Mate2", "Mate0", "Mate3"]);
        assert!(all.iter().all(|r| !r.qualified && r.session));
        // with min 5, the 3-kill member is not in the top 3 but would be next
        assert!(kd.iter().all(|r| r.qualified));
    }

    #[test]
    fn squad_member_without_events_is_listed() {
        const MATE: u64 = 5428000000000000077;
        let mut t = tracker();
        t.handle(&kill(100));
        // added at run time: no session yet
        let mut chars = HashMap::from([(
            ME,
            TrackedCharacter {
                name: "SampleSoldier".into(),
                faction_id: Some(3),
                own: true,
            },
        )]);
        chars.insert(
            MATE,
            TrackedCharacter {
                name: "NewMate".into(),
                faction_id: Some(3),
                own: false,
            },
        );
        t.set_characters(chars);
        let top = squad_top(&t, SquadMetric::Kd, 5, 200);
        assert_eq!(top.len(), 2, "two characters, so one empty slot");
        let mate = top.iter().find(|r| r.name == "NewMate").expect("listed");
        assert!(!mate.session && !mate.qualified);
        assert_eq!(mate.kills, 0);
        // the own character without a session is not listed
        let mut t2 = tracker();
        let mut c2 = HashMap::new();
        c2.insert(
            MATE,
            TrackedCharacter {
                name: "NewMate".into(),
                faction_id: Some(3),
                own: false,
            },
        );
        c2.insert(
            ME,
            TrackedCharacter {
                name: "SampleSoldier".into(),
                faction_id: Some(3),
                own: true,
            },
        );
        t2.set_characters(c2);
        assert_eq!(
            names_of(&squad_top(&t2, SquadMetric::Kills, 5, 200)),
            ["NewMate"]
        );
    }

    fn names_of(v: &[SquadTopRow]) -> Vec<String> {
        v.iter().map(|r| r.name.clone()).collect()
    }

    #[test]
    fn login_kill_logout_lifecycle() {
        let mut t = tracker();
        let o = t.handle(&GameEvent::PlayerLogin(PlayerSession {
            timestamp: 100,
            world_id: 10,
            character_id: ME,
        }));
        assert_eq!(o[0], TrackerOutput::SessionStarted(ME));
        t.handle(&kill(110));
        let o = t.handle(&kill(112));
        assert!(
            o.iter()
                .any(|x| matches!(x, TrackerOutput::Achievement { .. }))
        );
        let o = t.handle(&GameEvent::PlayerLogout(PlayerSession {
            timestamp: 200,
            world_id: 10,
            character_id: ME,
        }));
        let Some(TrackerOutput::SessionEnded(s)) = o.last() else {
            panic!("session not ended")
        };
        assert_eq!(s.stats.kills, 2);
        assert_eq!(s.ended_at, Some(200));
        assert!(t.session(ME).is_none());
    }

    #[test]
    fn auto_start_on_first_event_and_idle_end() {
        let mut t = tracker();
        let o = t.handle(&kill(1000));
        assert_eq!(o[0], TrackerOutput::SessionStarted(ME));
        assert!(t.tick(1000 + 1799).is_empty());
        let o = t.tick(1000 + 1801);
        assert!(matches!(&o[0], TrackerOutput::SessionEnded(s) if s.ended_at == Some(1000)));
    }

    #[test]
    fn ignores_untracked() {
        let mut t = tracker();
        let mut ev = kill(1);
        if let GameEvent::Death(ref mut d) = ev {
            d.attacker_character_id = 1;
        }
        assert!(t.handle(&ev).is_empty());
    }

    #[test]
    fn snapshot_uses_names_and_string_ids() {
        let mut t = tracker();
        t.handle(&kill(100));
        let mut names = NameCache::default();
        names.items.insert(80, "Gauss SAW".into());
        names.characters.insert(99, ("Victim".into(), 1));
        let mut lt = LifetimeStats {
            kills: 100,
            deaths: 100,
            play_time_secs: 6000,
            ..Default::default()
        };
        lt.weapon_kills = 10;
        lt.weapon_headshots = 2;
        let s = t.session(ME).unwrap();
        let snap = snapshot(s, true, 160, &names, Some(&lt), &[]);
        assert_eq!(snap.character_id, "5428000000000000042");
        assert_eq!(snap.killfeed[0].other_name, "Victim");
        assert_eq!(snap.weapons[0].name, "Gauss SAW");
        let c = snap.comparison.unwrap();
        assert_eq!(c.kd.delta_pct, Some(0.0));
        let json = serde_json::to_string(&snap.killfeed[0]).unwrap();
        assert!(json.contains("\"other_id\":\"99\""));
        assert_eq!(snap.killfeed[0].other_label, "");
    }

    #[test]
    fn feed_rows_explain_missing_names() {
        let mut names = NameCache::default();
        let row = |o: Outcome, n: &NameCache| {
            feed_row(
                &FeedEntry {
                    timestamp: 1,
                    outcome: o,
                    team: None,
                    other_loadout: None,
                    my_loadout: None,
                },
                ME,
                n,
            )
            .unwrap()
        };
        let death = |killer| Outcome::Death {
            killer,
            weapon_id: 0,
            vehicle_id: 0,
            headshot: false,
        };
        assert_eq!(row(death(0), &names).other_label, "environment");
        assert_eq!(row(death(77), &names).other_label, "pending");
        assert_eq!(
            row(Outcome::Suicide { weapon_id: 1 }, &names).other_label,
            "self"
        );
        names.record_characters(&[77], HashMap::new(), 10);
        assert_eq!(row(death(77), &names).other_label, "unknown");
        names.record_characters(&[77], HashMap::from([(77, ("Late".into(), 2))]), 20);
        let r = row(death(77), &names);
        assert_eq!(
            (r.other_name.as_str(), r.other_label.as_str()),
            ("Late", "")
        );
    }
}
