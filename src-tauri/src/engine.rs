//! Connects ESS, tracker, persistence, name resolution, sounds and UI.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ps2_tracker_core::achievements::RuleSet;
use ps2_tracker_core::ess::{self, EssConfig, EssUpdate};
use ps2_tracker_core::events::GameEvent;
use ps2_tracker_core::recursion_mapping;
use ps2_tracker_core::tracker::{self, TrackedCharacter, TrackerOutput};
use ps2_tracker_core::triggers::{TriggerEngine, TriggerSet};
use ps2_tracker_core::voice::{DetectorConfig, Selection, Trigger, VoiceDetector};
use ps2_tracker_core::voicepack;
use ps2_tracker_core::{CharacterId, SessionSnapshot};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, watch};

use crate::state::{AppState, EssStatus, lock, now, now_ms};

pub fn state(app: &AppHandle) -> Arc<AppState> {
    app.state::<Arc<AppState>>().inner().clone()
}

/// Rule set: custom file if present and valid, otherwise the default.
pub fn load_rules(st: &AppState) -> RuleSet {
    let p = st.paths.achievements_file();
    match std::fs::read_to_string(&p) {
        Ok(t) => RuleSet::parse(&t).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "custom achievement rule set invalid, using default");
            RuleSet::default_rules()
        }),
        Err(_) => RuleSet::default_rules(),
    }
}

pub fn reload_voice_packs(st: &AppState) -> Vec<(String, String)> {
    let (packs, bad) = voicepack::list_packs(&st.paths.voicepacks_dir());
    let active = st.cfg().voice.active_packs.clone();
    let ordered: Vec<voicepack::VoicePack> = active
        .iter()
        .filter_map(|id| packs.iter().find(|p| &p.id == id).cloned())
        .collect();
    // first active pack (highest rank) with a valid background image
    let background = ordered.iter().find_map(|p| {
        use base64::Engine as _;
        let (path, width, height) = ps2_tracker_core::recursion_pak::background_of(&p.dir)?;
        let bytes = std::fs::read(path).ok()?;
        Some(crate::state::PackBackground {
            pack: p.manifest.pack.name.clone(),
            data_url: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ),
            width,
            height,
        })
    });
    let mut v = lock(&st.voice);
    v.packs = ordered;
    v.background = background;
    bad
}

fn set_status(app: &AppHandle, f: impl FnOnce(&mut EssStatus)) {
    let st = state(app);
    let s = {
        let mut g = lock(&st.status);
        f(&mut g);
        g.clone()
    };
    let _ = app.emit("status", s);
}

/// Fills in missing character IDs via Census and hands the characters to the tracker.
pub async fn sync_characters(app: &AppHandle) {
    let st = state(app);
    let census = st.census();
    let mut cfg = st.cfg().clone();
    let mut changed = false;
    for c in cfg
        .characters
        .iter_mut()
        .chain(cfg.squad.iter_mut())
        .filter(|c| c.enabled && c.id.is_none())
    {
        match census.character_by_name(&c.name).await {
            Ok(info) => {
                c.id = Some(info.character_id);
                c.faction_id = Some(info.faction_id);
                c.name = info.name;
                changed = true;
            }
            Err(e) => {
                tracing::warn!(name = %c.name, error = %e, "character not found");
                set_status(app, |s| s.detail = format!("Character {}: {e}", c.name));
            }
        }
    }
    if changed {
        if let Err(e) = cfg.save(&st.paths.config_file()) {
            tracing::warn!(error = %e, "configuration not saved");
        }
        *st.config.write().unwrap_or_else(|e| e.into_inner()) = cfg.clone();
        let _ = app.emit("config-changed", ());
    }
    let mut chars: HashMap<CharacterId, TrackedCharacter> = HashMap::new();
    // squad first, so an own character listed in both stays "own"
    for (list, own) in [(&cfg.squad, false), (&cfg.characters, true)] {
        for c in list.iter().filter(|c| c.enabled) {
            if let Some(id) = c.id {
                chars.insert(
                    id,
                    TrackedCharacter {
                        name: c.name.clone(),
                        faction_id: c.faction_id,
                        own,
                    },
                );
            }
        }
    }
    lock(&st.tracker).set_characters(chars);
}

