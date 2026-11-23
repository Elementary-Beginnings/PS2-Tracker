//! X11 window type for our own overlay windows (never for the game window).
//!
//! KWin stacks windows by layer (`belongsToLayer()` in kwin `src/window.cpp`,
//! order in `src/effect/globals.h`):
//! `Above` < `Notification` < `Active` (active fullscreen) < `Popup` <
//! `CriticalNotification` < `OnScreenDisplay` < `Overlay`.
//! "Keep above" (`_NET_WM_STATE_ABOVE`) and `_NET_WM_WINDOW_TYPE_NOTIFICATION` are
//! therefore *below* an active fullscreen game. `_KDE_NET_WM_WINDOW_TYPE_ON_SCREEN_DISPLAY`
//! puts the window into the `OnScreenDisplay` layer, above it.
//! `_NET_WM_WINDOW_TYPE_NOTIFICATION` follows as a fallback for window managers
//! that do not know the KDE type (EWMH: the first known type wins).
//!
//! KWin reads the type only when it manages a window (`propertyNotifyEvent` in
//! kwin `src/events.cpp` ignores later changes), so the window is unmapped and
//! mapped again after the change – directly on the X server: a GTK hide/show would
//! write `_NET_WM_WINDOW_TYPE_NORMAL` back on show (seen with xprop).
//! Only possible when GTK runs on X11/XWayland.

#[cfg(target_os = "linux")]
use x11rb::connection::Connection;
#[cfg(target_os = "linux")]
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode, Window};
#[cfg(target_os = "linux")]
use x11rb::wrapper::ConnectionExt as _;

/// Are the GTK windows X11 windows (pure X11 session or `GDK_BACKEND=x11`)?
pub fn is_x11_backend() -> bool {
    cfg!(target_os = "linux")
        && backend_from_env(
            std::env::var("GDK_BACKEND").ok().as_deref(),
            std::env::var_os("WAYLAND_DISPLAY").is_some(),
            std::env::var_os("DISPLAY").is_some(),
        ) == "x11"
}

pub fn backend_name() -> &'static str {
    if cfg!(windows) {
        return "win32";
    }
    backend_from_env(
        std::env::var("GDK_BACKEND").ok().as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        std::env::var_os("DISPLAY").is_some(),
    )
}

/// GDK picks Wayland first when `WAYLAND_DISPLAY` is set, unless `GDK_BACKEND` says otherwise.
pub fn backend_from_env(gdk_backend: Option<&str>, wayland: bool, display: bool) -> &'static str {
    match gdk_backend.map(|b| b.split(',').next().unwrap_or("").trim()) {
        Some("x11") => "x11",
        Some("wayland") => "wayland",
        _ if wayland => "wayland",
        _ if display => "x11",
        _ => "none",
    }
}

#[cfg(target_os = "linux")]
fn atom(conn: &impl Connection, name: &str) -> Result<u32, String> {
    Ok(conn
        .intern_atom(false, name.as_bytes())
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?
        .atom)
}

#[cfg(target_os = "linux")]
fn prop32(conn: &impl Connection, win: Window, prop: u32, ty: u32) -> Vec<u32> {
    conn.get_property(false, win, prop, ty, 0, 4096)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| r.value32().map(|v| v.collect()))
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
/// Finds our managed window with this title (`_NET_CLIENT_LIST`, `_NET_WM_PID`).
fn find_own_window(
    conn: &impl Connection,
    root: Window,
    title: &str,
) -> Result<Option<Window>, String> {
    let client_list = atom(conn, "_NET_CLIENT_LIST")?;
    let pid_atom = atom(conn, "_NET_WM_PID")?;
    let name_atom = atom(conn, "_NET_WM_NAME")?;
    let utf8 = atom(conn, "UTF8_STRING")?;
    let pid = std::process::id();
    for w in prop32(conn, root, client_list, AtomEnum::WINDOW.into()) {
        if prop32(conn, w, pid_atom, AtomEnum::CARDINAL.into()).first() != Some(&pid) {
            continue;
        }
        let name = conn
            .get_property(false, w, name_atom, utf8, 0, 1024)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| String::from_utf8_lossy(&r.value).into_owned())
            .unwrap_or_default();
        if name == title {
            return Ok(Some(w));
        }
    }
    Ok(None)
}

