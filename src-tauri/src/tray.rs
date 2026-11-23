//! Tray icon with the most important switches.

use tauri::AppHandle;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;

use crate::hotkeys::{self, Action};

pub fn create(app: &AppHandle) -> tauri::Result<()> {
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
