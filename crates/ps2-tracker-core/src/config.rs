//! Configuration as TOML in `$XDG_CONFIG_HOME/ps2-tracker/config.toml`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::events::CharacterId;
use crate::session::SessionConfig;

pub const APP_DIR: &str = "ps2-tracker";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("file {0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("invalid TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("writing TOML: {0}")]
    Write(#[from] toml::ser::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("no home directory found")]
    NoHome,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    /// Ask the update endpoint once at start. Without this the app makes no
    /// connection besides Census/ESS (and the optional Discord IPC, which is local).
    pub check_on_start: bool,
}

/// Most squad/outfit members tracked at the same time (one ESS subscription,
/// Census rate limit).
pub const MAX_SQUAD: usize = 20;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub census: CensusSettings,
    #[serde(rename = "character")]
    pub characters: Vec<CharacterSettings>,
    /// Squad/outfit members tracked alongside (public game data, stored
    /// locally). At most [`MAX_SQUAD`].
    #[serde(default)]
    pub squad: Vec<CharacterSettings>,
    pub session: SessionSettings,
    /// Discord Rich Presence (off by default, needs the user's own app ID).
    pub discord: crate::presence::DiscordSettings,
    /// In-app updates (AppImage/Windows only; off by default).
    pub updates: UpdateSettings,
    /// Online sync to the ELME stats server (off by default, opt-in).
    pub online_sync: crate::online_sync::SyncSettings,
    pub overlay: OverlaySettings,
    pub voice: VoiceSettings,
    pub crosshair: CrosshairSettings,
    pub obs: ObsSettings,
    pub ui: UiSettings,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CensusSettings {
    /// Own Census service ID without `s:`; empty = the built-in default of
    /// official builds, otherwise `example`. `PS2_SERVICE_ID` takes
    /// precedence (see [`crate::service_id`]). Only the user's own value is
    /// ever stored here, never the built-in default or the environment value.
    pub service_id: String,
}

impl CensusSettings {
    /// The service ID in use: environment > this setting > built-in > example.
    pub fn resolve(&self) -> Result<crate::service_id::ServiceId, ConfigError> {
        crate::service_id::ServiceId::resolve(&self.service_id).map_err(ConfigError::Invalid)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterSettings {
    pub name: String,
    /// Filled in on the first lookup.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::events::id_string::option"
    )]
    pub id: Option<CharacterId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction_id: Option<u32>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionSettings {
    /// A session ends after this many minutes without an event.
    pub idle_timeout_minutes: u32,
    /// After a logout (or a crash: the server logs the character out) the
    /// session waits this many minutes for a relogin and then goes on;
    /// 0 = a logout ends the session at once.
    pub resume_minutes: u32,
    pub streak_step: u32,
    pub killfeed_len: usize,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            idle_timeout_minutes: 30,
            resume_minutes: 15,
            streak_step: 5,
            killfeed_len: 12,
        }
    }
}