/// IDs of all enabled own and squad characters (sorted, unique).
fn tracked_ids(cfg: &ps2_tracker_core::Config) -> Vec<CharacterId> {
    let mut ids: Vec<CharacterId> = cfg
        .characters
        .iter()
        .chain(&cfg.squad)
        .filter(|c| c.enabled)
        .filter_map(|c| c.id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// After own or squad characters changed: the running connection subscribes
/// the new ones and clears the removed ones (no reconnect); without a running
/// connection it is started. Then the windows and OBS get a fresh snapshot,
/// so the overlay ("Squad top 3") shows the change at once.
pub fn update_ess_characters(app: &AppHandle) {
    let st = state(app);
    let ids = tracked_ids(&st.cfg());
    let live = {
        let g = lock(&st.ess_characters);
        match g.as_ref() {
            Some(tx) if !tx.is_closed() && !ids.is_empty() => {
                tx.send_if_modified(|cur| {
                    if *cur == ids {
                        false
                    } else {
                        cur.clone_from(&ids);
                        true
                    }
                });
                true
            }
            _ => false,
        }
    };
    if !live {
        restart_ess(app);
    }
    emit_snapshot(app);
}

/// (Re)starts the ESS connection with the current characters.
pub fn restart_ess(app: &AppHandle) {
    let st = state(app);
    if let Some(stop) = lock(&st.ess_stop).take() {
        let _ = stop.send(true);
    }
    *lock(&st.ess_characters) = None;
    let cfg = st.cfg().clone();
    let ids = tracked_ids(&cfg);
    if ids.is_empty() {
        set_status(app, |s| {
            s.state = "idle".into();
            s.detail = "No character configured".into();
            s.retry_in_secs = None;
        });
        return;
    }
    let (stop_tx, stop_rx) = watch::channel(false);
    *lock(&st.ess_stop) = Some(stop_tx);
    let (tx, mut rx) = mpsc::channel(512);
    let (chars_tx, chars_rx) = watch::channel(ids.clone());
    *lock(&st.ess_characters) = Some(chars_tx);
    // validated on load and on save; "example" only as a last resort
    let sid = cfg
        .census
        .resolve()
        .map(|s| s.expose().to_owned())
        .unwrap_or_else(|_| ps2_tracker_core::service_id::EXAMPLE.to_owned());
    let ess_cfg = EssConfig::new(sid, ids);
    set_status(app, |s| {
        s.state = "connecting".into();
        s.detail = String::new();
        s.retry_in_secs = None;
    });
    tauri::async_runtime::spawn(ess::run(ess_cfg, tx, stop_rx, chars_rx));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // the "last event" time in the status bar: at most once a second
        let mut status_sent = std::time::Instant::now() - Duration::from_secs(2);
        while let Some(u) = rx.recv().await {
            let mut next = Some(u);
            while let Some(u) = next.take() {
                let EssUpdate::Event(ev) = u else {
                    connection_update(&app, u);
                    continue;
                };
                // a burst (fights with a squad, many XP events): handle what
                // is already waiting, then refresh the windows once. Before,
                // every event sent the full snapshot and the status to every
                // window; at a few dozen events per second the reader fell
                // behind and stopped answering pings (seen in the long-session
                // test: the stream closed the connection).
                let mut last = ev.timestamp();
                handle_event_quiet(&app, &ev);
                for _ in 0..BURST_MAX {
                    match rx.try_recv() {
                        Ok(EssUpdate::Event(ev)) => {
                            last = ev.timestamp();
                            handle_event_quiet(&app, &ev);
                        }
                        Ok(other) => {
                            next = Some(other);
                            break;
                        }
                        Err(_) => break,
                    }
                }
                emit_snapshot(&app);
                lock(&state(&app).status).last_event_at = Some(last);
                if status_sent.elapsed() >= Duration::from_secs(1) {
                    status_sent = std::time::Instant::now();
                    set_status(&app, |_| {});
                }
            }
        }
    });
}

/// Events handled in one go before the windows are refreshed.
const BURST_MAX: usize = 256;

fn connection_update(app: &AppHandle, u: EssUpdate) {
    match u {
        EssUpdate::Connected => set_status(app, |s| {
            s.state = "connected".into();
            s.detail = String::new();
            s.retry_in_secs = None;
        }),
        EssUpdate::Disconnected { reason, retry_in } => set_status(app, |s| {
            s.state = "disconnected".into();
            s.detail = reason;
            s.retry_in_secs = Some(retry_in.as_secs());
        }),
        EssUpdate::Event(_) => {}
    }
}

