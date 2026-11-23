//! Tauri commands for the UI. Errors are returned to the frontend as text.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use ps2_tracker_core::census::CharacterInfo;
use ps2_tracker_core::packloader::{self, PackFormat};
use ps2_tracker_core::sessionfile;
use ps2_tracker_core::store::{SessionSummary, export_csv, export_json};
use ps2_tracker_core::tracker::snapshot;
use ps2_tracker_core::triggers::TriggerRule;
use ps2_tracker_core::voice::Trigger;
use ps2_tracker_core::voicepack::{self, Manifest, MappingProposal, PackMeta, VoicePack};
use ps2_tracker_core::{CensusClient, CharacterId, Config, SessionSnapshot, crosshair};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::engine::{self, current_snapshot};
use crate::state::{AppState, EssStatus, lock, now};
use crate::{audio, focus, hotkeys, obs, windows};

type S<'a> = State<'a, Arc<AppState>>;
type R<T> = Result<T, String>;

fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

fn parse_id(id: &str) -> R<CharacterId> {
    id.parse()
        .map_err(|_| format!("invalid character ID: {id}"))
}

#[tauri::command]
pub fn get_config(st: S<'_>) -> Config {
    st.cfg().clone()
}

#[tauri::command]
pub async fn save_config(app: AppHandle, config: Config) -> R<()> {
    apply_config(&app, config).await
}