impl SessionSettings {
    pub fn session_config(&self) -> SessionConfig {
        SessionConfig {
            streak_step: self.streak_step,
            killfeed_len: self.killfeed_len.clamp(1, 100),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlaySettings {
    pub enabled: bool,
    /// Monitor name (as reported by Tauri); empty = primary monitor.
    pub monitor: String,
    /// Corner/edge the overlay sticks to (see `overlay::Anchor`). Configs from
    /// before this field stored x/y from the top-left corner.
    #[serde(default = "legacy_anchor")]
    pub anchor: String,
    /// Distance from the anchored edges in physical pixels.
    pub x: i32,
    pub y: i32,
    /// Window size = measured size of the content (set by the overlay itself,
    /// fixed while the settings stay the same).
    pub width: u32,
    pub height: u32,
    /// Size step "auto" | "S" | "M" | "L" | "XL" (auto = from monitor height).
    pub size: String,
    /// Look: "card" (default), "slim" or "bare".
    pub style: String,
    /// Achievement/voice pack pop-up: its own window, positioned like the
    /// overlay (anchor + offset on a monitor).
    pub popup_anchor: String,
    /// Monitor of the pop-up; empty = the same monitor as the overlay.
    pub popup_monitor: String,
    pub popup_x: i32,
    pub popup_y: i32,
    /// Measured size of the pop-up content (image × scale, or the text box).
    pub popup_width: u32,
    pub popup_height: u32,
    /// Integer scale of the pack image (1 = pixel exact).
    pub popup_scale: u32,
    /// How long a pop-up stays (ms); fades in and out.
    pub popup_duration_ms: u32,
    /// Achievement name/text on the image (shadow, no box).
    pub popup_text: bool,
    /// Without a pack image: "plain" (text with shadow) or "card" (app style).
    pub popup_style: String,
    /// Opacity of the pop-up (the image keeps its own alpha on top).
    pub popup_opacity: f32,
    /// Background opacity 0.0–1.0 (text stays fully visible).
    pub background_opacity: f32,
    pub scale: f32,
    pub click_through: bool,
    pub show_stats: bool,
    pub show_killfeed: bool,
    pub show_weapons: bool,
    pub show_toasts: bool,
    pub show_comparison: bool,
    /// Kill feed: also the own class at the time of the event (the other
    /// party's class is always shown when known).
    #[serde(default)]
    pub show_my_class: bool,
    /// Hotkey to switch move ↔ click-through (X11/XWayland).
    pub hotkey_edit: String,
    /// Ends the move mode and saves; a global hotkey for the whole move
    /// session, also while the game holds the pointer (Ctrl+F2 is often taken
    /// by the desktop, e.g. KWin "Switch to Desktop 2").
    pub hotkey_finish_move: String,
    /// The move mode ends by itself (and saves) after this many seconds
    /// without input in the move layer; 0 = never.
    pub move_idle_timeout_secs: u32,
    pub hotkey_toggle: String,
    /// Achievement pop-ups on the background image of the highest ranked active
    /// pack (`AchievementBG.png`), if it has one.
    pub pack_background: bool,
    /// Show the overlay only while the game window is active (same detection
    /// as the crosshair).
    pub auto_hide: bool,
    /// Smaller overlay: no weapon list, shorter kill feed.
    pub compact: bool,
    /// Extra tile: K/D with revives (deaths you were revived from not counted).
    pub show_kd_revived: bool,
    /// Block "Squad top 3": the three best tracked characters of the running
    /// sessions (own and squad). Always three rows, so the window size stays.
    pub show_squad: bool,
    /// Ranking of the block: "kd", "kills" or "kpm" (see `tracker::SquadMetric`).
    pub squad_metric: String,
    /// Minimum kills for the ratio metrics (K/D, KPM), so 1/0 is not first.
    pub squad_min_kills: u32,
}

fn legacy_anchor() -> String {
    "top-left".into()
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            monitor: String::new(),
            anchor: "top-right".into(),
            x: 24,
            y: 24,
            width: 360,
            height: 420,
            size: "auto".into(),
            style: "card".into(),
            popup_anchor: "top-center".into(),
            popup_monitor: String::new(),
            popup_x: 0,
            popup_y: 160,
            popup_width: 420,
            popup_height: 100,
            popup_scale: 1,
            popup_duration_ms: 4000,
            popup_text: true,
            popup_style: "plain".into(),
            popup_opacity: 1.0,
            background_opacity: 0.7,
            scale: 1.0,
            click_through: true,
            show_stats: true,
            show_killfeed: true,
            show_weapons: true,
            show_toasts: true,
            show_comparison: true,
            show_my_class: false,
            hotkey_edit: "Ctrl+F2".into(),
            hotkey_finish_move: default_finish_key().into(),
            move_idle_timeout_secs: 300,
            hotkey_toggle: "Ctrl+Shift+F10".into(),
            pack_background: true,
            auto_hide: false,
            compact: false,
            show_kd_revived: false,
            show_squad: false,
            squad_metric: "kd".into(),
            squad_min_kills: 5,
        }
    }
}

/// Stable rule for the game window (survives a game restart, unlike a window
/// id). Non-empty fields must all match; all empty = auto-detect PlanetSide 2.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameWindowRule {
    /// Part of WM_CLASS (Linux), e.g. "steam_app_218230".
    pub class: String,
    /// Part of the window title.
    pub title: String,
    /// Process name, e.g. "PlanetSide2_x64.exe".
    pub process: String,
}