#[derive(Serialize, Clone)]
pub struct Toast {
    pub title: String,
    pub text: String,
    pub kind: String,
}

/// Shows a toast in the overlay window and/or the OBS source.
pub fn show_toast(app: &AppHandle, t: &Toast, overlay: bool, stream: bool) {
    if overlay {
        let _ = app.emit_to(crate::windows::POPUP, "toast", t.clone());
    }
    if stream && let Ok(json) = serde_json::to_string(t) {
        let _ = state(app).obs_tx.send(("toast".into(), json));
    }
}

fn detector_cfg(st: &AppState) -> DetectorConfig {
    let c = st.cfg();
    DetectorConfig {
        multikill_window_secs: c.voice.multikill_window_secs,
    }
}

/// Trigger rules: custom file if present and valid, otherwise the default.
/// Rules from triggers.toml (if present) or the built-in defaults, without the
/// conditions edited in the UI.
pub fn base_triggers(st: &AppState) -> TriggerSet {
    let p = st.paths.config_dir.join("triggers.toml");
    match std::fs::read_to_string(&p) {
        Ok(t) => TriggerSet::parse(&t).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "custom triggers.toml invalid, using default");
            TriggerSet::default_set()
        }),
        Err(_) => TriggerSet::default_set(),
    }
}

/// Effective rules: base rules with the conditions edited on the Events page.
pub fn load_triggers(st: &AppState) -> TriggerSet {
    let base = base_triggers(st);
    let o = st.cfg().voice.trigger_overrides.clone();
    base.with_overrides(&o).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "edited trigger conditions invalid, ignored");
        base
    })
}

/// One event: tracker, sounds, crosshair, names – without refreshing the
/// windows (the caller does that once per burst).
fn handle_event_quiet(app: &AppHandle, ev: &GameEvent) {
    let st = state(app);
    let prev_killers: HashMap<CharacterId, Option<CharacterId>> = {
        let t = lock(&st.tracker);
        t.active_sessions()
            .map(|s| (s.character_id, s.last_killer))
            .collect()
    };
    crosshair_feedback(app, &st, ev);
    let outputs = lock(&st.tracker).handle(ev);
    process_outputs(app, outputs, ev.timestamp(), &prev_killers);
    request_names(app, ev);
}

/// Payload of the `crosshair-fx` event for the crosshair window.
#[derive(Serialize, Clone)]
pub struct CrosshairFx {
    pub kind: &'static str,
    pub effect: ps2_tracker_core::hitfx::FxEffect,
    /// Swap mode: the replacement PNG as data URL.
    pub data_url: Option<String>,
}

/// Hit feedback in the crosshair (kill, headshot, assist …), sent before the
/// rest of the event handling so it reaches the window as early as possible.
fn crosshair_feedback(app: &AppHandle, st: &AppState, ev: &GameEvent) {
    let cfg = st.cfg().crosshair.clone();
    if !cfg.enabled || !cfg.hit_feedback.enabled {
        return;
    }
    let ids: Vec<CharacterId> = lock(&st.tracker).own_ids().collect();
    let Some((kind, fx)) = ids
        .iter()
        .find_map(|me| cfg.hit_feedback.for_event(ev, *me))
    else {
        return;
    };
    send_crosshair_fx(app, st, kind, fx.clone());
    // ESS timestamps have a resolution of one second, so this is a rough value
    let delay_ms = (now_ms() as i64 - ev.timestamp() * 1000).max(0);
    tracing::info!(
        kind = kind.name(),
        delay_ms,
        "crosshair feedback (delay since the ESS timestamp, 1 s resolution)"
    );
}

pub fn send_crosshair_fx(
    app: &AppHandle,
    st: &AppState,
    kind: ps2_tracker_core::hitfx::FxKind,
    effect: ps2_tracker_core::hitfx::FxEffect,
) {
    let data_url = (effect.mode == "swap")
        .then(|| crate::windows::crosshair_data_url(st, &effect.file))
        .flatten();
    let _ = app.emit_to(
        crate::windows::CROSSHAIR,
        "crosshair-fx",
        CrosshairFx {
            kind: kind.name(),
            effect,
            data_url,
        },
    );
}