/// Validates, saves and applies a new configuration (also used by the
/// debug-only CLI `--debug-squad-add` of the end-to-end test).
pub async fn apply_config(app: &AppHandle, config: Config) -> R<()> {
    let st = engine::state(app);
    let app = app.clone();
    config.validate().map_err(e)?;
    let old = st.cfg().clone();
    let sid = config.census.resolve().map_err(e)?;
    config.save(&st.paths.config_file()).map_err(e)?;
    *st.config.write().unwrap_or_else(|x| x.into_inner()) = config.clone();
    if old.census.service_id != config.census.service_id {
        tracing::info!(source = sid.source().as_str(), "Census service ID changed");
        *st.census.write().unwrap_or_else(|x| x.into_inner()) =
            CensusClient::new(sid.expose()).map_err(e)?;
    }
    if old.voice.output_device != config.voice.output_device
        || old.voice.volume != config.voice.volume
        || old.voice.max_concurrent != config.voice.max_concurrent
    {
        st.audio.configure(
            config.voice.output_device.clone(),
            config.voice.volume,
            config.voice.max_concurrent,
        );
    }
    if old.session.resume_minutes != config.session.resume_minutes {
        lock(&st.tracker).set_resume_grace(i64::from(config.session.resume_minutes) * 60);
    }
    if old.voice.active_packs != config.voice.active_packs {
        engine::reload_voice_packs(&st);
    }
    if old.voice.trigger_overrides != config.voice.trigger_overrides {
        lock(&st.voice).triggers = engine::load_triggers(&st);
    }
    if old.census != config.census {
        engine::sync_characters(&app).await;
        engine::restart_ess(&app);
    } else if old.characters != config.characters || old.squad != config.squad {
        // live: subscribe/clear on the open connection, fresh snapshot
        engine::sync_characters(&app).await;
        engine::update_ess_characters(&app);
    }
    if old.obs != config.obs {
        obs::restart(&app);
    }
    hotkeys::register(&app);
    windows::apply_overlay(&app, "config saved").map_err(e)?;
    windows::apply_crosshair(&app, "config saved").map_err(e)?;
    let _ = app.emit("config-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn lookup_character(st: S<'_>, name: String) -> R<CharacterInfo> {
    st.census().character_by_name(&name).await.map_err(e)
}

#[tauri::command]
pub fn get_status(st: S<'_>) -> EssStatus {
    lock(&st.status).clone()
}

#[tauri::command]
pub fn get_snapshot(st: S<'_>) -> Option<SessionSnapshot> {
    current_snapshot(&st)
}

#[tauri::command]
pub fn list_sessions(st: S<'_>, limit: u32, offset: u32) -> R<Vec<SessionSummary>> {
    st.store.list_sessions(limit.min(500), offset).map_err(e)
}

#[tauri::command]
pub fn get_session(st: S<'_>, id: i64) -> R<SessionSnapshot> {
    let cfg = st.cfg().session.session_config();
    let s = st.store.load_session(id, cfg).map_err(e)?;
    let names = st.names();
    Ok(snapshot(&s, s.ended_at.is_none(), now(), &names, None, &[]))
}

#[tauri::command]
pub fn delete_session(app: AppHandle, st: S<'_>, id: i64) -> R<()> {
    st.store.delete_session(id).map_err(e)?;
    let _ = app.emit("sessions-changed", ());
    Ok(())
}

#[tauri::command]
pub fn export_session(st: S<'_>, id: i64, format: String, path: PathBuf) -> R<()> {
    let s = st
        .store
        .load_session(id, st.cfg().session.session_config())
        .map_err(e)?;
    let names = st.names();
    let bytes = match format.as_str() {
        "json" => export_json(&s, &names).map_err(e)?.into_bytes(),
        "csv" => export_csv(&s, &names).map_err(e)?.into_bytes(),
        "xlsx" => {
            sessionfile::to_xlsx(&sessionfile::SessionFile::new(&s, &names), &names).map_err(e)?
        }
        _ => return Err("format must be json, csv or xlsx".into()),
    };
    std::fs::write(path, bytes).map_err(e)
}

#[derive(Serialize)]
pub struct ImportOutcome {
    id: i64,
    /// The session was already stored (same character and start time).
    duplicate: bool,
    character_name: String,
    started_at: i64,
}

/// Imports a session file chosen in the file dialog (.json or .xlsx exported
/// by the tracker). Checked before anything is stored; duplicates are skipped.
#[tauri::command]
pub fn import_session(app: AppHandle, st: S<'_>, path: PathBuf) -> R<ImportOutcome> {
    let f = sessionfile::read_file(&path).map_err(e)?;
    f.validate(now()).map_err(e)?;
    let mut s = f.session.clone();
    let cid = s.character_id.to_string();
    if let Some(id) = st.store.find_session(&cid, s.started_at).map_err(e)? {
        return Ok(ImportOutcome {
            id,
            duplicate: true,
            character_name: s.character_name,
            started_at: s.started_at,
        });
    }
    s.id = None;
    if s.ended_at.is_none() {
        s.end(s.last_event_at);
    }
    s.restore(st.cfg().session.session_config());
    let id = st.store.save_session(&mut s).map_err(e)?;
    // names from the file, without overwriting what the local cache knows
    {
        let mut names = st.names();
        let ts = now();
        let chars: Vec<(String, String, Option<i64>)> = f
            .names
            .characters
            .iter()
            .filter(|(k, _)| k.parse().is_ok_and(|k| !names.characters.contains_key(&k)))
            .map(|(k, v)| (k.clone(), v.clone(), None))
            .collect();
        let _ = st.store.save_names(
            "character",
            chars.iter().map(|(a, b, c)| (a.clone(), b.as_str(), *c)),
            ts,
        );
        for (k, v, _) in chars {
            if let Ok(k) = k.parse() {
                names.characters.insert(k, (v, 0));
            }
        }
        for (k, v) in &f.names.items {
            names.items.entry(*k).or_insert_with(|| v.clone());
        }
        for (k, v) in &f.names.vehicles {
            names.vehicles.entry(*k).or_insert_with(|| v.clone());
        }
    }
    tracing::info!(id, "session imported");
    let _ = app.emit("sessions-changed", ());
    Ok(ImportOutcome {
        id,
        duplicate: false,
        character_name: s.character_name,
        started_at: s.started_at,
    })
}

#[derive(Serialize)]
pub struct SquadRow {
    character_id: String,
    name: String,
    faction_id: Option<u32>,
    own: bool,
    /// A session is running (the character produced events recently).
    online: bool,
    kills: u32,
    deaths: u32,
    revives: u32,
    kd: f64,
    kpm: f64,
    hsr: f64,
    started_at: Option<i64>,
    /// Last server/continent of the running session (IDs and Census names).
    world_id: Option<u32>,
    zone_id: Option<u32>,
    world_name: Option<String>,
    zone_name: Option<String>,
    /// Current class (last loadout seen), if known.
    class: Option<ps2_tracker_core::classes::Class>,
}

/// Live values of every tracked character (own and squad) for the Squad page.
#[tauri::command]
pub fn squad_rows(st: S<'_>) -> Vec<SquadRow> {
    let names = st.names().clone_locations();
    let t = lock(&st.tracker);
    let mut rows: Vec<SquadRow> = t
        .character_ids()
        .filter_map(|id| {
            let c = t.character(id)?;
            let s = t.session(id);
            let r = s.map(|s| s.rates(now()));
            Some(SquadRow {
                character_id: id.to_string(),
                name: c.name.clone(),
                faction_id: c.faction_id,
                own: c.own,
                online: s.is_some(),
                kills: s.map_or(0, |s| s.stats.kills),
                deaths: s.map_or(0, |s| s.stats.deaths),
                revives: s.map_or(0, |s| s.stats.revives),
                kd: r.as_ref().map_or(0.0, |r| r.kd),
                kpm: r.as_ref().map_or(0.0, |r| r.kpm),
                hsr: r.as_ref().map_or(0.0, |r| r.hsr),
                started_at: s.map(|s| s.started_at),
                world_id: s.and_then(|s| s.world_id),
                zone_id: s.and_then(|s| s.zone_id),
                world_name: s
                    .and_then(|s| s.world_id)
                    .and_then(|i| names.0.get(&i).cloned()),
                zone_name: s
                    .and_then(|s| s.zone_id)
                    .and_then(|i| names.1.get(&i).cloned()),
                class: s.and_then(|s| ps2_tracker_core::classes::class_of_opt(s.loadout_id)),
            })
        })
        .collect();
    rows.sort_by(|a, b| b.own.cmp(&a.own).then(a.name.cmp(&b.name)));
    rows
}

#[derive(Serialize)]
pub struct OutfitResult {
    outfit: String,
    members: Vec<ps2_tracker_core::census::OutfitMember>,
}

/// Outfit members by tag or name (Census, read-only), for the squad import.
#[tauri::command]
pub async fn outfit_members(st: S<'_>, query: String) -> R<OutfitResult> {
    let census = st.census();
    let (outfit, members) = census.outfit_members(&query, 1000).await.map_err(e)?;
    Ok(OutfitResult { outfit, members })
}

/// Removes a squad member; optionally deletes all stored sessions of that
/// character.
#[tauri::command]
pub async fn squad_remove(
    app: AppHandle,
    st: S<'_>,
    character_id: String,
    delete_data: bool,
) -> R<usize> {
    let id = parse_id(&character_id)?;
    let mut cfg = st.cfg().clone();
    let before = cfg.squad.len();
    cfg.squad.retain(|c| c.id != Some(id));
    if cfg.squad.len() != before {
        cfg.save(&st.paths.config_file()).map_err(e)?;
        *st.config.write().unwrap_or_else(|x| x.into_inner()) = cfg;
        engine::sync_characters(&app).await;
        engine::update_ess_characters(&app);
        let _ = app.emit("config-changed", ());
    }
    let n = if delete_data {
        st.store.delete_sessions_of(&character_id).map_err(e)?
    } else {
        0
    };
    if n > 0 {
        let _ = app.emit("sessions-changed", ());
    }
    tracing::info!(deleted_sessions = n, "squad member removed");
    Ok(n)
}

#[tauri::command]
pub fn update_status(app: AppHandle) -> crate::update::UpdateStatus {
    crate::update::status(&app)
}

/// "Check now" (Settings → Updates): asks the update endpoint once.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> R<Option<crate::update::UpdateInfo>> {
    crate::update::check(&app).await
}