impl GameWindowRule {
    pub fn is_auto(&self) -> bool {
        self.class.is_empty() && self.title.is_empty() && self.process.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    pub enabled: bool,
    /// Master volume 0.0–1.0
    pub volume: f32,
    /// Output device name (cpal/ALSA, e.g. "pipewire"); empty = default.
    pub output_device: String,
    /// Active packs (folder names) in fallback order.
    pub active_packs: Vec<String>,
    /// Custom files per event; take precedence over all packs.
    pub overrides: BTreeMap<String, Vec<PathBuf>>,
    /// Muted events.
    pub muted: Vec<String>,
    pub max_concurrent: usize,
    pub multikill_window_secs: i64,
    /// Pack key played on every kill as the least specific choice, for packs
    /// without `PER_KILL` (e.g. older XML packs); empty = none.
    pub kill_key: String,
    /// Kill chain announcements: "auto" | "classic" (Excellent/Outstanding/
    /// Superb) | "unreal" (Double/Triple).
    pub chain_style: String,
    /// Built-in standard tones as fallback while a real pack is active (off:
    /// an event without a pack sound stays silent).
    pub builtin_fallback: bool,
    /// Trigger conditions edited on the Events page; replace the rules of their
    /// event from triggers.toml / the defaults. Empty list = no trigger.
    pub trigger_overrides: BTreeMap<String, Vec<crate::triggers::TriggerRule>>,
}

impl VoiceSettings {
    /// Forgets a removed pack: out of the active list (the order is the rank,
    /// so the ranks close up) and drops overrides that point into its folder.
    pub fn forget_pack(&mut self, id: &str, pack_dir: &std::path::Path) {
        self.active_packs.retain(|p| p != id);
        self.overrides
            .retain(|_, files| !files.iter().any(|f| f.starts_with(pack_dir)));
    }
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            volume: 0.6,
            output_device: String::new(),
            active_packs: vec![crate::voicepack::STANDARD_PACK_ID.into()],
            overrides: BTreeMap::new(),
            muted: vec!["kill".into()],
            max_concurrent: 1,
            multikill_window_secs: 4,
            kill_key: String::new(),
            chain_style: "auto".into(),
            builtin_fallback: false,
            trigger_overrides: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrosshairSettings {
    pub enabled: bool,
    /// File name in the crosshair library.
    pub file: String,
    /// Integer for pixel-exact rendering (1–8).
    pub scale: u32,
    pub opacity: f32,
    /// Tint "#rrggbb"; empty = original colours.
    pub tint: String,
    pub offset_x: i32,
    pub offset_y: i32,
    /// Monitor name; empty = primary monitor.
    pub monitor: String,
    pub hotkey_toggle: String,
    pub hotkey_cycle: String,
    /// Only show while the game window is active (X11/XWayland, see docs/wiki/Crosshair.md).
    pub auto_hide: bool,
    /// Which window is the game. All fields empty = auto-detect PlanetSide 2.
    pub game_window: GameWindowRule,
    /// Pattern for the game's WM_CLASS or window title (substring, case-insensitive, | separates).
    pub game_match: String,
    /// Flash / swap the crosshair on kills, headshots, assists (from ESS).
    pub hit_feedback: crate::hitfx::HitFeedback,
}

impl Default for CrosshairSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            file: "kreuz-punkt-gruen.png".into(),
            scale: 1,
            opacity: 1.0,
            tint: String::new(),
            offset_x: 0,
            offset_y: 0,
            monitor: String::new(),
            hotkey_toggle: "Ctrl+Shift+F11".into(),
            hotkey_cycle: "Ctrl+Shift+F12".into(),
            auto_hide: false,
            game_window: GameWindowRule::default(),
            game_match: "planetside2|steam_app_218230|PlanetSide 2".into(),
            hit_feedback: crate::hitfx::HitFeedback::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ObsSettings {
    /// Local endpoint (127.0.0.1 only) for an OBS browser source.
    pub enabled: bool,
    pub port: u16,
}

impl Default for ObsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 47_823,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    /// "system", "dark" or "light"
    pub theme: String,
    pub start_minimized: bool,
    /// Run the GTK windows through XWayland (`GDK_BACKEND=x11`) in a Wayland
    /// session. Only then can the app place the crosshair/overlay itself and keep
    /// them above the game (KWin ignores both for native Wayland clients).
    pub x11_backend: bool,
    /// Accent colour: "faction" (of the active character), "neutral" or "custom".
    pub accent: String,
    /// Custom accent colour (#rrggbb), used with `accent = "custom"`.
    pub accent_custom: String,
    /// Linux + NVIDIA: start WebKitGTK with WEBKIT_DISABLE_DMABUF_RENDERER=1
    /// (grey window otherwise). Takes effect after a restart.
    pub webkit_dmabuf_workaround: bool,
    /// Linux + NVIDIA: start WebKitGTK with WEBKIT_DISABLE_COMPOSITING_MODE=1
    /// (the overlay background got darker with every repaint otherwise, see
    /// `overlay::compositing_fix_wanted`). Takes effect after a restart.
    pub webkit_compositing_workaround: bool,
    /// The first-start assistant was finished or skipped.
    pub setup_done: bool,
    /// Big K/D on the live page: "true" (kills/deaths) or "revived" (deaths
    /// you were revived from not counted, as in game).
    pub primary_kd: String,
    /// Debug mode at every start: log level debug, log output in a console
    /// (Windows: own console window; Linux: the calling terminal). Same as
    /// the command line switch `--debug-console`.
    #[serde(default)]
    pub debug_console: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            start_minimized: false,
            x11_backend: true,
            accent: "faction".into(),
            accent_custom: "#4fc3c9".into(),
            setup_done: false,
            primary_kd: "true".into(),
            webkit_dmabuf_workaround: true,
            webkit_compositing_workaround: true,
            debug_console: false,
        }
    }
}

