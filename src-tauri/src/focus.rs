//! Detects whether the game window is active – only through window properties of
//! the X server (XWayland/X11: `_NET_ACTIVE_WINDOW`, `WM_CLASS`, `_NET_WM_NAME`).
//! No access to the game process. PlanetSide 2 runs under Proton as an XWayland
//! window (`WM_CLASS steam_app_218230`), so this is the one reliable signal.
//!
//! Fail open: the crosshair is only hidden when an X window is active that is
//! clearly not the game. No X server, an unreadable property or "no active X
//! window" (a native Wayland window has the focus, or nothing) count as
//! [`FocusState::Unknown`], and the crosshair stays visible.

use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

use crate::engine::state;
use crate::{windows, x11};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FocusState {
    /// The active X window matches the game pattern.
    Game,
    /// Another X window is active.
    Other,
    /// Not determinable: never hides the crosshair.
    #[default]
    Unknown,
}

impl FocusState {
    pub fn to_u8(self) -> u8 {
        match self {
            Self::Game => 1,
            Self::Other => 2,
            Self::Unknown => 0,
        }
    }
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Game,
            2 => Self::Other,
            _ => Self::Unknown,
        }
    }
}

/// What the X server reports about the active window.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Active {
    pub window: u32,
    pub class: String,
    pub title: String,
    /// Process name (Linux: from /proc/<_NET_WM_PID>/cmdline, read only; empty
    /// in Flatpak, where host PIDs are not visible; Windows: image file name).
    pub process: String,
}

/// The game-window rule plus the advanced pattern.
#[derive(Debug, Clone, Default)]
pub struct GameMatch {
    pub rule: ps2_tracker_core::config::GameWindowRule,
    pub pattern: String,
}

impl GameMatch {
    pub fn from_cfg(c: &ps2_tracker_core::Config) -> Self {
        Self {
            rule: c.crosshair.game_window.clone(),
            pattern: c.crosshair.game_match.clone(),
        }
    }

    /// Auto: PlanetSide 2 by WM_CLASS (Proton), title or process name, or the
    /// advanced pattern. With a rule: every non-empty field must match.
    pub fn is_game(&self, a: &Active) -> bool {
        let has = |hay: &str, needle: &str| hay.to_lowercase().contains(&needle.to_lowercase());
        let r = &self.rule;
        if r.is_auto() {
            return has(&a.class, "steam_app_218230")
                || has(&a.title, "planetside 2")
                || a.process.to_lowercase().starts_with("planetside2")
                || matches(&self.pattern, &a.class, &a.title);
        }
        (r.class.is_empty() || has(&a.class, &r.class))
            && (r.title.is_empty() || has(&a.title, &r.title))
            && (r.process.is_empty() || a.process.eq_ignore_ascii_case(&r.process))
    }
}

/// Pure decision, see the module comment. `None` = query failed.
pub fn classify(active: Option<&Active>, m: &GameMatch) -> FocusState {
    let Some(a) = active else {
        return FocusState::Unknown;
    };
    if a.window == 0 || (a.class.is_empty() && a.title.is_empty()) {
        return FocusState::Unknown;
    }
    // our own windows (main window while adjusting the crosshair) do not hide it
    if is_own(a) {
        return FocusState::Unknown;
    }
    if m.is_game(a) {
        FocusState::Game
    } else {
        FocusState::Other
    }
}

/// A window of this app (WM_CLASS "ps2-tracker", Windows: ps2-tracker.exe).
pub fn is_own(a: &Active) -> bool {
    a.class.to_lowercase().contains("ps2-tracker")
        || a.process.eq_ignore_ascii_case("ps2-tracker.exe")
}