pub fn process_outputs(
    app: &AppHandle,
    outputs: Vec<TrackerOutput>,
    ts: i64,
    prev_killers: &HashMap<CharacterId, Option<CharacterId>>,
) {
    let st = state(app);
    let dcfg = detector_cfg(&st);
    let mut facts: Vec<(CharacterId, Trigger)> = Vec::new();
    let mut per_char: HashMap<CharacterId, Vec<ps2_tracker_core::Outcome>> = HashMap::new();
    // sounds, toasts and lifetime stats only for the user's own characters;
    // squad members are tracked and stored silently
    let own: std::collections::HashSet<CharacterId> = lock(&st.tracker).own_ids().collect();
    for o in outputs {
        match o {
            TrackerOutput::SessionStarted(id) if !own.contains(&id) => {
                if let Some(s) = lock(&st.tracker).session_mut(id)
                    && let Err(e) = st.store.save_session(s)
                {
                    tracing::warn!(error = %e, "session not saved");
                }
                let _ = app.emit("sessions-changed", ());
            }
            TrackerOutput::Outcome { character_id, .. }
            | TrackerOutput::Achievement { character_id, .. }
                if !own.contains(&character_id) => {}
            TrackerOutput::SessionStarted(id) => {
                {
                    let mut v = lock(&st.voice);
                    v.detectors.insert(id, VoiceDetector::new(dcfg));
                    v.engines.insert(id, TriggerEngine::default());
                }
                facts.push((id, Trigger::new("session_start")));
                if let Some(s) = lock(&st.tracker).session_mut(id)
                    && let Err(e) = st.store.save_session(s)
                {
                    tracing::warn!(error = %e, "session not saved");
                }
                fetch_lifetime(app, id);
                let _ = app.emit("sessions-changed", ());
            }
            TrackerOutput::Outcome {
                character_id,
                outcome,
                ..
            } => per_char.entry(character_id).or_default().push(outcome),
            TrackerOutput::Achievement {
                character_id,
                unlocked,
            } => {
                facts.push((character_id, Trigger::new("achievement")));
                let t = Toast {
                    title: unlocked.name.clone(),
                    text: unlocked.description.clone(),
                    kind: "achievement".into(),
                };
                show_toast(app, &t, true, true);
            }
            TrackerOutput::SessionPaused(id) => {
                if let Some(s) = lock(&st.tracker).session_mut(id)
                    && let Err(e) = st.store.save_session(s)
                {
                    tracing::warn!(error = %e, "session not saved");
                }
                if own.contains(&id) {
                    let mins = st.cfg().session.resume_minutes;
                    let t = Toast {
                        title: "Logged out – session paused".into(),
                        text: format!("Log back in within {mins} min and the session goes on."),
                        kind: "session".into(),
                    };
                    show_toast(app, &t, true, false);
                }
                let _ = app.emit("sessions-changed", ());
            }
            TrackerOutput::SessionResumed(id) => {
                if own.contains(&id) {
                    let t = Toast {
                        title: "Session resumed".into(),
                        text: "Back in game – the stats continue.".into(),
                        kind: "session".into(),
                    };
                    show_toast(app, &t, true, false);
                }
                let _ = app.emit("sessions-changed", ());
            }
            TrackerOutput::SessionEnded(mut s) => {
                {
                    let mut v = lock(&st.voice);
                    v.detectors.remove(&s.character_id);
                    v.engines.remove(&s.character_id);
                }
                if let Err(e) = st.store.save_session(&mut s) {
                    tracing::warn!(error = %e, "session not saved");
                }
                let _ = app.emit("sessions-changed", ());
            }
        }
    }
    {
        let mut v = lock(&st.voice);
        let mut ordered = Vec::new();
        for (id, outcomes) in per_char {
            let det = v
                .detectors
                .entry(id)
                .or_insert_with(|| VoiceDetector::new(dcfg));
            let prev = prev_killers.get(&id).copied().flatten();
            ordered.extend(
                det.on_outcomes(&outcomes, ts, prev)
                    .into_iter()
                    .map(|t| (id, t)),
            );
        }
        ordered.append(&mut facts);
        facts = ordered;
    }
    play_facts(app, &st, facts);
}