/// Default key for "Finish move mode": Ctrl+Esc on Linux; Windows reserves
/// Ctrl+Esc for the Start menu, so Ctrl+Shift+F2 there.
pub fn default_finish_key() -> &'static str {
    if cfg!(windows) {
        WIN_FINISH_KEYS[0]
    } else {
        "Ctrl+Escape"
    }
}

const WIN_FINISH_KEYS: [&str; 2] = ["Ctrl+Shift+F2", "Ctrl+Alt+F2"];

/// Windows finish key that no other hotkey of the app uses.
pub fn windows_finish_key(others: &[&str]) -> &'static str {
    let used = |k: &str| others.iter().any(|o| o.trim().eq_ignore_ascii_case(k));
    if used(WIN_FINISH_KEYS[0]) {
        WIN_FINISH_KEYS[1]
    } else {
        WIN_FINISH_KEYS[0]
    }
}

/// XDG directories.
#[derive(Debug, Clone)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    /// `$XDG_STATE_HOME/ps2-tracker` (log file); falls back to `data_dir`.
    pub state_dir: PathBuf,
}

impl Paths {
    pub fn xdg() -> Result<Self, ConfigError> {
        let base = directories::BaseDirs::new().ok_or(ConfigError::NoHome)?;
        Ok(Self {
            config_dir: base.config_dir().join(APP_DIR),
            data_dir: base.data_dir().join(APP_DIR),
            state_dir: base
                .state_dir()
                .unwrap_or_else(|| base.data_dir())
                .join(APP_DIR),
        })
    }
    pub fn log_file(&self) -> PathBuf {
        self.state_dir.join("ps2-tracker.log")
    }
    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }
    pub fn achievements_file(&self) -> PathBuf {
        self.config_dir.join("achievements.toml")
    }
    pub fn database(&self) -> PathBuf {
        self.data_dir.join("tracker.sqlite3")
    }
    pub fn voicepacks_dir(&self) -> PathBuf {
        self.data_dir.join("voicepacks")
    }
    pub fn crosshairs_dir(&self) -> PathBuf {
        self.data_dir.join("crosshairs")
    }
    /// Copies of the files chosen as custom sounds for single events.
    pub fn custom_sounds_dir(&self) -> PathBuf {
        self.data_dir.join("custom-sounds")
    }
}