/// Process name from /proc (read only): the file name of argv[0] (under Proton
/// e.g. `PlanetSide2_x64.exe`), else `comm`. Not in Flatpak: the sandbox has
/// its own PID namespace, the host PID would name a different process.
#[cfg(target_os = "linux")]
fn process_name(pid: u32) -> String {
    if pid == 0 || std::path::Path::new("/.flatpak-info").exists() {
        return String::new();
    }
    let cmd = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    let argv0 = cmd.split(|b| *b == 0).next().unwrap_or_default();
    let argv0 = String::from_utf8_lossy(argv0);
    let base = argv0
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim()
        .to_owned();
    if !base.is_empty() {
        return base;
    }
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct FocusInfo {
    pub available: bool,
    pub class: String,
    pub title: String,
    pub matches: bool,
    pub error: String,
    /// Result of this probe.
    pub state: FocusState,
    /// What the background watcher currently uses.
    pub watcher: FocusState,
    /// GTK backend of the app windows: "x11" or "wayland".
    pub backend: String,
}

/// Linux: `_NET_ACTIVE_WINDOW` and its `WM_CLASS`/`_NET_WM_NAME` from the X server.
#[cfg(target_os = "linux")]
struct X {
    conn: x11rb::rust_connection::RustConnection,
    root: x11rb::protocol::xproto::Window,
    active: u32,
    utf8: u32,
    wm_name: u32,
}

#[cfg(target_os = "linux")]
impl X {
    fn connect() -> Result<Self, String> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::ConnectionExt;
        let (conn, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let root = conn.setup().roots[screen].root;
        let atom = |n: &str| -> Result<u32, String> {
            Ok(conn
                .intern_atom(false, n.as_bytes())
                .map_err(|e| e.to_string())?
                .reply()
                .map_err(|e| e.to_string())?
                .atom)
        };
        let active = atom("_NET_ACTIVE_WINDOW")?;
        let utf8 = atom("UTF8_STRING")?;
        let wm_name = atom("_NET_WM_NAME")?;
        Ok(Self {
            conn,
            root,
            active,
            utf8,
            wm_name,
        })
    }

    fn prop(&self, win: x11rb::protocol::xproto::Window, prop: u32, ty: u32) -> Option<Vec<u8>> {
        use x11rb::protocol::xproto::ConnectionExt;
        self.conn
            .get_property(false, win, prop, ty, 0, 1024)
            .ok()?
            .reply()
            .ok()
            .map(|r| r.value)
    }

    fn query(&self) -> Option<Active> {
        use x11rb::protocol::xproto::AtomEnum;
        let v = self.prop(self.root, self.active, AtomEnum::WINDOW.into())?;
        let window = u32::from_ne_bytes(v.get(0..4)?.try_into().ok()?);
        if window == 0 {
            return Some(Active::default());
        }
        Some(self.info(window))
    }

    /// All managed windows (`_NET_CLIENT_LIST`).
    fn list(&self) -> Vec<Active> {
        use x11rb::protocol::xproto::AtomEnum;
        let Some(atom) = self.atom("_NET_CLIENT_LIST") else {
            return Vec::new();
        };
        self.prop(self.root, atom, AtomEnum::WINDOW.into())
            .unwrap_or_default()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_ne_bytes(*c))
            .map(|w| self.info(w))
            .collect()
    }

    fn atom(&self, name: &str) -> Option<u32> {
        use x11rb::protocol::xproto::ConnectionExt;
        Some(
            self.conn
                .intern_atom(false, name.as_bytes())
                .ok()?
                .reply()
                .ok()?
                .atom,
        )
    }

    fn info(&self, window: u32) -> Active {
        use x11rb::protocol::xproto::AtomEnum;
        let class = self
            .prop(window, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())
            .map(|b| {
                b.split(|c| *c == 0)
                    .filter(|s| !s.is_empty())
                    .map(|s| String::from_utf8_lossy(s).into_owned())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let title = self
            .prop(window, self.wm_name, self.utf8)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        let pid = self
            .atom("_NET_WM_PID")
            .and_then(|a| self.prop(window, a, AtomEnum::CARDINAL.into()))
            .and_then(|v| {
                v.get(0..4)
                    .and_then(|b| b.try_into().ok())
                    .map(u32::from_ne_bytes)
            })
            .unwrap_or(0);
        Active {
            window,
            class,
            title,
            process: process_name(pid),
        }
    }
}

/// Windows: the foreground window and the file name of its process
/// (`PlanetSide2_x64.exe`). The crate opens the process only with
/// PROCESS_QUERY_LIMITED_INFORMATION to read the image name – nothing else.
#[cfg(windows)]
struct X;

#[cfg(windows)]
impl X {
    fn connect() -> Result<Self, String> {
        Ok(Self)
    }

    fn query(&self) -> Option<Active> {
        let w = active_win_pos_rs::get_active_window().ok()?;
        let class = w
            .process_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Some(Active {
            window: u32::try_from(w.process_id).unwrap_or(1).max(1),
            process: class.clone(),
            class,
            title: w.title,
        })
    }

    /// Only the foreground window: listing all windows needs EnumWindows,
    /// i.e. unsafe FFI with a callback, which this crate keeps out (unsafe
    /// code is denied outside win_console.rs).
    fn list(&self) -> Vec<Active> {
        self.query().into_iter().collect()
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
struct X;

#[cfg(not(any(target_os = "linux", windows)))]
impl X {
    fn connect() -> Result<Self, String> {
        Err("not supported on this platform".into())
    }
    fn query(&self) -> Option<Active> {
        None
    }
    fn list(&self) -> Vec<Active> {
        Vec::new()
    }
}

/// Centre of the game window in physical pixels (root coordinates), to open
/// the move layer on the game's monitor. `None` if the game is not found.
#[cfg(target_os = "linux")]
pub fn game_window_center(m: &GameMatch) -> Option<(i32, i32)> {
    use x11rb::protocol::xproto::ConnectionExt;
    let x = X::connect().ok()?;
    let game = x.list().into_iter().find(|a| m.is_game(a))?;
    let g = x.conn.get_geometry(game.window).ok()?.reply().ok()?;
    let t = x
        .conn
        .translate_coordinates(game.window, x.root, 0, 0)
        .ok()?
        .reply()
        .ok()?;
    Some((
        i32::from(t.dst_x) + i32::from(g.width) / 2,
        i32::from(t.dst_y) + i32::from(g.height) / 2,
    ))
}

/// Windows: only when the game is the foreground window.
#[cfg(windows)]
pub fn game_window_center(m: &GameMatch) -> Option<(i32, i32)> {
    let w = active_win_pos_rs::get_active_window().ok()?;
    let a = X.query()?;
    if !m.is_game(&a) {
        return None;
    }
    let p = w.position;
    Some(((p.x + p.width / 2.0) as i32, (p.y + p.height / 2.0) as i32))
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn game_window_center(_m: &GameMatch) -> Option<(i32, i32)> {
    None
}

pub fn matches(pattern: &str, class: &str, title: &str) -> bool {
    let c = class.to_lowercase();
    let t = title.to_lowercase();
    pattern
        .split('|')
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .any(|p| c.contains(&p) || t.contains(&p))
}

/// One-off query (for the settings page).
/// Entry of the game-window list in the UI.
#[derive(Debug, Clone, Serialize)]
pub struct WindowEntry {
    pub class: String,
    pub title: String,
    pub process: String,
    pub matches: bool,
}

/// Open X11/XWayland windows (Windows: the foreground window), for the
/// "Game window" selection.
pub fn list_windows(m: &GameMatch) -> Vec<WindowEntry> {
    let Ok(x) = X::connect() else {
        return Vec::new();
    };
    let mut v: Vec<WindowEntry> = x
        .list()
        .into_iter()
        .filter(|a| !a.class.is_empty() || !a.title.is_empty())
        .map(|a| WindowEntry {
            matches: m.is_game(&a),
            class: a.class,
            title: a.title,
            process: a.process,
        })
        .collect();
    v.sort_by(|a, b| b.matches.cmp(&a.matches).then(a.title.cmp(&b.title)));
    v
}

#[derive(Debug, Clone, Serialize)]
pub struct GameStatus {
    /// "active" | "background" | "not_running" | "unknown"
    pub state: String,
    pub title: String,
}

pub fn game_status(m: &GameMatch) -> GameStatus {
    let Ok(x) = X::connect() else {
        return GameStatus {
            state: "unknown".into(),
            title: String::new(),
        };
    };
    let active = x.query();
    if let Some(a) = active.as_ref().filter(|a| a.window != 0 && m.is_game(a)) {
        return GameStatus {
            state: "active".into(),
            title: a.title.clone(),
        };
    }
    match x.list().into_iter().find(|a| m.is_game(a)) {
        Some(a) => GameStatus {
            state: "background".into(),
            title: a.title,
        },
        None => GameStatus {
            state: if cfg!(windows) {
                "unknown"
            } else {
                "not_running"
            }
            .into(),
            title: String::new(),
        },
    }
}

pub fn probe(app: &AppHandle, m: &GameMatch) -> FocusInfo {
    let watcher = FocusState::from_u8(state(app).focus_state.load(Ordering::Relaxed));
    let backend = x11::backend_name().to_string();
    match X::connect() {
        Ok(x) => match x.query() {
            Some(a) => FocusInfo {
                available: true,
                matches: m.is_game(&a),
                state: classify(Some(&a), m),
                error: if a.window == 0 {
                    "no active X window (a Wayland window or nothing has the focus)".into()
                } else {
                    String::new()
                },
                class: a.class,
                title: a.title,
                watcher,
                backend,
            },
            None => FocusInfo {
                error: "_NET_ACTIVE_WINDOW not readable".into(),
                watcher,
                backend,
                ..Default::default()
            },
        },
        Err(e) => FocusInfo {
            error: e,
            watcher,
            backend,
            ..Default::default()
        },
    }
}

fn publish(app: &AppHandle, new: FocusState, last: &mut Option<FocusState>, a: Option<&Active>) {
    if *last == Some(new) {
        return;
    }
    let (class, title) = a.map_or(("", ""), |a| (a.class.as_str(), a.title.as_str()));
    tracing::info!(state = ?new, previous = ?last, class, title, "game focus changed");
    *last = Some(new);
    state(app).focus_state.store(new.to_u8(), Ordering::Relaxed);
    let h = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Err(e) = windows::apply_crosshair(&h, "focus watcher") {
            tracing::warn!(error = %e, "crosshair update failed");
        }
        if let Err(e) = windows::apply_overlay(&h, "focus watcher") {
            tracing::warn!(error = %e, "overlay update failed");
        }
    });
}

/// Background thread: checks every 300 ms while "auto hide" is enabled.
pub fn spawn_watcher(app: &AppHandle) {
    let app = app.clone();
    std::thread::Builder::new()
        .name("focus-watch".into())
        .spawn(move || {
            let mut x: Option<X> = None;
            let mut last: Option<FocusState> = None;
            let mut last_error = String::new();
            loop {
                std::thread::sleep(Duration::from_millis(300));
                let st = state(&app);
                let (auto, pattern) = {
                    let c = st.cfg();
                    (
                        (c.crosshair.auto_hide && c.crosshair.enabled)
                            || (c.overlay.auto_hide && c.overlay.enabled),
                        GameMatch::from_cfg(&c),
                    )
                };
                if !auto {
                    if last.take().is_some() {
                        st.focus_state
                            .store(FocusState::Unknown.to_u8(), Ordering::Relaxed);
                    }
                    continue;
                }
                if x.is_none() {
                    match X::connect() {
                        Ok(c) => {
                            tracing::info!("focus watcher connected to the X server");
                            last_error.clear();
                            x = Some(c);
                        }
                        Err(e) => {
                            if e != last_error {
                                tracing::warn!(error = %e, "focus watcher: no X server, crosshair stays visible");
                                last_error = e;
                            }
                            publish(&app, FocusState::Unknown, &mut last, None);
                            std::thread::sleep(Duration::from_secs(5));
                            continue;
                        }
                    }
                }
                let active = x.as_ref().and_then(X::query);
                if active.is_none() {
                    tracing::debug!("focus watcher: query failed, reconnecting");
                    x = None;
                }
                let new = classify(active.as_ref(), &pattern);
                publish(&app, new, &mut last, active.as_ref());
            }
        })
        .expect("focus thread starts");
}

#[cfg(test)]
mod tests {
    use super::{Active, FocusState, GameMatch, classify, matches};
    use ps2_tracker_core::config::GameWindowRule;

    const P: &str = "planetside2|steam_app_218230|PlanetSide 2";

    fn auto() -> GameMatch {
        GameMatch {
            rule: GameWindowRule::default(),
            pattern: String::new(),
        }
    }

    #[test]
    fn auto_detects_planetside_and_rules_match_stably() {
        let m = auto();
        let ps2 = Active {
            window: 7,
            class: "steam_app_218230 steam_app_218230".into(),
            title: "PlanetSide 2".into(),
            process: "PlanetSide2_x64.exe".into(),
        };
        assert!(m.is_game(&ps2));
        // only the process name (e.g. title not set yet)
        assert!(m.is_game(&Active {
            window: 7,
            process: "PlanetSide2_x64.exe".into(),
            class: "x".into(),
            ..Default::default()
        }));
        let konsole = active(3, "konsole org.kde.konsole", "bash");
        assert!(!m.is_game(&konsole));
        // a saved rule: every non-empty field must match, case-insensitive,
        // independent of the window id (survives a game restart)
        let rule = GameMatch {
            rule: GameWindowRule {
                class: "STEAM_APP_218230".into(),
                title: "".into(),
                process: "planetside2_x64.exe".into(),
            },
            pattern: P.into(),
        };
        let restarted = Active {
            window: 99,
            ..ps2.clone()
        };
        assert!(rule.is_game(&restarted));
        let other_game = Active {
            class: "steam_app_1".into(),
            ..ps2
        };
        assert!(
            !rule.is_game(&other_game),
            "pattern does not apply when a rule is set"
        );
        // advanced pattern still works in auto mode
        let custom = GameMatch {
            rule: GameWindowRule::default(),
            pattern: "mygame".into(),
        };
        assert!(custom.is_game(&active(5, "mygame mygame", "")));
    }

    #[test]
    fn pattern_matching() {
        assert!(matches(P, "steam_app_218230 steam_app_218230", ""));
        assert!(matches(P, "", "PlanetSide 2 (DX11)"));
        assert!(!matches(P, "konsole org.kde.konsole", "bash"));
        assert!(!matches("", "x", "y"));
    }

    fn active(window: u32, class: &str, title: &str) -> Active {
        Active {
            window,
            class: class.into(),
            title: title.into(),
            process: String::new(),
        }
    }

    #[test]
    fn game_and_other_windows() {
        let game = active(
            0x1c0_0007,
            "steam_app_218230 steam_app_218230",
            "PlanetSide 2",
        );
        assert_eq!(classify(Some(&game), &auto()), FocusState::Game);
        let konsole = active(0x200_0003, "konsole org.kde.konsole", "bash");
        assert_eq!(classify(Some(&konsole), &auto()), FocusState::Other);
    }

    /// Focus on the tracker's own window keeps the crosshair visible (preview
    /// while adjusting it).
    #[test]
    fn own_window_does_not_count_as_other() {
        let me = active(9, "ps2-tracker Ps2-tracker", "PS2 Session Tracker");
        assert_eq!(classify(Some(&me), &auto()), FocusState::Unknown);
    }

    /// Uncertain detection must never hide the crosshair.
    #[test]
    fn uncertain_detection_fails_open() {
        // query failed / no X server
        assert_eq!(classify(None, &auto()), FocusState::Unknown);
        // KWin under Wayland: a native Wayland window is active → no X window
        assert_eq!(
            classify(Some(&Active::default()), &auto()),
            FocusState::Unknown
        );
        // window without class and title
        assert_eq!(
            classify(Some(&active(42, "", "")), &auto()),
            FocusState::Unknown
        );
        for s in [FocusState::Game, FocusState::Unknown] {
            assert!(crate::windows::crosshair_wanted(true, true, true, s));
        }
        assert!(!crate::windows::crosshair_wanted(
            true,
            true,
            true,
            FocusState::Other
        ));
        // auto hide off: focus does not matter
        assert!(crate::windows::crosshair_wanted(
            true,
            true,
            false,
            FocusState::Other
        ));
        assert!(!crate::windows::crosshair_wanted(
            false,
            true,
            false,
            FocusState::Game
        ));
        assert!(!crate::windows::crosshair_wanted(
            true,
            false,
            true,
            FocusState::Game
        ));
    }

    #[test]
    fn state_roundtrip() {
        for s in [FocusState::Game, FocusState::Other, FocusState::Unknown] {
            assert_eq!(FocusState::from_u8(s.to_u8()), s);
        }
    }
}