/// All sound events of one fact: trigger rules in the given kill chain style,
/// plus the user's "every kill" key.
pub fn sound_events(
    eng: &mut ps2_tracker_core::triggers::TriggerEngine,
    triggers: &ps2_tracker_core::triggers::TriggerSet,
    fact: &Trigger,
    cfg: &ps2_tracker_core::config::VoiceSettings,
    style: ps2_tracker_core::triggers::ChainStyle,
) -> Vec<(Trigger, u8)> {
    let mut events = eng.evaluate_style(triggers, fact, style);
    // user choice for packs without PER_KILL: this key on every kill
    if fact.event == "kill" && voicepack::is_recursion_style_key(&cfg.kill_key) {
        events.push((
            Trigger {
                event: cfg.kill_key.clone(),
                ..fact.clone()
            },
            10,
        ));
    }
    events
}

/// Evaluates the facts through the trigger rules and plays only the most important
/// sound per batch (= one ESS event); the queue handles the rest.
fn play_facts(app: &AppHandle, st: &AppState, mut facts: Vec<(CharacterId, Trigger)>) {
    let cfg = st.cfg().voice.clone();
    if facts.is_empty() {
        return;
    }
    {
        let names = st.names();
        let cats = lock(&st.item_categories);
        for (_, t) in &mut facts {
            if t.weapon_id != 0 {
                t.weapon_category = cats.get(&t.weapon_id).cloned();
            }
            if t.vehicle_id != 0 {
                t.vehicle = names.vehicles.get(&t.vehicle_id).cloned();
            }
        }
    }
    let mut v = lock(&st.voice);
    let packs = ps2_tracker_core::voice::effective_packs(&v.packs, cfg.builtin_fallback);
    let triggers = v.triggers.clone();
    let now = now_ms();
    let style = ps2_tracker_core::voice::chain_style(&cfg.chain_style, &packs);
    let mut events = Vec::new();
    for (id, fact) in &facts {
        let eng = v.engines.entry(*id).or_default();
        events.extend(sound_events(eng, &triggers, fact, &cfg, style));
    }
    let base = facts
        .first()
        .map(|(_, f)| f.event.clone())
        .unwrap_or_default();
    let choice = ps2_tracker_core::voice::choose(
        &mut v.selector,
        &events,
        &cfg.overrides,
        &packs,
        &cfg.muted,
        now,
    );
    if !choice.tried.is_empty() {
        tracing::info!(
            "voice: {base}: tried {} (chain style {})",
            choice.tried.join(" "),
            style.name()
        );
    } else {
        // every kill/death leaves a line, also when no rule produced an event
        tracing::info!(
            "voice: {base}: no sound event ({} candidates, chain style {})",
            events.len(),
            style.name()
        );
    }
    let best: Option<Selection> = choice.best;
    let display: Option<Selection> = choice.display;
    drop(v);
    match best {
        Some(b) if cfg.enabled => {
            if let Some(f) = b.file {
                tracing::info!(event = %b.event, pack = %b.pack, file = %f.display(), priority = b.priority, "voice: play");
                lock(&st.last_played).insert(b.event.clone(), crate::state::now());
                st.audio.play(f, b.volume, b.priority);
            }
        }
        Some(b) => tracing::debug!(event = %b.event, "voice: sounds disabled in settings"),
        None if !events.is_empty() => tracing::debug!("voice: nothing to play for this event"),
        None => {}
    }
    if let Some(d) = display {
        let t = Toast {
            title: recursion_mapping::label(&d.event),
            text: String::new(),
            kind: "voice".into(),
        };
        show_toast(app, &t, d.overlay, d.stream);
    }
}

/// Fetches a character's lifetime stats (once per session).
pub fn fetch_lifetime(app: &AppHandle, id: CharacterId) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let st = state(&app);
        match st.census().lifetime(id).await {
            Ok(l) => {
                let weapons: Vec<u32> = l.weapons.keys().copied().collect();
                lock(&st.lifetime).insert(id, l);
                emit_snapshot(&app);
                resolve(&app, weapons, vec![]).await;
            }
            Err(e) => tracing::warn!(error = %e, "lifetime stats not loaded"),
        }
    });
}

fn request_names(app: &AppHandle, ev: &GameEvent) {
    let (items, chars) = match ev {
        GameEvent::Death(d) => (
            vec![d.attacker_weapon_id],
            vec![d.character_id, d.attacker_character_id],
        ),
        GameEvent::VehicleDestroy(v) => (
            vec![v.attacker_weapon_id],
            vec![v.character_id, v.attacker_character_id],
        ),
        _ => return,
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move { resolve(&app, items, chars).await });
}