/// Installs the update found by the last check, after the user confirmed it.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> R<()> {
    crate::update::install(&app).await
}

/// Writes a chart image (PNG data URL from the Analytics page) to a path the
/// user picked in the save dialog.
#[tauri::command]
pub fn save_png(path: PathBuf, data_url: String) -> R<()> {
    use base64::Engine as _;
    if path
        .extension()
        .and_then(|x| x.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
        != Some("png")
    {
        return Err("only .png files".into());
    }
    let b64 = data_url
        .strip_prefix("data:image/png;base64,")
        .ok_or("not a PNG image")?;
    if b64.len() > 40 * 1024 * 1024 {
        return Err("image too large".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(e)?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("not a PNG image".into());
    }
    std::fs::write(path, bytes).map_err(e)
}

/// Evaluations for the Analytics page (at most 50 sessions at a time).
#[tauri::command]
pub fn session_analysis(st: S<'_>, ids: Vec<i64>) -> R<Vec<ps2_tracker_core::analysis::Analysis>> {
    if ids.len() > 50 {
        return Err("at most 50 sessions at a time".into());
    }
    let cfg = st.cfg().session.session_config();
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let s = st.store.load_session(id, cfg).map_err(e)?;
        let names = st.names();
        out.push(ps2_tracker_core::analysis::analyse(&s, &names));
    }
    Ok(out)
}

#[tauri::command]
pub fn start_session(app: AppHandle, st: S<'_>, character_id: String) -> R<()> {
    let id = parse_id(&character_id)?;
    let out = lock(&st.tracker).start_manual(id, now());
    if out.is_empty() {
        return Err("a session is already running or the character is not active".into());
    }
    engine::process_outputs(&app, out, now(), &Default::default());
    engine::emit_snapshot(&app);
    Ok(())
}

#[tauri::command]
pub fn stop_session(app: AppHandle, st: S<'_>, character_id: String) -> R<()> {
    let id = parse_id(&character_id)?;
    let mut s = lock(&st.tracker)
        .stop(id, now())
        .ok_or("no running session")?;
    st.store.save_session(&mut s).map_err(e)?;
    lock(&st.voice).detectors.remove(&id);
    let _ = app.emit("sessions-changed", ());
    engine::emit_snapshot(&app);
    Ok(())
}

#[derive(Serialize)]
pub struct OverlayState {
    edit: bool,
    /// Object selected in the move mode ("stats" or "popup").
    selected: &'static str,
    enabled: bool,
    /// Size step factor for the overlay's monitor (S/M/L/XL or auto).
    size_factor: f32,
}

#[tauri::command]
pub fn get_overlay_state(app: AppHandle, st: S<'_>) -> OverlayState {
    OverlayState {
        edit: st.overlay_edit.load(Ordering::Relaxed),
        selected: windows::move_selected(&app).name(),
        enabled: st.cfg().overlay.enabled,
        size_factor: windows::overlay_size_factor(&app),
    }
}

#[tauri::command]
pub fn overlay_set_edit(app: AppHandle, edit: bool) -> R<()> {
    windows::set_overlay_edit(&app, edit, "button").map_err(e)
}

/// From an overlay object: measured content size in physical pixels.
#[tauri::command]
pub fn overlay_content_size(app: AppHandle, object: String, width: u32, height: u32) -> R<()> {
    let obj = windows::Obj::parse(&object).ok_or("unknown overlay object")?;
    windows::set_content_size(&app, obj, width, height).map_err(e)
}

/// Move layer: pointer movement (throttled in the page) resets the idle timer.
#[tauri::command]
pub fn move_layer_activity(app: AppHandle) {
    windows::move_touch(&app);
}

#[derive(Serialize)]
pub struct HotkeyFailure {
    hotkey: String,
    action: &'static str,
    reason: String,
}

/// Version, commit and build date (About, bug reports).
#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
    commit: &'static str,
    build_date: &'static str,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        commit: env!("PS2_BUILD_COMMIT"),
        build_date: env!("PS2_BUILD_DATE"),
    }
}

/// Official class icons as data URLs (class → `data:image/png;base64,…`),
/// only those already in the cache. Missing ones are downloaded once per run
/// in the background; afterwards the event `class-icons` asks the windows to
/// call this again. Without network the UI keeps its own symbols.
#[tauri::command]
pub fn class_icons(app: AppHandle, st: S<'_>) -> std::collections::BTreeMap<String, String> {
    use base64::Engine as _;
    use ps2_tracker_core::class_icons as icons;
    static FETCHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let Some(dir) = icons::cache_dir() else {
        return Default::default();
    };
    if !icons::missing(&dir).is_empty() && !FETCHING.swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        let census = st.census();
        let dir = dir.clone();
        tauri::async_runtime::spawn(async move {
            let n = icons::fetch_missing(&census, &dir).await;
            tracing::info!(stored = n, "class icons from Census");
            if n > 0 {
                let _ = app.emit("class-icons", ());
            }
        });
    }
    ps2_tracker_core::classes::Class::ALL
        .into_iter()
        .filter_map(|c| {
            let b = icons::cached(&dir, c)?;
            let name = serde_json::to_value(c).ok()?.as_str()?.to_owned();
            Some((
                name,
                format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(b)
                ),
            ))
        })
        .collect()
}

