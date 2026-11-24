//! Tray icon with the most important switches.
//!
//! Not every desktop shows one: the Linux tray needs libayatana-appindicator3
//! (loaded at run time; the crate panics when it is missing, e.g. in a Flatpak
//! runtime without it) and a StatusNotifierItem host (GNOME Shell has none
//! without an extension and no longer shows the XEmbed fallback). Without a
//! visible tray the main window is the only way back into the app, so closing
//! it quits instead of hiding (see [`visible`] and main.rs).

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;

use crate::hotkeys::{self, Action};

static VISIBLE: AtomicBool = AtomicBool::new(false);

/// Whether a tray icon was created and the desktop can show it.
pub fn visible() -> bool {
    VISIBLE.load(Ordering::Relaxed)
}

/// Creates the tray icon. A missing tray library is an error, not a crash.
pub fn create(app: &AppHandle) -> Result<(), String> {
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build(app)));
    let res = match built {
        Ok(r) => r.map_err(|e| e.to_string()),
        Err(_) => Err("tray library not available (libayatana-appindicator3)".into()),
    };
    let host = host_available();
    VISIBLE.store(res.is_ok() && host, Ordering::Relaxed);
    if res.is_ok() && !host {
        tracing::warn!(
            "no StatusNotifierItem host on this GNOME desktop: the tray icon is not shown \
             (AppIndicator extension missing); closing the main window quits the app"
        );
    }
    res
}

/// GNOME Shell shows tray icons only through an extension that provides
/// `org.kde.StatusNotifierWatcher`; other desktops (KDE, Xfce, Cinnamon …)
/// show either that or the XEmbed fallback. Only GNOME without a watcher is
/// treated as "no tray".
#[cfg(target_os = "linux")]
fn host_available() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if !is_gnome(&desktop) {
        return true;
    }
    let has_watcher = || -> zbus::Result<bool> {
        let conn = zbus::blocking::Connection::session()?;
        let reply = conn.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "NameHasOwner",
            &("org.kde.StatusNotifierWatcher",),
        )?;
        reply.body().deserialize::<bool>()
    };
    // unknown (no session bus …): assume a tray, as before
    has_watcher().unwrap_or(true)
}

#[cfg(not(target_os = "linux"))]
fn host_available() -> bool {
    true
}

/// `XDG_CURRENT_DESKTOP` is a colon-separated list ("ubuntu:GNOME").
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn is_gnome(desktop: &str) -> bool {
    desktop
        .split(':')
        .any(|d| d.eq_ignore_ascii_case("gnome") || d.eq_ignore_ascii_case("gnome-classic"))
}

fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open main window", true, None::<&str>)?;
    let overlay = MenuItem::with_id(app, "overlay", "Toggle overlay", true, None::<&str>)?;
    let edit = MenuItem::with_id(app, "edit", "Move overlay (move mode)", true, None::<&str>)?;
    let finish = MenuItem::with_id(app, "finish", "Finish move mode", true, None::<&str>)?;
    let xh = MenuItem::with_id(app, "crosshair", "Toggle crosshair", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "Next crosshair", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &show, &sep, &overlay, &edit, &finish, &xh, &next, &sep2, &quit,
        ],
    )?;
    let mut b = TrayIconBuilder::with_id("main")
        .tooltip("PS2 Session Tracker")
        .menu(&menu)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "show" => hotkeys::run(app, Action::ShowMain, "tray"),
            "overlay" => hotkeys::run(app, Action::ToggleOverlay, "tray"),
            "edit" => hotkeys::run(app, Action::ToggleEdit, "tray"),
            // saves; does nothing outside the move mode
            "finish" => hotkeys::run(app, Action::MoveEnd, "tray"),
            "crosshair" => hotkeys::run(app, Action::ToggleCrosshair, "tray"),
            "next" => hotkeys::run(app, Action::NextCrosshair, "tray"),
            "quit" => {
                crate::engine::shutdown(app);
                app.exit(0);
            }
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn gnome_desktops() {
        assert!(super::is_gnome("GNOME"));
        assert!(super::is_gnome("ubuntu:GNOME"));
        assert!(super::is_gnome("GNOME-Classic:GNOME"));
        assert!(!super::is_gnome("KDE"));
        assert!(!super::is_gnome("XFCE"));
        assert!(!super::is_gnome(""));
    }
}