/// Sets the window type of our window `title`: on-screen display (above
/// fullscreen) or normal. Waits up to 3 s until the window manager lists it.
/// Returns the X window id.
#[cfg(not(target_os = "linux"))]
pub fn set_window_type(_title: &str, _osd: bool) -> Result<u32, String> {
    Err("X11 window types exist only on Linux".into())
}

#[cfg(target_os = "linux")]
pub fn set_window_type(title: &str, osd: bool) -> Result<Window, String> {
    let (conn, screen) = x11rb::connect(None).map_err(|e| format!("X server: {e}"))?;
    let root = conn.setup().roots[screen].root;
    let mut win = None;
    for _ in 0..30 {
        win = find_own_window(&conn, root, title)?;
        if win.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let win = win.ok_or_else(|| format!("window \"{title}\" not in _NET_CLIENT_LIST"))?;
    let types: Vec<u32> = if osd {
        vec![
            atom(&conn, "_KDE_NET_WM_WINDOW_TYPE_ON_SCREEN_DISPLAY")?,
            atom(&conn, "_NET_WM_WINDOW_TYPE_NOTIFICATION")?,
        ]
    } else {
        vec![atom(&conn, "_NET_WM_WINDOW_TYPE_NORMAL")?]
    };
    let type_atom = atom(&conn, "_NET_WM_WINDOW_TYPE")?;
    let x = |r: Result<x11rb::cookie::VoidCookie<'_, _>, _>| -> Result<(), String> {
        r.map_err(|e: x11rb::errors::ConnectionError| e.to_string())?;
        Ok(())
    };
    // the window manager forgets the window state on unmap: keep it, plus
    // above / sticky / skip taskbar and pager (EWMH: set before mapping)
    let state_atom = atom(&conn, "_NET_WM_STATE")?;
    let mut states = prop32(&conn, win, state_atom, AtomEnum::ATOM.into());
    for name in [
        "_NET_WM_STATE_ABOVE",
        "_NET_WM_STATE_STICKY",
        "_NET_WM_STATE_SKIP_TASKBAR",
        "_NET_WM_STATE_SKIP_PAGER",
    ] {
        let a = atom(&conn, name)?;
        if !states.contains(&a) {
            states.push(a);
        }
    }
    x(conn.change_property32(PropMode::REPLACE, win, type_atom, AtomEnum::ATOM, &types))?;
    // remap so the window manager manages it again with the new type
    x(conn.unmap_window(win))?;
    conn.flush().map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(150));
    x(conn.change_property32(PropMode::REPLACE, win, type_atom, AtomEnum::ATOM, &types))?;
    x(conn.change_property32(PropMode::REPLACE, win, state_atom, AtomEnum::ATOM, &states))?;
    x(conn.map_window(win))?;
    conn.flush().map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(150));
    let now = prop32(&conn, win, type_atom, AtomEnum::ATOM.into());
    if now.first() != types.first() {
        return Err(format!("window type was overwritten ({now:?})"));
    }
    Ok(win)
}

/// Result of opening the move layer: did it get the input focus, and is the
/// pointer free (or does another client, i.e. the game in mouse look, hold a
/// pointer grab)? `None` = could not be determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerInput {
    pub focus: Option<bool>,
    pub pointer_free: Option<bool>,
}

#[cfg(not(target_os = "linux"))]
pub fn activate_layer(_title: &str) -> Result<LayerInput, String> {
    Err("X11 only".into())
}

#[cfg(not(target_os = "linux"))]
pub fn layer_pointer_free(_title: &str) -> Option<bool> {
    None
}