/// Where the active Census service ID comes from: "environment", "config",
/// "builtin" or "example". Never the value itself.
#[tauri::command]
pub fn service_id_source(st: S<'_>) -> String {
    st.cfg()
        .census
        .resolve()
        .map(|s| s.source().as_str())
        .unwrap_or("environment")
        .to_owned()
}

/// Hotkeys that could not be registered (taken by the desktop, invalid).
#[tauri::command]
pub fn hotkey_status(st: S<'_>) -> Vec<HotkeyFailure> {
    st.hotkey_failures
        .lock()
        .unwrap_or_else(|x| x.into_inner())
        .iter()
        .map(|(h, a, r)| HotkeyFailure {
            hotkey: h.clone(),
            action: a,
            reason: r.clone(),
        })
        .collect()
}

/// Messages from the start (damaged database …); handed out once.
#[tauri::command]
pub fn startup_notices(st: S<'_>) -> Vec<crate::state::Notice> {
    std::mem::take(&mut *st.notices.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Move layer: what it draws (`None` when no move session is open).
#[tauri::command]
pub fn move_layer_state(app: AppHandle) -> Option<windows::MoveLayerState> {
    windows::move_layer_state(&app)
}

/// Move layer: end of a drag, absolute physical position of the object.
#[tauri::command]
pub fn move_layer_set(app: AppHandle, object: String, x: i32, y: i32) -> R<()> {
    let obj = windows::Obj::parse(&object).ok_or("unknown overlay object")?;
    windows::move_set(&app, obj, x, y);
    Ok(())
}

#[tauri::command]
pub fn move_layer_select(app: AppHandle, object: String) -> R<()> {
    let obj = windows::Obj::parse(&object).ok_or("unknown overlay object")?;
    windows::move_select(&app, obj);
    Ok(())
}

/// Move layer: "Save" (true) or "Cancel" (false).
#[tauri::command]
pub fn move_layer_end(app: AppHandle, save: bool) -> R<()> {
    windows::end_move(&app, save, "button").map_err(e)
}

#[tauri::command]
pub fn move_layer_reset(app: AppHandle, object: String) -> R<()> {
    let obj = windows::Obj::parse(&object).ok_or("unknown overlay object")?;
    windows::move_reset(&app, obj);
    Ok(())
}

/// Move layer: "Move to monitor…".
#[tauri::command]
pub fn move_layer_monitor(app: AppHandle, object: String, monitor: String) -> R<()> {
    let obj = windows::Obj::parse(&object).ok_or("unknown overlay object")?;
    windows::move_to_monitor(&app, obj, &monitor)
}

#[tauri::command]
pub fn list_monitors(app: AppHandle) -> Vec<windows::MonitorInfo> {
    windows::monitors(&app)
}

#[derive(Serialize)]
pub struct PathsInfo {
    config_file: String,
    achievements_file: String,
    database: String,
    voicepacks: String,
    crosshairs: String,
    log_file: String,
}

#[tauri::command]
pub fn get_paths(st: S<'_>) -> PathsInfo {
    let p = &st.paths;
    PathsInfo {
        config_file: p.config_file().display().to_string(),
        achievements_file: p.achievements_file().display().to_string(),
        database: p.database().display().to_string(),
        voicepacks: p.voicepacks_dir().display().to_string(),
        crosshairs: p.crosshairs_dir().display().to_string(),
        log_file: p.log_file().display().to_string(),
    }
}

// ------------------------------------------------------------------ Voice

#[derive(Serialize)]
pub struct PackInfo {
    id: String,
    meta: PackMeta,
    format: PackFormat,
    events: usize,
    files: usize,
    active: bool,
    position: Option<usize>,
    warnings: Vec<String>,
    /// Background image of the pack as data URL (preview on the pack card).
    background: Option<String>,
    /// Events this pack has a sound for.
    sounds: Vec<String>,
    /// No `PER_KILL` sound: proposal for "every kill" (`voice.kill_key`).
    suggested_kill_key: Option<String>,
}

#[derive(Serialize)]
pub struct EventInfo {
    key: String,
    label: String,
    /// Description of the trigger condition; `None` = no trigger yet.
    trigger: Option<String>,
    priority: u8,
    recursion: bool,
    /// Effective rules (with the conditions edited in the UI).
    rules: Vec<TriggerRule>,
    /// Rules from triggers.toml / the defaults, for "Reset".
    default_rules: Vec<TriggerRule>,
    /// Last time a sound of this event was played (Unix seconds, this run).
    last_played: Option<i64>,
}

#[derive(Serialize)]
pub struct VoiceOverview {
    packs: Vec<PackInfo>,
    invalid: Vec<(String, String)>,
    events: Vec<EventInfo>,
    triggers_file: String,
}

#[tauri::command]
pub fn voice_overview(st: S<'_>) -> VoiceOverview {
    let invalid = engine::reload_voice_packs(&st);
    let (packs, _) = voicepack::list_packs(&st.paths.voicepacks_dir());
    let active = st.cfg().voice.active_packs.clone();
    let triggers = lock(&st.voice).triggers.clone();
    let base = engine::base_triggers(&st);
    let played = lock(&st.last_played).clone();
    VoiceOverview {
        packs: packs
            .into_iter()
            .map(|p| PackInfo {
                format: packloader::detect(&p.dir).unwrap_or(PackFormat::Open),
                position: active.iter().position(|a| *a == p.id),
                active: active.contains(&p.id),
                events: p.manifest.events.len(),
                files: p.manifest.events.iter().map(|x| x.files.len()).sum(),
                sounds: p
                    .manifest
                    .events
                    .iter()
                    .filter(|x| !x.files.is_empty() && !x.silent)
                    .map(|x| x.event.clone())
                    .collect(),
                suggested_kill_key: p.manifest.suggested_kill_key(),
                background: ps2_tracker_core::recursion_pak::background_of(&p.dir).and_then(
                    |(path, _, _)| {
                        use base64::Engine as _;
                        let b = std::fs::read(path).ok()?;
                        Some(format!(
                            "data:image/png;base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(b)
                        ))
                    },
                ),
                warnings: p.warnings,
                meta: p.manifest.pack,
                id: p.id,
            })
            .collect(),
        invalid,
        events: voicepack::all_events()
            .into_iter()
            .map(|(key, label)| EventInfo {
                trigger: triggers.describe(&key),
                priority: triggers.priority(&key),
                recursion: voicepack::is_recursion_style_key(&key),
                rules: triggers.rules_for(&key),
                default_rules: base.rules_for(&key),
                last_played: played.get(&key).copied(),
                key,
                label,
            })
            .collect(),
        triggers_file: st
            .paths
            .config_dir
            .join("triggers.toml")
            .display()
            .to_string(),
    }
}

fn pack_dir(st: &AppState, id: &str) -> R<PathBuf> {
    let rel = voicepack::safe_relative(id).ok_or("invalid pack ID")?;
    if rel.components().count() != 1 {
        return Err("invalid pack ID".into());
    }
    Ok(st.paths.voicepacks_dir().join(rel))
}

#[tauri::command]
pub fn voice_get_pack(st: S<'_>, id: String) -> R<VoicePack> {
    packloader::load_any(&pack_dir(&st, &id)?)
        .map(|(_, p)| p)
        .map_err(e)
}

#[tauri::command]
pub fn voice_save_manifest(st: S<'_>, id: String, manifest: Manifest) -> R<()> {
    let dir = pack_dir(&st, &id)?;
    let pack = VoicePack {
        id,
        dir,
        manifest,
        warnings: vec![],
        stats: Default::default(),
    };
    // files must exist
    for f in pack.manifest.events.iter().flat_map(|x| &x.files) {
        let p = pack.file_path(f).ok_or("unsafe path")?;
        if !p.is_file() {
            return Err(format!("file missing: {f}"));
        }
    }
    pack.save().map_err(e)?;
    engine::reload_voice_packs(&st);
    Ok(())
}

#[tauri::command]
pub fn voice_new_pack(st: S<'_>, name: String, author: String, license: String) -> R<String> {
    let lib = st.paths.voicepacks_dir();
    let mut dir = lib.join(voicepack::slug(&name));
    let mut i = 2;
    while dir.exists() {
        dir = lib.join(format!("{}-{i}", voicepack::slug(&name)));
        i += 1;
    }
    std::fs::create_dir_all(dir.join("sounds")).map_err(e)?;
    let pack = VoicePack {
        id: dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        dir,
        manifest: Manifest {
            pack: PackMeta {
                source: String::new(),
                name,
                author,
                version: "1.0.0".into(),
                license,
                description: String::new(),
            },
            events: vec![],
        },
        warnings: vec![],
        stats: Default::default(),
    };
    pack.save().map_err(e)?;
    Ok(pack.id)
}

/// Copies a custom file into a pack and assigns it to an event.
#[tauri::command]
pub fn voice_add_file(st: S<'_>, id: String, event: String, source: PathBuf) -> R<()> {
    if !voicepack::is_known_event(&event) {
        return Err("unknown event".into());
    }
    let (_, mut pack) = packloader::load_any(&pack_dir(&st, &id)?).map_err(e)?;
    let ext = source
        .extension()
        .and_then(|x| x.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|x| voicepack::AUDIO_EXTENSIONS.contains(&x.as_str()))
        .ok_or("only ogg, mp3, wav or flac")?;
    let len = std::fs::metadata(&source).map_err(e)?.len();
    if len > voicepack::MAX_FILE_BYTES {
        return Err("file too large (max. 20 MB)".into());
    }
    let stem = voicepack::slug(&source.file_stem().unwrap_or_default().to_string_lossy());
    std::fs::create_dir_all(pack.dir.join("sounds")).map_err(e)?;
    let mut name = format!("sounds/{event}_{stem}.{ext}");
    let mut i = 2;
    while pack.dir.join(&name).exists() {
        name = format!("sounds/{event}_{stem}-{i}.{ext}");
        i += 1;
    }
    std::fs::copy(&source, pack.dir.join(&name)).map_err(e)?;
    match pack
        .manifest
        .events
        .iter_mut()
        .find(|x| x.event == event && x.when == Default::default())
    {
        Some(entry) => entry.files.push(name),
        None => pack
            .manifest
            .events
            .push(voicepack::PackEntry::new(&event, vec![name])),
    }
    pack.save().map_err(e)?;
    engine::reload_voice_packs(&st);
    Ok(())
}

#[derive(Serialize)]
pub struct ImportResult {
    id: String,
    name: String,
    format: PackFormat,
    warnings: Vec<String>,
    /// Detected variant ("recursion: SharpSerializer (binary)", empty = own format)
    source: String,
    events: usize,
    stats: voicepack::ImportStats,
}

/// Import from .rtst_vpk, ZIP or folder; the format is detected automatically.
#[tauri::command]
pub fn voice_import(st: S<'_>, path: PathBuf) -> R<ImportResult> {
    let (format, pack) = packloader::import(&path, &st.paths.voicepacks_dir()).map_err(e)?;
    tracing::info!(
        id = %pack.id,
        format = ?format,
        source = %pack.manifest.pack.source,
        events = pack.manifest.events.len(),
        embedded = pack.stats.embedded,
        loose = pack.stats.loose,
        background = pack.stats.background,
        "voice pack imported"
    );
    Ok(ImportResult {
        id: pack.id,
        name: pack.manifest.pack.name,
        format,
        source: pack.manifest.pack.source,
        events: pack.manifest.events.len(),
        stats: pack.stats,
        warnings: pack.warnings,
    })
}

/// Custom sound for one event: the chosen file (from the file dialog) is
/// checked and copied into the app's data folder, so moving or deleting the
/// original does not break it. Returns the path of the copy.
#[tauri::command]
pub fn voice_set_override(app: AppHandle, st: S<'_>, event: String, source: PathBuf) -> R<PathBuf> {
    crate::audio::check_audio(&source)?;
    let meta = std::fs::metadata(&source).map_err(e)?;
    if !meta.is_file() || meta.len() > voicepack::MAX_FILE_BYTES {
        return Err("not a file or larger than the per-file limit".into());
    }
    let dir = st.paths.custom_sounds_dir();
    std::fs::create_dir_all(&dir).map_err(e)?;
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| voicepack::safe_relative(n).is_some_and(|r| r.components().count() == 1))
        .ok_or("unusable file name")?;
    let mut dst = dir.join(&name);
    let mut i = 2;
    while dst.exists() {
        let (stem, ext) = name.rsplit_once('.').unwrap_or((&name, "ogg"));
        dst = dir.join(format!("{stem}-{i}.{ext}"));
        i += 1;
    }
    std::fs::copy(&source, &dst).map_err(e)?;
    let mut cfg = st.cfg().clone();
    let old = cfg.voice.overrides.insert(event.clone(), vec![dst.clone()]);
    cfg.save(&st.paths.config_file()).map_err(e)?;
    *st.config.write().unwrap_or_else(|x| x.into_inner()) = cfg;
    remove_unused_custom(&st, old.unwrap_or_default());
    tracing::info!(event, source = %source.display(), copy = %dst.display(), "custom sound copied into the data folder");
    let _ = app.emit("config-changed", ());
    Ok(dst)
}

#[tauri::command]
pub fn voice_clear_override(app: AppHandle, st: S<'_>, event: String) -> R<()> {
    let mut cfg = st.cfg().clone();
    let old = cfg.voice.overrides.remove(&event);
    cfg.save(&st.paths.config_file()).map_err(e)?;
    *st.config.write().unwrap_or_else(|x| x.into_inner()) = cfg;
    remove_unused_custom(&st, old.unwrap_or_default());
    let _ = app.emit("config-changed", ());
    Ok(())
}

/// Deletes copies in custom-sounds/ that no override uses any more (never
/// files outside that folder).
fn remove_unused_custom(st: &AppState, files: Vec<PathBuf>) {
    let dir = st.paths.custom_sounds_dir();
    let used: Vec<PathBuf> = st
        .cfg()
        .voice
        .overrides
        .values()
        .flatten()
        .cloned()
        .collect();
    for f in files {
        if f.parent() == Some(dir.as_path()) && !used.contains(&f) {
            let _ = std::fs::remove_file(&f);
        }
    }
}

#[derive(Serialize)]
pub struct PackSize {
    files: usize,
    bytes: u64,
}

/// Size of the imported copy, for the confirmation dialog.
#[tauri::command]
pub fn voice_pack_size(st: S<'_>, id: String) -> R<PackSize> {
    let dir = voicepack::removable_pack_dir(&st.paths.voicepacks_dir(), &id).map_err(e)?;
    let (files, bytes) = voicepack::pack_size(&dir);
    Ok(PackSize { files, bytes })
}

/// Seconds a removed pack stays in the trash folder (undo).
pub const UNDO_SECS: u64 = 10;

#[derive(Serialize, Deserialize)]
pub struct RemovedPack {
    id: String,
    trash: PathBuf,
    position: Option<usize>,
    overrides: std::collections::BTreeMap<String, Vec<PathBuf>>,
}

/// Removes an imported pack: only its folder in the library (moved to the
/// trash folder, deleted for good after UNDO_SECS), then cleans the
/// configuration (active list/ranks, overrides into the folder).
#[tauri::command]
pub fn voice_remove_pack(app: AppHandle, st: S<'_>, id: String) -> R<RemovedPack> {
    let lib = st.paths.voicepacks_dir();
    let dir = voicepack::removable_pack_dir(&lib, &id).map_err(e)?;
    st.audio.stop_under(dir.clone());
    let trash = voicepack::trash_pack(&lib, &id).map_err(e)?;
    let mut cfg = st.cfg().clone();
    let position = cfg.voice.active_packs.iter().position(|p| *p == id);
    let overrides = cfg
        .voice
        .overrides
        .iter()
        .filter(|(_, f)| f.iter().any(|f| f.starts_with(&dir)))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    cfg.voice.forget_pack(&id, &dir);
    cfg.save(&st.paths.config_file()).map_err(e)?;
    *st.config.write().unwrap_or_else(|x| x.into_inner()) = cfg;
    engine::reload_voice_packs(&st);
    tracing::info!(pack = %id, dir = %dir.display(), trash = %trash.display(), "voice pack removed (undo possible for {UNDO_SECS} s)");
    let (lib2, t2) = (lib.clone(), trash.clone());
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(UNDO_SECS));
        if t2.exists() {
            match voicepack::purge_trashed(&lib2, &t2) {
                Ok(()) => tracing::info!(trash = %t2.display(), "removed voice pack deleted"),
                Err(err) => {
                    tracing::warn!(trash = %t2.display(), error = %err, "removed voice pack not deleted")
                }
            }
        }
    });
    let _ = app.emit("config-changed", ());
    Ok(RemovedPack {
        id,
        trash,
        position,
        overrides,
    })
}