/// Device names that mean "whatever the system default is" (cpal's name for
/// ALSA "default", and the old UI label).
pub fn is_default_device_name(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    n == "default audio device" || n == "default" || n.starts_with("system default")
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let c: Config = toml::from_str(text)?;
        c.validate()?;
        Ok(c)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        let sid = &self.census.service_id;
        if !sid.is_empty() && !crate::service_id::is_valid(sid) {
            return Err(ConfigError::Invalid(
                "Service ID may only contain letters, digits and _ (without \"s:\")".into(),
            ));
        }
        self.discord.validate().map_err(ConfigError::Invalid)?;
        self.online_sync
            .validate()
            .map_err(|e| ConfigError::Invalid(format!("online sync: {e}")))?;
        if self.squad.len() > MAX_SQUAD {
            return Err(ConfigError::Invalid(format!(
                "at most {MAX_SQUAD} squad members"
            )));
        }
        for c in self.characters.iter().chain(&self.squad) {
            if c.name.trim().is_empty() || c.name.len() > 32 {
                return Err(ConfigError::Invalid(format!(
                    "invalid character name: {:?}",
                    c.name
                )));
            }
        }
        if !(0.0..=1.0).contains(&self.voice.volume) {
            return Err(ConfigError::Invalid(
                "volume must be between 0 and 1".into(),
            ));
        }
        crate::triggers::TriggerSet::validate_overrides(&self.voice.trigger_overrides)
            .map_err(|e| ConfigError::Invalid(format!("trigger: {e}")))?;
        self.crosshair
            .hit_feedback
            .validate()
            .map_err(|e| ConfigError::Invalid(format!("hit feedback: {e}")))?;
        if !(1..=8).contains(&self.crosshair.scale) {
            return Err(ConfigError::Invalid("crosshair scale must be 1-8".into()));
        }
        if !(0.0..=1.0).contains(&self.crosshair.opacity) {
            return Err(ConfigError::Invalid("crosshair opacity must be 0-1".into()));
        }
        let t = &self.crosshair.tint;
        if !t.is_empty()
            && !(t.len() == 7
                && t.starts_with('#')
                && t[1..].chars().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(ConfigError::Invalid("tint must be #rrggbb".into()));
        }
        if self.crosshair.file.contains(['/', '\\', ':']) || self.crosshair.file.contains("..") {
            return Err(ConfigError::Invalid(
                "crosshair file: file name only".into(),
            ));
        }
        if !(1..=4).contains(&self.voice.max_concurrent) {
            return Err(ConfigError::Invalid("concurrent sounds must be 1-4".into()));
        }
        if self.obs.port < 1024 {
            return Err(ConfigError::Invalid("OBS port must be >= 1024".into()));
        }
        Ok(())
    }

    /// Loads the file, or returns defaults if it is missing.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let mut c = match std::fs::read_to_string(path) {
            Ok(t) => Self::parse(&t)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(ConfigError::Io(path.to_owned(), e)),
        };
        // an invalid PS2_SERVICE_ID fails here, as before
        c.census.resolve()?;
        c.migrate();
        Ok(c)
    }

    /// Older versions stored the name of cpal's default device; that is the
    /// ALSA "default" (dmix on the hardware with PipeWire running) and not the
    /// system default the user meant. Empty = system default (PipeWire first).
    pub fn migrate(&mut self) {
        // the move mode moved from Ctrl+Shift+F9 to Ctrl+F2
        if self.overlay.hotkey_edit == "Ctrl+Shift+F9" {
            self.overlay.hotkey_edit = "Ctrl+F2".into();
        }
        self.migrate_finish_key(cfg!(windows));
        // Older versions wrote the default "example" into the file. Empty now
        // means "built-in default, otherwise example".
        if self.census.service_id == crate::service_id::EXAMPLE {
            tracing::info!("service ID \"example\" from the file cleared (empty = default)");
            self.census.service_id.clear();
        }
        // Rich Presence: empty = the built-in default application; a stored
        // copy of the default is cleared (the value itself is never logged)
        if self.discord.migrate() {
            tracing::info!("Discord application ID equal to the default cleared (empty = default)");
        }
        if is_default_device_name(&self.voice.output_device) {
            tracing::info!(
                old = %self.voice.output_device,
                "output device migrated to the system default (PipeWire first)"
            );
            self.voice.output_device.clear();
        }
    }

    /// Windows reserves Ctrl+Esc (Start menu); RegisterHotKey fails for it.
    /// A stored Ctrl+Esc as "Finish move mode" becomes Ctrl+Shift+F2, or
    /// Ctrl+Alt+F2 if another hotkey of the app already uses that.
    pub fn migrate_finish_key(&mut self, windows: bool) {
        if !windows
            || !self
                .overlay
                .hotkey_finish_move
                .eq_ignore_ascii_case("Ctrl+Escape")
        {
            return;
        }
        let others = [
            self.overlay.hotkey_edit.as_str(),
            self.overlay.hotkey_toggle.as_str(),
            self.crosshair.hotkey_toggle.as_str(),
            self.crosshair.hotkey_cycle.as_str(),
        ];
        let new = windows_finish_key(&others);
        tracing::info!(
            old = %self.overlay.hotkey_finish_move,
            new,
            "finish move mode key migrated (Ctrl+Esc is reserved by Windows)"
        );
        self.overlay.hotkey_finish_move = new.into();
    }

    /// Writes atomically (temp file + rename), mode 0600.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        self.validate()?;
        let dir = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).map_err(|e| ConfigError::Io(dir.to_owned(), e))?;
        // census.service_id only ever holds the user's own value; the
        // environment value and the built-in default are resolved at use.
        let text = format!(
            "# PS2 Session Tracker configuration (also written by the UI)\n{}",
            toml::to_string_pretty(self)?
        );
        // one temp file per call: saves from different threads (hotkey,
        // UI, overlay size) must not write into the same temp file
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = path.with_extension(format!("toml.{}-{n}.tmp", std::process::id()));
        std::fs::write(&tmp, text).map_err(|e| ConfigError::Io(tmp.clone(), e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        let res = rename_retrying(&tmp, path);
        if res.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        res.map_err(|e| ConfigError::Io(path.to_owned(), e))
    }
}

