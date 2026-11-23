//! Global hotkeys (X11/XWayland) and equivalent command line switches
//! (for KDE shortcuts under Wayland, see README).

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::engine::state;
use crate::windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ToggleOverlay,
    ToggleEdit,
    ToggleCrosshair,
    NextCrosshair,
    ShowMain,
    /// Move mode only (temporary hotkeys): next object, move, end.
    MoveSelectNext,
    MoveBy(i32, i32),
    /// Save and close the move layer.
    MoveEnd,
    /// Close the move layer, keep the old positions.
    MoveCancel,
}

pub const CLI_FLAGS: &[(&str, Action)] = &[
    ("--toggle-overlay", Action::ToggleOverlay),
    ("--toggle-edit", Action::ToggleEdit),
    ("--finish-move", Action::MoveEnd),
    ("--toggle-crosshair", Action::ToggleCrosshair),
    ("--next-crosshair", Action::NextCrosshair),
    ("--show", Action::ShowMain),
];

pub const HELP: &str = "\
PS2 Session Tracker - session stats, overlay, crosshair and voice packs for PlanetSide 2.
Uses only the public Daybreak Census API; it does not touch the game process.

Usage: ps2-tracker [OPTION]

Options:
  --hidden             start minimized to the tray
  --show               show the main window
  --toggle-overlay     show/hide the overlay
  --toggle-edit        start/end the overlay move mode
  --finish-move        end the move mode and save (does nothing outside it)
  --toggle-crosshair   show/hide the crosshair
  --next-crosshair     switch to the next crosshair in the library
  --debug-console      debug mode: log level debug, log in a console window
                       (Windows) or in this terminal
  -V, --version        show the version
  -h, --help           show this help

If the app is already running, the option is forwarded to the running instance.
Bind these commands to KDE shortcuts for hotkeys under Wayland.

Environment:
  PS2_SERVICE_ID       Census service ID (overrides the configuration file)
  RUST_LOG             log level, e.g. info or debug
";

impl Action {
    pub fn describe(self) -> &'static str {
        match self {
            Action::ToggleOverlay => "toggle overlay",
            Action::ToggleEdit => "toggle overlay move mode",
            Action::MoveSelectNext => "move mode: next object",
            Action::MoveBy(..) => "move mode: move object",
            Action::MoveEnd => "move mode: save",
            Action::MoveCancel => "move mode: cancel",
            Action::ToggleCrosshair => "toggle crosshair",
            Action::NextCrosshair => "next crosshair",
            Action::ShowMain => "show main window",
        }
    }
}

pub fn from_args(args: &[String]) -> Vec<Action> {
    args.iter()
        .filter_map(|a| CLI_FLAGS.iter().find(|(f, _)| f == a).map(|(_, x)| *x))
        .collect()
}

/// Runs an action. `via` says where it came from ("hotkey", "tray", "cli"),
/// for the log.
pub fn run(app: &AppHandle, a: Action, via: &'static str) {
    let res = match a {
        Action::ToggleOverlay => windows::toggle_overlay(app).map(|_| ()),
        Action::ToggleEdit => {
            let edit = state(app)
                .overlay_edit
                .load(std::sync::atomic::Ordering::Relaxed);
            windows::set_overlay_edit(app, !edit, via)
        }
        Action::ToggleCrosshair => windows::toggle_crosshair(app),
        Action::NextCrosshair => windows::next_crosshair(app),
        Action::ShowMain => {
            windows::show_main(app);
            Ok(())
        }
        Action::MoveSelectNext => {
            windows::select_next(app);
            Ok(())
        }
        Action::MoveBy(dx, dy) => windows::nudge(app, windows::move_selected(app), dx, dy),
        Action::MoveEnd => windows::end_move(app, true, via),
        Action::MoveCancel => windows::end_move(app, false, via),
    };
    if let Err(e) = res {
        tracing::warn!(?a, error = %e, "action failed");
    }
}

/// Temporary global hotkeys while the move mode is on (the game may keep the
/// keyboard focus or the pointer): Tab = next object, arrows = 1 px,
/// Shift+arrows = 10 px, Enter = save, Esc = cancel.
pub const MOVE_KEYS: &[(&str, Action)] = &[
    ("Tab", Action::MoveSelectNext),
    ("ArrowLeft", Action::MoveBy(-1, 0)),
    ("ArrowRight", Action::MoveBy(1, 0)),
    ("ArrowUp", Action::MoveBy(0, -1)),
    ("ArrowDown", Action::MoveBy(0, 1)),
    ("Shift+ArrowLeft", Action::MoveBy(-10, 0)),
    ("Shift+ArrowRight", Action::MoveBy(10, 0)),
    ("Shift+ArrowUp", Action::MoveBy(0, -10)),
    ("Shift+ArrowDown", Action::MoveBy(0, 10)),
    ("Enter", Action::MoveEnd),
    ("Escape", Action::MoveCancel),
];