/// Undo within UNDO_SECS: folder back, rank and overrides back.
#[tauri::command]
pub fn voice_undo_remove(app: AppHandle, st: S<'_>, removed: RemovedPack) -> R<()> {
    let lib = st.paths.voicepacks_dir();
    voicepack::restore_pack(&lib, &removed.trash, &removed.id).map_err(e)?;
    let mut cfg = st.cfg().clone();
    if let Some(i) = removed.position
        && !cfg.voice.active_packs.contains(&removed.id)
    {
        let i = i.min(cfg.voice.active_packs.len());
        cfg.voice.active_packs.insert(i, removed.id.clone());
    }
    cfg.voice.overrides.extend(removed.overrides);
    cfg.save(&st.paths.config_file()).map_err(e)?;
    *st.config.write().unwrap_or_else(|x| x.into_inner()) = cfg;
    engine::reload_voice_packs(&st);
    tracing::info!(pack = %removed.id, "voice pack removal undone");
    let _ = app.emit("config-changed", ());
    Ok(())
}

#[tauri::command]
pub fn voice_export(st: S<'_>, id: String, path: PathBuf, format: PackFormat) -> R<()> {
    let (_, pack) = packloader::load_any(&pack_dir(&st, &id)?).map_err(e)?;
    packloader::loader(format)
        .export_zip(&pack, &path)
        .map_err(e)
}