/// Resolves missing names via Census, caches them and refreshes the display.
pub async fn resolve(app: &AppHandle, items: Vec<u32>, chars: Vec<CharacterId>) {
    let st = state(app);
    let (mi, mc) = {
        let n = st.names();
        (n.missing_items(items), n.characters_to_fetch(chars, now()))
    };
    if mi.is_empty() && mc.is_empty() {
        return;
    }
    let census = st.census();
    let mut changed = false;
    if !mi.is_empty() {
        match census.item_names_with_category(&mi).await {
            Ok(found) => {
                let ts = now();
                let entries: Vec<(String, String, Option<i64>)> = found
                    .iter()
                    .map(|(id, (n, cat))| (id.to_string(), n.clone(), cat.map(i64::from)))
                    .collect();
                let _ = st.store.save_names(
                    "item",
                    entries.iter().map(|(a, b, c)| (a.clone(), b.as_str(), *c)),
                    ts,
                );
                {
                    let mut names = st.names();
                    for (id, (n, _)) in &found {
                        names.items.insert(*id, n.clone());
                    }
                }
                fill_categories(&st, &found).await;
                changed = true;
            }
            Err(e) => tracing::debug!(error = %e, "weapon names not loaded"),
        }
    }
    if !mc.is_empty() {
        match census.character_names(&mc).await {
            Ok(found) => {
                let ts = now();
                let entries: Vec<(String, String, Option<i64>)> = found
                    .iter()
                    .map(|(id, (n, f))| (id.to_string(), n.clone(), Some(i64::from(*f))))
                    .collect();
                let _ = st.store.save_names(
                    "character",
                    entries.iter().map(|(a, b, c)| (a.clone(), b.as_str(), *c)),
                    ts,
                );
                st.names().record_characters(&mc, found, ts);
                changed = true;
            }
            Err(e) => tracing::debug!(error = %e, "character names not loaded"),
        }
    }
    if changed {
        emit_snapshot(app);
    }
}

async fn fill_categories(st: &AppState, found: &HashMap<u32, (String, Option<u32>)>) {
    ensure_category_names(st).await;
    let names = lock(&st.category_names).clone();
    let mut cats = lock(&st.item_categories);
    for (id, (_, cat)) in found {
        if let Some(n) = cat.and_then(|c| names.get(&c)) {
            cats.insert(*id, n.clone());
        }
    }
}

async fn ensure_category_names(st: &AppState) {
    if !lock(&st.category_names).is_empty() {
        return;
    }
    match st.census().item_categories().await {
        Ok(m) => *lock(&st.category_names) = m,
        Err(e) => tracing::debug!(error = %e, "weapon categories not loaded"),
    }
}

/// Startup: load name cache, vehicle names and weapon categories.
pub async fn warm_up(app: &AppHandle) {
    let st = state(app);
    if st.names().vehicles.is_empty() {
        match st.census().vehicle_names().await {
            Ok(v) => {
                let ts = now();
                let entries: Vec<(String, String)> = v
                    .iter()
                    .map(|(id, n)| (id.to_string(), n.clone()))
                    .collect();
                let _ = st.store.save_names(
                    "vehicle",
                    entries.iter().map(|(a, b)| (a.clone(), b.as_str(), None)),
                    ts,
                );
                st.names().vehicles.extend(v);
            }
            Err(e) => tracing::warn!(error = %e, "vehicle names not loaded"),
        }
    }
    ensure_category_names(&st).await;
    let names = lock(&st.category_names).clone();
    if let Ok(ids) = st.store.item_category_ids() {
        let mut cats = lock(&st.item_categories);
        for (item, cat) in ids {
            if let Some(n) = names.get(&cat) {
                cats.insert(item, n.clone());
            }
        }
    }
}

