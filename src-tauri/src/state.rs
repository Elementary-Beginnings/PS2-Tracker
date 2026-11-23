//! Shared app state.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8};
use std::sync::{Arc, Mutex, RwLock};

use ps2_tracker_core::census::{NameCache, SharedNames};
use ps2_tracker_core::triggers::{TriggerEngine, TriggerSet};
use ps2_tracker_core::voice::{VoiceDetector, VoiceSelector};
use ps2_tracker_core::voicepack::VoicePack;
use ps2_tracker_core::{CensusClient, CharacterId, Config, LifetimeStats, Paths, Store, Tracker};
use serde::Serialize;
use tokio::sync::{broadcast, watch};

use crate::audio::AudioHandle;

#[derive(Debug, Clone, Serialize, Default)]
pub struct EssStatus {
    /// "idle" | "connecting" | "connected" | "disconnected"
    pub state: String,
    pub detail: String,
    pub retry_in_secs: Option<u64>,
    pub last_event_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackBackground {
    pub pack: String,
    pub data_url: String,
    pub width: u32,
    pub height: u32,
}

pub struct VoiceState {
    pub packs: Vec<VoicePack>,
    /// Background of the highest ranked active pack that has one.
    pub background: Option<PackBackground>,
    pub selector: VoiceSelector,
    pub detectors: HashMap<CharacterId, VoiceDetector>,
    pub triggers: TriggerSet,
    pub engines: HashMap<CharacterId, TriggerEngine>,
}

/// Names waiting for the batched Census lookup (see `engine::request_names`).
#[derive(Default)]
pub struct NameQueue {
    pub items: std::collections::BTreeSet<u32>,
    pub chars: std::collections::BTreeSet<ps2_tracker_core::events::CharacterId>,
    /// A worker is draining the queue.
    pub running: bool,
}

pub struct AppState {
    pub paths: Paths,
    pub name_queue: Mutex<NameQueue>,
    pub config: RwLock<Config>,
    pub store: Arc<Store>,
    pub names: SharedNames,
    /// item_id → category name (for voice conditions)
    pub item_categories: Mutex<HashMap<u32, String>>,
    /// item_category_id → Name
    pub category_names: Mutex<HashMap<u32, String>>,
    pub tracker: Mutex<Tracker>,
    /// Event key → last time its sound played (Unix seconds), for the Events page.
    pub last_played: Mutex<HashMap<String, i64>>,
    pub lifetime: Mutex<HashMap<CharacterId, LifetimeStats>>,
    pub census: RwLock<CensusClient>,
    pub status: Mutex<EssStatus>,
    pub ess_stop: Mutex<Option<watch::Sender<bool>>>,
    /// Characters of the running ESS connection; a change is subscribed live.
    pub ess_characters: Mutex<Option<watch::Sender<Vec<ps2_tracker_core::events::CharacterId>>>>,
    pub overlay_edit: AtomicBool,
    pub crosshair_visible: AtomicBool,
    /// Set by the window watcher: `FocusState` as u8 (game / other / unknown).
    pub focus_state: AtomicU8,
    /// Last applied crosshair visibility (0 = not yet, 1 = hidden, 2 = shown), for the log.
    pub crosshair_last: AtomicU8,
    /// Last overlay log line (only changes are logged at info level).
    pub overlay_last: Mutex<HashMap<&'static str, String>>,
    /// Requested position per overlay object (for the position check).
    pub overlay_target: Mutex<HashMap<&'static str, (i32, i32)>>,
    pub overlay_corrections: Mutex<HashMap<&'static str, Vec<std::time::Instant>>>,
    /// Object selected in the move mode (0 stats, 1 pop-up).
    pub overlay_selected: AtomicU8,
    /// X11 window type applied per window label (true = on-screen display).
    pub x11_types: Mutex<HashMap<&'static str, bool>>,
    /// Hotkeys that could not be registered: (combination, action, reason).
    pub hotkey_failures: Mutex<Vec<(String, &'static str, String)>>,
    /// Messages for the main window from the start (shown once).
    pub notices: Mutex<Vec<Notice>>,
    /// Open move layer (move mode), see `windows::start_move`.
    pub move_session: Mutex<Option<crate::windows::MoveSession>>,
    pub voice: Mutex<VoiceState>,
    pub audio: AudioHandle,
    /// (event name, JSON) for the OBS source
    pub obs_tx: broadcast::Sender<(String, String)>,
    pub obs_stop: Mutex<Option<watch::Sender<bool>>>,
}

impl AppState {
    pub fn names(&self) -> std::sync::MutexGuard<'_, NameCache> {
        self.names.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn cfg(&self) -> std::sync::RwLockReadGuard<'_, Config> {
        self.config.read().unwrap_or_else(|e| e.into_inner())
    }
    pub fn census(&self) -> CensusClient {
        self.census
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

pub fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A message from the start for the main window; `kind` selects the text in
/// the UI (en.ts `notices`), `detail` is inserted (e.g. a file name).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Notice {
    pub kind: &'static str,
    pub detail: String,
}