/// Previews return what was played ("Ogg Vorbis, 44100 Hz, … → default") or why
/// not; they run off the main thread because they wait for the audio thread.
#[tauri::command(async)]
pub fn voice_preview_file(st: S<'_>, id: String, file: String) -> R<String> {
    let (_, pack) = packloader::load_any(&pack_dir(&st, &id)?).map_err(e)?;
    let p = pack.file_path(&file).ok_or("unsafe path")?;
    tracing::debug!(pack = %id, file, resolved = %p.display(), "preview file");
    st.audio.preview(p, 1.0)
}

/// Plays what this event would play with the current settings – the same
/// selection as in a session: a base event ("kill", "death", "headshot" …) goes
/// through the trigger rules and all active packs, a pack key is played from
/// the packs directly.
#[tauri::command(async)]
pub fn voice_preview_event(st: S<'_>, event: String, count: Option<u32>) -> R<String> {
    use ps2_tracker_core::triggers::{FACTS, TriggerEngine};
    let cfg = st.cfg().voice.clone();
    let fact_name = if event == "headshot" {
        "kill"
    } else {
        event.as_str()
    };
    let mut t = Trigger::new(fact_name);
    t.count = count.unwrap_or(match event.as_str() {
        "multikill" => 2,
        "streak" => 5,
        "nemesis" => 3,
        _ => 0,
    });
    t.headshot = event == "headshot";
    let (packs, triggers, mut selector) = {
        let v = lock(&st.voice);
        (
            ps2_tracker_core::voice::effective_packs(&v.packs, cfg.builtin_fallback),
            v.triggers.clone(),
            v.selector.clone(),
        )
    };
    // a copy of the selector at "infinitely later": previews ignore and do not
    // touch the cooldowns of the real playback
    let events = if FACTS.contains(&fact_name) {
        let style = ps2_tracker_core::voice::chain_style(&cfg.chain_style, &packs);
        engine::sound_events(&mut TriggerEngine::default(), &triggers, &t, &cfg, style)
    } else {
        vec![(t, 50)]
    };
    let choice = ps2_tracker_core::voice::choose(
        &mut selector,
        &events,
        &cfg.overrides,
        &packs,
        &[],
        u64::MAX,
    );
    tracing::info!("voice preview: {event}: tried {}", choice.tried.join(" "));
    let sel = choice
        .best
        .ok_or("no sound for this event in the active packs")?;
    let file = sel.file.clone().ok_or("this event is set to no sound")?;
    let info = st.audio.preview(file.clone(), sel.volume)?;
    Ok(format!(
        "{} → {} from {}: {} ({info})",
        event,
        sel.event,
        sel.pack,
        file.file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    ))
}