fn bindings(app: &AppHandle) -> Vec<(String, Action)> {
    let st = state(app);
    let c = st.cfg().clone();
    let mut v = vec![
        (c.overlay.hotkey_toggle, Action::ToggleOverlay),
        (c.overlay.hotkey_edit, Action::ToggleEdit),
        (c.crosshair.hotkey_toggle, Action::ToggleCrosshair),
        (c.crosshair.hotkey_cycle, Action::NextCrosshair),
    ];
    if st.overlay_edit.load(std::sync::atomic::Ordering::Relaxed) {
        // the finish key stays registered for the whole session, also while
        // the game holds the pointer (the game then gets no Esc from it)
        v.push((c.overlay.hotkey_finish_move.clone(), Action::MoveEnd));
        // while the game holds the pointer, Esc must reach the game (it frees
        // the cursor there); it becomes "cancel" once the pointer is free
        let grabbed = st
            .move_session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|s| s.pointer_free == Some(false));
        v.extend(
            MOVE_KEYS
                .iter()
                .filter(|(k, _)| !(grabbed && *k == "Escape"))
                .map(|(k, a)| ((*k).to_string(), *a)),
        );
    }
    v
}

/// Shortcuts pressed and not yet released (by id). See [`wait_released`].
static HELD: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());

fn held() -> std::sync::MutexGuard<'static, Vec<u32>> {
    HELD.lock().unwrap_or_else(|e| e.into_inner())
}

/// Re-registering while a hotkey is still held deadlocks: the plugin keeps its
/// shortcut lock on the main thread during unregister_all and waits for the
/// X11 hotkey thread, which is just delivering the key release and waits for
/// the same lock (seen as a hang after Enter/Ctrl+Esc ended the move mode,
/// backtrace in the E2E test). So wait for the release first, at most 1.5 s.
fn wait_released() {
    let t0 = std::time::Instant::now();
    while !held().is_empty() {
        if t0.elapsed() > std::time::Duration::from_millis(1500) {
            tracing::debug!("hotkey release not seen, registering anyway");
            held().clear();
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Plugin handler: maps the pressed shortcut to an action.
pub fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, ev_state: ShortcutState) {
    if ev_state == ShortcutState::Released {
        held().retain(|&id| id != shortcut.id());
        return;
    }
    held().push(shortcut.id());
    for (s, a) in bindings(app) {
        if s.parse::<Shortcut>().is_ok_and(|p| p == *shortcut) {
            tracing::debug!(hotkey = %s, action = a.describe(), "hotkey pressed");
            // never work inside the hotkey callback: it runs on the hotkey
            // thread, window calls wait for the main thread, and the main
            // thread may wait for the hotkey thread when shortcuts are
            // re-registered (seen as a hang when Enter ended the move mode)
            let app = app.clone();
            std::thread::spawn(move || run(&app, a, "hotkey"));
        }
    }
}

/// (Re)registration. If it fails (e.g. pure Wayland, or the desktop holds the
/// combination: KWin "Switch to Desktop 2" on Ctrl+F2), the app stays usable;
/// the failures are kept for the UI and the move layer.
pub fn register(app: &AppHandle) {
    wait_released();
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut failed = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (s, a) in bindings(app) {
        if s.trim().is_empty() || !seen.insert(s.clone()) {
            continue;
        }
        let res = s
            .parse::<Shortcut>()
            .map_err(|e| format!("invalid: {e}"))
            .and_then(|sc| gs.register(sc).map_err(|e| e.to_string()));
        match res {
            Ok(()) => tracing::debug!(hotkey = %s, action = a.describe(), "hotkey registered ok"),
            Err(reason) => {
                tracing::warn!(hotkey = %s, action = a.describe(), %reason, "hotkey registered failed");
                failed.push((s, a.describe(), reason));
            }
        }
    }
    *state(app)
        .hotkey_failures
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = failed;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_flags() {
        let a = from_args(&[
            "ps2-tracker".into(),
            "--toggle-crosshair".into(),
            "--x".into(),
        ]);
        assert_eq!(a, vec![Action::ToggleCrosshair]);
    }

    #[test]
    fn move_keys_parse() {
        for (k, _) in MOVE_KEYS {
            assert!(k.parse::<Shortcut>().is_ok(), "{k}");
        }
    }

    #[test]
    fn default_hotkeys_parse() {
        let c = ps2_tracker_core::Config::default();
        for s in [
            c.overlay.hotkey_edit,
            c.overlay.hotkey_finish_move,
            c.overlay.hotkey_toggle,
            c.crosshair.hotkey_toggle,
            c.crosshair.hotkey_cycle,
        ] {
            assert!(s.parse::<Shortcut>().is_ok(), "{s}");
        }
    }
}