pub fn current_snapshot(st: &AppState) -> Option<SessionSnapshot> {
    // config first: never hold the tracker lock while waiting for the config
    let squad = {
        let o = &st.cfg().overlay;
        o.show_squad.then(|| {
            (
                tracker::SquadMetric::parse(&o.squad_metric),
                o.squad_min_kills,
            )
        })
    };
    let t = lock(&st.tracker);
    let id = t.focus()?;
    let s = t.session(id)?;
    let names = st.names();
    let lt = lock(&st.lifetime);
    let mut snap = tracker::snapshot(s, true, now(), &names, lt.get(&id), t.achievements(id));
    if let Some((metric, min_kills)) = squad {
        snap.squad_top = tracker::squad_top(&t, metric, min_kills, now());
    }
    Some(snap)
}

pub fn emit_snapshot(app: &AppHandle) {
    let st = state(app);
    let snap = current_snapshot(&st);
    if let Ok(json) = serde_json::to_string(&snap) {
        let _ = st.obs_tx.send(("snapshot".into(), json));
    }
    let _ = app.emit("snapshot", snap);
}

/// Periodic: idle timeout, autosave, refresh the display.
pub fn spawn_ticker(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut n: u64 = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            n += 1;
            let st = state(&app);
            let outputs = lock(&st.tracker).tick(now());
            if !outputs.is_empty() {
                process_outputs(&app, outputs, now(), &HashMap::new());
            }
            if n.is_multiple_of(12) {
                let mut t = lock(&st.tracker);
                let ids: Vec<CharacterId> = t.active_sessions().map(|s| s.character_id).collect();
                for id in ids {
                    if let Some(s) = t.session_mut(id) {
                        let _ = st.store.save_session(s);
                    }
                }
            }
            if n.is_multiple_of(6) {
                resolve_feed_names(&app);
                resolve_locations(&app);
            }
            emit_snapshot(&app);
        }
    });
}

/// Retries names the kill feed still shows as missing (a failed or rate-limited
/// lookup, or a session restored after a restart). Characters Census does not
/// know are only asked for again after `NOT_FOUND_RETRY`.
fn resolve_feed_names(app: &AppHandle) {
    let st = state(app);
    let (items, chars) = {
        let t = lock(&st.tracker);
        let Some(s) = t.focus().and_then(|id| t.session(id)) else {
            return;
        };
        let mut items = Vec::new();
        let mut chars = Vec::new();
        for e in s.killfeed() {
            if let Some((c, w)) = e.outcome.other_and_weapon() {
                chars.push(c);
                items.push(w);
            }
        }
        (items, chars)
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move { resolve(&app, items, chars).await });
}

/// Names of the servers and continents the running sessions are on (Census
/// `world`/`zone`, only IDs that occur; cached in the database). Each ID is
/// asked for once per run after a successful answer.
fn resolve_locations(app: &AppHandle) {
    let st = state(app);
    let ids: Vec<(Option<u32>, Option<u32>)> = lock(&st.tracker)
        .active_sessions()
        .map(|s| (s.world_id, s.zone_id))
        .collect();
    let (worlds, zones) = st.names().locations_to_fetch(ids);
    if worlds.is_empty() && zones.is_empty() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let st = state(&app);
        let census = st.census();
        let mut changed = false;
        for (is_zone, ids) in [(false, worlds), (true, zones)] {
            if ids.is_empty() {
                continue;
            }
            let res = if is_zone {
                census.zone_names(&ids).await
            } else {
                census.world_names(&ids).await
            };
            match res {
                Ok(found) => {
                    let kind = if is_zone { "zone" } else { "world" };
                    let _ = st.store.save_names(
                        kind,
                        found
                            .iter()
                            .map(|(id, n)| (id.to_string(), n.as_str(), None)),
                        now(),
                    );
                    let mut names = st.names();
                    names
                        .asked_locations
                        .extend(ids.iter().map(|i| (is_zone, *i)));
                    if is_zone {
                        names.zones.extend(found);
                    } else {
                        names.worlds.extend(found);
                    }
                    changed = true;
                }
                Err(e) => tracing::debug!(error = %e, "server/continent names not loaded"),
            }
        }
        if changed {
            emit_snapshot(&app);
        }
    });
}

/// On exit: end and save running sessions.
pub fn shutdown(app: &AppHandle) {
    let st = state(app);
    let ids: Vec<CharacterId> = lock(&st.tracker)
        .active_sessions()
        .map(|s| s.character_id)
        .collect();
    for id in ids {
        if let Some(mut s) = lock(&st.tracker).stop(id, now()) {
            let _ = st.store.save_session(&mut s);
        }
    }
}