/// Background image for achievement pop-ups (highest ranked active pack).
#[tauri::command]
pub fn toast_background(st: S<'_>) -> Option<crate::state::PackBackground> {
    lock(&st.voice).background.clone()
}

#[tauri::command]
pub fn voice_output_devices() -> Vec<String> {
    audio::output_devices()
}

#[tauri::command]
pub fn voice_propose_mapping(folder: PathBuf) -> R<Vec<MappingProposal>> {
    voicepack::propose_mapping(&folder).map_err(e)
}

#[derive(Deserialize)]
pub struct MappingRequest {
    folder: PathBuf,
    meta: PackMeta,
    mapping: Vec<MappingProposal>,
}

#[tauri::command]
pub fn voice_create_from_mapping(st: S<'_>, request: MappingRequest) -> R<String> {
    let lib = st.paths.voicepacks_dir();
    std::fs::create_dir_all(&lib).map_err(e)?;
    voicepack::create_pack_from_mapping(&request.folder, &lib, request.meta, &request.mapping)
        .map(|p| p.id)
        .map_err(e)
}

#[tauri::command(async)]
pub fn voice_preview_foreign(folder: PathBuf, file: String, st: S<'_>) -> R<String> {
    let rel = voicepack::safe_relative(&file).ok_or("unsafe path")?;
    st.audio.preview(folder.join(rel), 1.0)
}