/// Raises our move layer and asks for the input focus.
///
/// KWin (checked against kwin `src/`, Nov 2026): an on-screen-display window may
/// take the focus – `X11Window::acceptsFocus()` only looks at the WM_HINTS input
/// flag / WM_TAKE_FOCUS, and `Workspace::activateWindow()` has no window-type
/// check – but KWin does not focus special windows by itself when they are
/// mapped (`if (!isSpecialWindow()) requestFocus`), and a plain
/// `XSetInputFocus()` from a client goes through focus stealing prevention
/// (`X11Window::focusInEvent` → `allowWindowActivation`). A `_NET_ACTIVE_WINDOW`
/// request with source 2 (pager/tool) is activated with `force`
/// (`RootInfo::changeActiveWindow`). That request goes first; `XSetInputFocus`
/// follows only for window managers that ignore it.
#[cfg(target_os = "linux")]
pub fn activate_layer(title: &str) -> Result<LayerInput, String> {
    use x11rb::protocol::xproto::{
        ClientMessageEvent, ConfigureWindowAux, EventMask, InputFocus, StackMode,
    };
    let (conn, screen) = x11rb::connect(None).map_err(|e| format!("X server: {e}"))?;
    let root = conn.setup().roots[screen].root;
    let mut win = None;
    for _ in 0..30 {
        win = find_own_window(&conn, root, title)?;
        if win.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let win = win.ok_or_else(|| format!("window \"{title}\" not in _NET_CLIENT_LIST"))?;
    let e = |r: Result<x11rb::cookie::VoidCookie<'_, _>, x11rb::errors::ConnectionError>| {
        r.map(|_| ()).map_err(|e| e.to_string())
    };
    e(conn.configure_window(win, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE)))?;
    let active = atom(&conn, "_NET_ACTIVE_WINDOW")?;
    // data: source 2 = pager/tool, timestamp 0 = CurrentTime, no requestor
    let ev = ClientMessageEvent::new(32, win, active, [2u32, 0, 0, 0, 0]);
    e(conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        ev,
    ))?;
    conn.flush().map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(250));
    let mut focus = has_focus(&conn, win);
    if focus == Some(false) {
        e(conn.set_input_focus(InputFocus::PARENT, win, x11rb::CURRENT_TIME))?;
        conn.flush().map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(150));
        focus = has_focus(&conn, win);
    }
    Ok(LayerInput {
        focus,
        pointer_free: probe_pointer(&conn, win),
    })
}

/// Is the pointer free for our move layer (no grab by another client)?
#[cfg(target_os = "linux")]
pub fn layer_pointer_free(title: &str) -> Option<bool> {
    let (conn, screen) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots[screen].root;
    let win = find_own_window(&conn, root, title).ok()??;
    probe_pointer(&conn, win)
}

/// Does `win` (or a child of it, GTK may focus a child) hold the input focus?
#[cfg(target_os = "linux")]
fn has_focus(conn: &impl Connection, win: Window) -> Option<bool> {
    let mut f = conn.get_input_focus().ok()?.reply().ok()?.focus;
    for _ in 0..16 {
        if f == win {
            return Some(true);
        }
        if f <= 1 {
            return Some(false);
        }
        let t = conn.query_tree(f).ok()?.reply().ok()?;
        if t.parent == 0 || t.parent == t.root {
            return Some(false);
        }
        f = t.parent;
    }
    Some(false)
}

/// Tries to grab the pointer for a moment and releases it at once. Another
/// client holding an active grab (a game in mouse look) makes the request fail
/// with `AlreadyGrabbed`; that is the only way an X client can see it.
#[cfg(target_os = "linux")]
fn probe_pointer(conn: &impl Connection, win: Window) -> Option<bool> {
    use x11rb::protocol::xproto::{EventMask, GrabMode, GrabStatus};
    let r = conn
        .grab_pointer(
            false,
            win,
            EventMask::NO_EVENT,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
            x11rb::NONE,
            x11rb::NONE,
            x11rb::CURRENT_TIME,
        )
        .ok()?
        .reply()
        .ok()?;
    match r.status {
        GrabStatus::SUCCESS => {
            let _ = conn.ungrab_pointer(x11rb::CURRENT_TIME);
            let _ = conn.flush();
            Some(true)
        }
        GrabStatus::ALREADY_GRABBED | GrabStatus::FROZEN => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::backend_from_env;

    #[test]
    fn backend_detection() {
        assert_eq!(backend_from_env(None, true, true), "wayland");
        assert_eq!(backend_from_env(Some("x11"), true, true), "x11");
        assert_eq!(backend_from_env(Some("wayland,x11"), true, true), "wayland");
        assert_eq!(backend_from_env(None, false, true), "x11");
        assert_eq!(backend_from_env(None, false, false), "none");
    }
}
