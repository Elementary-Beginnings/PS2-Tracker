//! Console for command line output and the debug mode.
//!
//! Windows release builds use the GUI subsystem (`windows_subsystem` in
//! main.rs): starting from the menu or Explorer opens no console window.
//! Started from a terminal, the process attaches to that console, so `--help`,
//! `--version` and the messages of `--toggle-*`/`--finish-move` appear there.
//! The debug mode (`--debug-console` or Settings → Advanced) opens a console
//! window of its own when there is no terminal. The Win32 calls are in
//! `win_console.rs`, the only module with `unsafe` code.
//!
//! Linux: output always goes to the calling terminal (if any); the debug mode
//! only raises the log level.

/// Attaches to the console of the calling terminal, if there is one.
/// Returns whether output is visible somewhere.
pub fn attach_parent() -> bool {
    #[cfg(windows)]
    {
        crate::win_console::attach_parent()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Debug mode: the terminal it was started from, else (Windows) a console
/// window of its own.
pub fn open_debug() -> bool {
    #[cfg(windows)]
    {
        attach_parent() || crate::win_console::alloc_with_title("PS2 Session Tracker – debug log")
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// `--debug-console` on the command line or "Show debug console on start" in
/// the configuration (read early, before logging starts; errors mean "off").
pub fn debug_wanted(args: &[String]) -> bool {
    args.iter().any(|a| a == "--debug-console")
        || ps2_tracker_core::Paths::xdg()
            .ok()
            .and_then(|p| ps2_tracker_core::Config::load(&p.config_file()).ok())
            .is_some_and(|c| c.ui.debug_console)
}

/// Log filter: `RUST_LOG` wins; the debug mode logs the app at debug level.
pub fn log_filter(debug: bool) -> &'static str {
    if debug {
        "info,ps2_tracker=debug,ps2_tracker_core=debug,discord_rich_presence=error"
    } else {
        "info,discord_rich_presence=error"
    }
}

/// Whether a cursor clip rectangle (left, top, right, bottom) leaves the whole
/// virtual screen (x, y, width, height) free, i.e. no game confines the cursor.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn clip_is_free(clip: (i32, i32, i32, i32), screen: (i32, i32, i32, i32)) -> bool {
    let (l, t, r, b) = clip;
    let (x, y, w, h) = screen;
    l <= x && t <= y && r >= x + w && b >= y + h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_and_filter() {
        assert!(debug_wanted(&["x".into(), "--debug-console".into()]));
        assert!(log_filter(true).contains("ps2_tracker=debug"));
        assert!(!log_filter(false).contains("debug"));
    }

    #[test]
    fn clip_rect() {
        // two monitors side by side, the left one at -1920
        let screen = (-1920, 0, 4480, 1440);
        assert!(clip_is_free((-1920, 0, 2560, 1440), screen));
        // a game confines the cursor to its 2560x1440 window, or to a point
        assert!(!clip_is_free((0, 0, 2560, 1440), screen));
        assert!(!clip_is_free((1280, 720, 1281, 721), screen));
    }

    /// `unsafe` is denied workspace-wide; only win_console.rs may allow it and
    /// contain unsafe blocks.
    #[test]
    fn unsafe_only_in_win_console() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let allow = ["allow(", "unsafe_code)"].concat();
        let block = ["unsafe", " {"].concat();
        let unsafe_fn = ["unsafe", " fn"].concat();
        let mut found = Vec::new();
        let mut stack = vec![root.join("src-tauri/src"), root.join("crates")];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    if !p.ends_with("target") {
                        stack.push(p);
                    }
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") || p.ends_with("win_console.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap();
                if text.contains(&allow) || text.contains(&block) || text.contains(&unsafe_fn) {
                    found.push(p.display().to_string());
                }
            }
        }
        assert!(found.is_empty(), "unsafe outside win_console.rs: {found:?}");
        let wc = std::fs::read_to_string(root.join("src-tauri/src/win_console.rs")).unwrap();
        assert_eq!(
            wc.matches(&block).count(),
            wc.matches("// SAFETY:").count(),
            "every unsafe block in win_console.rs needs a SAFETY note"
        );
    }
}