// ------------------------------------------------------------------ Crosshair

#[derive(Serialize)]
pub struct CrosshairItem {
    file: String,
    width: u32,
    height: u32,
    data_url: String,
}

#[tauri::command]
pub fn crosshair_list(st: S<'_>) -> Vec<CrosshairItem> {
    use base64::Engine as _;
    crosshair::list(&st.paths.crosshairs_dir())
        .into_iter()
        .filter_map(|p| {
            let bytes = std::fs::read(&p).ok()?;
            let (width, height) = crosshair::inspect(&bytes).ok()?;
            Some(CrosshairItem {
                file: p.file_name()?.to_string_lossy().into_owned(),
                width,
                height,
                data_url: format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&bytes)
                ),
            })
        })
        .collect()
}

#[tauri::command]
pub fn crosshair_import(st: S<'_>, path: PathBuf) -> R<String> {
    crosshair::import(&path, &st.paths.crosshairs_dir()).map_err(e)
}

#[tauri::command]
pub fn crosshair_render_info(app: AppHandle) -> Option<windows::CrosshairRender> {
    windows::crosshair_render(&app)
}

/// Shows a hit-feedback effect in the crosshair window (preview button; also
/// with unsaved settings, so the effect comes from the page).
#[tauri::command]
pub fn crosshair_fx_preview(
    app: AppHandle,
    st: S<'_>,
    kind: String,
    effect: ps2_tracker_core::hitfx::FxEffect,
) -> R<()> {
    use ps2_tracker_core::hitfx::{FxKind, HitFeedback};
    let kind = FxKind::ALL
        .into_iter()
        .find(|k| k.name() == kind)
        .ok_or("unknown effect")?;
    let probe = HitFeedback {
        kill: effect.clone(),
        ..HitFeedback::default()
    };
    probe.validate()?;
    engine::send_crosshair_fx(&app, &st, kind, effect);
    Ok(())
}

#[tauri::command]
pub fn crosshair_toggle(app: AppHandle) -> R<()> {
    windows::toggle_crosshair(&app).map_err(e)
}

/// Opens one of the app's own folders in the file manager (fixed list, no
/// arbitrary paths from the UI).
#[tauri::command]
pub fn open_folder(st: S<'_>, kind: String) -> R<()> {
    let p = &st.paths;
    let dir = match kind.as_str() {
        "config" => p.config_dir.clone(),
        "data" => p.data_dir.clone(),
        "voicepacks" => p.voicepacks_dir(),
        "crosshairs" => p.crosshairs_dir(),
        "logs" => p.state_dir.clone(),
        _ => return Err("unknown folder".into()),
    };
    std::fs::create_dir_all(&dir).map_err(e)?;
    // Linux: xdg-open (inside Flatpak the portal's OpenURI); Windows: Explorer
    let opener = if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(&dir)
        .spawn()
        .map(|mut child| {
            // reap the helper process
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        })
        .map_err(|err| format!("xdg-open: {err}"))
}

#[tauri::command]
pub fn focus_probe(app: AppHandle, st: S<'_>) -> focus::FocusInfo {
    let m = focus::GameMatch::from_cfg(&st.cfg());
    focus::probe(&app, &m)
}

/// Open windows for the "Game window" selection (off the main thread: X11 round trips).
#[tauri::command(async)]
pub fn game_windows(st: S<'_>) -> Vec<focus::WindowEntry> {
    let m = focus::GameMatch::from_cfg(&st.cfg());
    focus::list_windows(&m)
}

#[tauri::command(async)]
pub fn game_status(st: S<'_>) -> focus::GameStatus {
    let m = focus::GameMatch::from_cfg(&st.cfg());
    focus::game_status(&m)
}

#[cfg(test)]
mod capability_tests {
    /// Every command in the invoke handler must be declared in build.rs and be
    /// allowed for at least one window – otherwise Tauri refuses it at runtime.
    #[test]
    fn every_command_is_declared_and_allowed() {
        let main_rs = include_str!("main.rs");
        let build_rs = include_str!("../build.rs");
        let caps = [
            include_str!("../capabilities/main.json"),
            include_str!("../capabilities/overlay.json"),
            include_str!("../capabilities/popup.json"),
            include_str!("../capabilities/crosshair.json"),
            include_str!("../capabilities/movelayer.json"),
        ]
        .concat();
        let start = main_rs.find("generate_handler![").expect("handler list");
        let list = &main_rs[start..main_rs[start..].find("])").expect("end") + start];
        let names: Vec<&str> = list
            .split("commands::")
            .skip(1)
            .map(|s| {
                s.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .next()
                    .unwrap_or("")
            })
            .collect();
        assert!(names.len() > 40, "parsed {} commands", names.len());
        for n in names {
            assert!(
                build_rs.contains(&format!("\"{n}\"")),
                "{n} missing in build.rs COMMANDS"
            );
            let perm = format!("\"allow-{}\"", n.replace('_', "-"));
            assert!(
                caps.contains(&perm),
                "{perm} missing in capabilities/*.json"
            );
        }
        // and nothing allowed that is not a command of this app
        for perm in caps.split('"').filter(|p| p.starts_with("allow-")) {
            let cmd = perm.trim_start_matches("allow-").replace('-', "_");
            assert!(
                list.contains(&format!("commands::{cmd},"))
                    || list.contains(&format!("commands::{cmd}\n")),
                "{perm} allowed, but no command {cmd}"
            );
        }
    }
}