/// `rename` over the old file. Windows refuses it with "access denied" while
/// another program (virus scanner, indexer, editor) has the target open
/// without delete sharing; that is usually over within milliseconds.
fn rename_retrying(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut tries = 0;
    loop {
        match std::fs::rename(from, to) {
            Err(e)
                if cfg!(windows)
                    && e.kind() == std::io::ErrorKind::PermissionDenied
                    && tries < 10 =>
            {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            r => return r,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_device_name_migrates_to_system_default() {
        let mut c = Config::default();
        c.voice.output_device = "Default Audio Device".into();
        c.migrate();
        assert_eq!(c.voice.output_device, "");
        c.voice.output_device = "PipeWire Sound Server".into();
        c.migrate();
        assert_eq!(c.voice.output_device, "PipeWire Sound Server");
    }

    #[test]
    fn forget_pack_cleans_config() {
        let mut v = Config::default().voice;
        v.active_packs = vec!["a".into(), "gone".into(), "b".into()];
        v.overrides
            .insert("kill".into(), vec!["/lib/gone/sounds/x.ogg".into()]);
        v.overrides
            .insert("death".into(), vec!["/home/me/own.ogg".into()]);
        v.forget_pack("gone", std::path::Path::new("/lib/gone"));
        assert_eq!(v.active_packs, vec!["a".to_string(), "b".to_string()]);
        assert!(!v.overrides.contains_key("kill"));
        assert!(v.overrides.contains_key("death"));
    }

    use super::*;

    #[test]
    fn windows_finish_key_migration() {
        // Linux keeps Ctrl+Esc
        let mut c = Config::default();
        c.overlay.hotkey_finish_move = "Ctrl+Escape".into();
        c.migrate_finish_key(false);
        assert_eq!(c.overlay.hotkey_finish_move, "Ctrl+Escape");
        // Windows: Ctrl+Shift+F2, or Ctrl+Alt+F2 when that is taken
        c.migrate_finish_key(true);
        assert_eq!(c.overlay.hotkey_finish_move, "Ctrl+Shift+F2");
        let mut c = Config::default();
        c.overlay.hotkey_finish_move = "ctrl+escape".into();
        c.crosshair.hotkey_cycle = "Ctrl+Shift+F2".into();
        c.migrate_finish_key(true);
        assert_eq!(c.overlay.hotkey_finish_move, "Ctrl+Alt+F2");
        // a key the user chose is never touched
        let mut c = Config::default();
        c.overlay.hotkey_finish_move = "Ctrl+F3".into();
        c.migrate_finish_key(true);
        assert_eq!(c.overlay.hotkey_finish_move, "Ctrl+F3");
        // the default does not collide with the other default hotkeys
        let d = Config::default();
        for k in [
            &d.overlay.hotkey_edit,
            &d.overlay.hotkey_toggle,
            &d.crosshair.hotkey_toggle,
            &d.crosshair.hotkey_cycle,
        ] {
            assert_ne!(k, &d.overlay.hotkey_finish_move);
            assert_ne!(k.as_str(), "Ctrl+Shift+F2");
        }
        assert_eq!(
            default_finish_key(),
            if cfg!(windows) {
                "Ctrl+Shift+F2"
            } else {
                "Ctrl+Escape"
            }
        );
    }

    #[test]
    fn crosshair_file_is_a_plain_name_on_every_platform() {
        for bad in [
            "a/b.png",
            "a\\b.png",
            "C:x.png",
            "C:\\x.png",
            "..",
            "x..png",
        ] {
            let mut c = Config::default();
            c.crosshair.file = bad.into();
            assert!(c.validate().is_err(), "{bad}");
        }
        let mut c = Config::default();
        c.crosshair.file = "my crosshair.png".into();
        assert!(c.validate().is_ok());
    }

    #[test]
    fn concurrent_saves_leave_a_valid_file() {
        let dir = std::env::temp_dir().join(format!("ps2t-cfgsave-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("config.toml");
        let threads: Vec<_> = (0..8)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let mut c = Config::default();
                    c.overlay.width = 300 + i;
                    for _ in 0..20 {
                        c.save(&path).unwrap();
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        let c = Config::load(&path).unwrap();
        assert!((300..308).contains(&c.overlay.width));
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(left.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn defaults_roundtrip() {
        let c = Config::default();
        let t = toml::to_string_pretty(&c).unwrap();
        assert_eq!(Config::parse(&t).unwrap(), c);
    }

    #[test]
    fn parses_characters() {
        let t = r#"
[census]
service_id = "myservice"

[[character]]
name = "SampleSoldier"
faction_id = 3

[[character]]
name = "AltVS"
enabled = false
"#;
        let c = Config::parse(t).unwrap();
        assert_eq!(c.characters.len(), 2);
        assert!(!c.characters[1].enabled);
        assert_eq!(c.overlay.width, 360);
    }

    #[test]
    fn character_id_is_string_in_json_and_accepts_numbers() {
        let c = CharacterSettings {
            name: "SampleSoldier".into(),
            id: Some(5428000000000000042),
            faction_id: Some(3),
            enabled: true,
        };
        let j = serde_json::to_string(&c).unwrap();
        assert!(j.contains("\"id\":\"5428000000000000042\""), "{j}");
        let back: CharacterSettings = serde_json::from_str(&j).unwrap();
        assert_eq!(back.id, c.id);
        let t: Config =
            toml::from_str("[[character]]\nname='x'\nid=5428000000000000042\n").unwrap();
        assert_eq!(t.characters[0].id, Some(5428000000000000042));
    }

    #[test]
    fn rejects_bad_service_id() {
        assert!(Config::parse("[census]\nservice_id = \"a/b\"\n").is_err());
        assert!(Config::parse("[census]\nservice_id = \"s:abc\"\n").is_err());
        // empty = built-in default / example
        assert!(Config::parse("[census]\nservice_id = \"\"\n").is_ok());
    }

    #[test]
    fn stored_example_becomes_empty() {
        let mut c = Config::parse("[census]\nservice_id = \"example\"\n").unwrap();
        c.migrate();
        assert_eq!(c.census.service_id, "");
        let mut own = Config::parse("[census]\nservice_id = \"own_id\"\n").unwrap();
        own.migrate();
        assert_eq!(own.census.service_id, "own_id");
    }

    /// The file holds only the user's own value: with the setting empty,
    /// neither the built-in default nor "example" is written.
    #[test]
    fn built_in_default_is_not_saved() {
        let dir = std::env::temp_dir().join(format!("ps2t-sid-{}", std::process::id()));
        let p = dir.join("config.toml");
        let c = Config::default();
        c.save(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("service_id = \"\""), "{text}");
        assert!(!text.contains("example"), "{text}");
        if let Some(b) = crate::service_id::built_in() {
            assert!(!text.contains(b), "built-in default written to the file");
        }
        // what the app would use comes from the resolver, not from the file
        let back = Config::load(&p).unwrap();
        assert_eq!(back.census.service_id, "");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_and_load() {
        let dir = std::env::temp_dir().join(format!("ps2t-cfg-{}", std::process::id()));
        let p = dir.join("config.toml");
        let mut c = Config::default();
        c.characters.push(CharacterSettings {
            name: "Test".into(),
            id: Some(1),
            faction_id: Some(2),
            enabled: true,
        });
        c.save(&p).unwrap();
        let back = Config::load(&p).unwrap();
        assert_eq!(back.characters, c.characters);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
