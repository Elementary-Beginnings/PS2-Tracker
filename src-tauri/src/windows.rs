//! Overlay and crosshair windows. Both are our own transparent windows –
//! there is no access to the game.

use std::sync::atomic::Ordering;

use base64::Engine as _;
use ps2_tracker_core::{crosshair, overlay};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::engine::state;
use crate::focus::FocusState;
use crate::state::now_ms;
use crate::x11;
use ps2_tracker_core::config::OverlaySettings;

pub const OVERLAY: &str = "overlay";
pub const POPUP: &str = "popup";
pub const CROSSHAIR: &str = "crosshair";
/// Window titles; the KWin rule in packaging/kde matches them exactly.
pub const OVERLAY_TITLE: &str = "PS2 Session Tracker – Overlay";
pub const POPUP_TITLE: &str = "PS2 Session Tracker – Popup";
pub const CROSSHAIR_TITLE: &str = "PS2 Session Tracker – Crosshair";
/// Move layer: full-monitor window of the move mode (see [`start_move`]).
pub const MOVE_LAYER: &str = "movelayer";
pub const MOVE_LAYER_TITLE: &str = "PS2 Session Tracker – Move";

#[derive(Serialize, Clone)]
pub struct MonitorInfo {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub primary: bool,
}

pub fn monitors(app: &AppHandle) -> Vec<MonitorInfo> {
    let primary = app
        .primary_monitor()
        .ok()
        .flatten()
        .and_then(|m| m.name().cloned());
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| MonitorInfo {
            name: m.name().cloned().unwrap_or_default(),
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            scale_factor: m.scale_factor(),
            primary: m.name().cloned() == primary,
        })
        .collect()
}

fn area_of(m: &Monitor) -> (i32, i32, u32, u32) {
    (
        m.position().x,
        m.position().y,
        m.size().width,
        m.size().height,
    )
}

/// Monitor setting of an object: the pop-up has its own (empty = same as the
/// overlay).
fn monitor_setting(o: &OverlaySettings, obj: Obj) -> &str {
    match obj {
        Obj::Popup if !o.popup_monitor.is_empty() => &o.popup_monitor,
        _ => &o.monitor,
    }
}

fn find_monitor(app: &AppHandle, name: &str) -> Option<Monitor> {
    let all = app.available_monitors().ok()?;
    if !name.is_empty()
        && let Some(m) = all.iter().find(|m| m.name().is_some_and(|n| n == name))
    {
        return Some(m.clone());
    }
    app.primary_monitor()
        .ok()
        .flatten()
        .or_else(|| all.first().cloned())
}

/// The two overlay objects: the stats overlay and the achievement/voice pack
/// pop-up. Both are windows like the crosshair window: not resizable, not
/// focusable, click-through, on-screen-display type under X11/XWayland set
/// once, never changed (a type change needs an unmap/map, after which the
/// window manager may place it anew). Their size is the measured size of their
/// content and only changes with the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Obj {
    Stats,
    Popup,
}

impl Obj {
    pub fn label(self) -> &'static str {
        match self {
            Obj::Stats => OVERLAY,
            Obj::Popup => POPUP,
        }
    }
    pub fn parse(s: &str) -> Option<Obj> {
        match s {
            "stats" | OVERLAY => Some(Obj::Stats),
            "popup" => Some(Obj::Popup),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Obj::Stats => "stats",
            Obj::Popup => "popup",
        }
    }
    fn url(self) -> &'static str {
        match self {
            Obj::Stats => "overlay.html",
            Obj::Popup => "popup.html",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Obj::Stats => OVERLAY_TITLE,
            Obj::Popup => POPUP_TITLE,
        }
    }
    /// (anchor, offset, size) from the settings.
    fn geometry(self, o: &OverlaySettings) -> (String, (i32, i32), (u32, u32)) {
        match self {
            Obj::Stats => (
                o.anchor.clone(),
                (o.x, o.y),
                (o.width.clamp(120, 4000), o.height.clamp(60, 4000)),
            ),
            Obj::Popup => (
                o.popup_anchor.clone(),
                (o.popup_x, o.popup_y),
                (
                    o.popup_width.clamp(40, 4000),
                    o.popup_height.clamp(20, 4000),
                ),
            ),
        }
    }
    fn set_size(self, o: &mut OverlaySettings, size: (u32, u32)) {
        match self {
            Obj::Stats => (o.width, o.height) = size,
            Obj::Popup => (o.popup_width, o.popup_height) = size,
        }
    }
}

fn object_window(app: &AppHandle, obj: Obj) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(obj.label()) {
        return Ok(w);
    }
    let (_, _, size) = obj.geometry(&state(app).cfg().overlay);
    WebviewWindowBuilder::new(app, obj.label(), WebviewUrl::App(obj.url().into()))
        .title(obj.title())
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .visible_on_all_workspaces(true)
        .inner_size(f64::from(size.0), f64::from(size.1))
        .build()
}

/// Size step of the overlay for its monitor (see `overlay::size_factor`).
pub fn overlay_size_factor(app: &AppHandle) -> f32 {
    let o = state(app).cfg().overlay.clone();
    let h = find_monitor(app, &o.monitor).map_or(1440, |m| m.size().height);
    overlay::size_factor(&o.size, h)
}

fn log_overlay(app: &AppHandle, obj: Obj, shown: bool, reason: &str, detail: &str) {
    let st = state(app);
    let mut last = st.overlay_last.lock().unwrap_or_else(|e| e.into_inner());
    let line = format!("{}|{shown}|{detail}", obj.name());
    let slot = last.entry(obj.name()).or_default();
    // showing and hiding the pop-up window per notification is routine
    let routine = reason == "move" || reason.starts_with("pop-up ");
    if *slot != line && !routine {
        tracing::info!(
            object = obj.name(),
            shown,
            reason,
            backend = x11::backend_name(),
            "{detail}"
        );
    } else {
        tracing::debug!(object = obj.name(), shown, reason, "{detail}");
    }
    *slot = line;
}

/// A pop-up starts: map the pop-up window now and hide it again once the
/// pop-up has faded out, unless a newer one came in the meantime. Without
/// this the empty transparent window stayed mapped, and on KWin/XWayland it
/// could keep showing the last frame of the pack image.
pub fn popup_started(app: &AppHandle) {
    let st = state(app);
    let duration = u64::from(st.cfg().overlay.popup_duration_ms);
    let (generation, until) = lock(&st.popup_gate).start(now_ms(), duration);
    if let Err(e) = apply_object(app, Obj::Popup, "pop-up start") {
        tracing::warn!(error = %e, "pop-up window not shown");
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let wait = until.saturating_sub(now_ms());
        tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
        let st = state(&app);
        if lock(&st.popup_gate).expire(generation, now_ms())
            && let Err(e) = apply_object(&app, Obj::Popup, "pop-up end")
        {
            tracing::warn!(error = %e, "pop-up window not hidden");
        }
    });
}

/// Shows both overlay objects at their anchors (always fully on the monitor)
/// or hides them. `reason` goes to the log.
pub fn apply_overlay(app: &AppHandle, reason: &str) -> tauri::Result<()> {
    apply_object(app, Obj::Stats, reason)?;
    apply_object(app, Obj::Popup, reason)
}

/// Same order as the crosshair: size, position, show, keep above,
/// click-through (off only in the move mode), window type once.
pub fn apply_object(app: &AppHandle, obj: Obj, reason: &str) -> tauri::Result<()> {
    let st = state(app);
    let o = st.cfg().overlay.clone();
    let w = object_window(app, obj)?;
    let moving = st.overlay_edit.load(Ordering::Relaxed);
    let focus = FocusState::from_u8(st.focus_state.load(Ordering::Relaxed));
    let hidden = if !o.enabled {
        Some("hidden: overlay disabled")
    } else if obj == Obj::Popup && !o.show_toasts {
        Some("hidden: notifications off")
    } else if obj == Obj::Popup
        && !overlay::popup_window_wanted(lock(&st.popup_gate).active(now_ms()), moving)
    {
        Some("hidden: no pop-up active")
    } else if o.auto_hide && !moving && focus == FocusState::Other {
        Some("hidden: other window focused")
    } else {
        None
    };
    if let Some(why) = hidden {
        log_overlay(app, obj, false, reason, why);
        return w.hide();
    }
    let (anchor, offset, size) = obj.geometry(&o);
    w.set_size(PhysicalSize::new(size.0, size.1))?;
    let mut placed = format!(
        "{} placed: size={}x{}, no monitor",
        obj.name(),
        size.0,
        size.1
    );
    if let Some(m) = find_monitor(app, monitor_setting(&o, obj)) {
        let area = (
            m.position().x,
            m.position().y,
            m.size().width,
            m.size().height,
        );
        let (x, y) = overlay::place(area, size, overlay::Anchor::parse(&anchor), offset);
        w.set_position(PhysicalPosition::new(x, y))?;
        st.overlay_target
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(obj.name(), (x, y));
        placed = format!(
            "{} placed: preset={}, size={}x{}, anchor={anchor} offset {},{}, requested {x},{y}, monitor={} at {},{} {}x{}, focus={focus:?}",
            obj.name(),
            preset_name(&o),
            size.0,
            size.1,
            offset.0,
            offset.1,
            m.name().map_or("?", |s| s.as_str()),
            area.0,
            area.1,
            area.2,
            area.3
        );
    }
    w.show()?;
    w.set_always_on_top(true)?;
    // while the move layer is open the real windows stay click-through (they
    // show nothing then; the layer draws the objects and takes the pointer)
    w.set_ignore_cursor_events(moving || o.click_through)?;
    log_overlay(app, obj, true, reason, &placed);
    ensure_window_type(app, obj.label(), true);
    if reason != "move" {
        verify_position(app, obj);
    }
    let _ = app.emit_to(obj.label(), "overlay-mode", moving);
    Ok(())
}

fn preset_name(o: &OverlaySettings) -> &'static str {
    match (
        o.show_killfeed,
        o.show_weapons,
        o.show_comparison,
        o.compact,
    ) {
        (false, false, false, true) => "minimal",
        (true, false, false, false) => "streamer",
        (true, true, true, false) => "full",
        _ => "custom",
    }
}

/// Reads the position the window manager actually gave the window a moment
/// after showing it and logs it; if it differs from the request it is set once
/// more (at most 3 corrections per 10 s per object, against loops with a window
/// manager that insists on its own placement).
fn verify_position(app: &AppHandle, obj: Obj) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let h = app.clone();
        let _ = app.run_on_main_thread(move || {
            let st = state(&h);
            let Some(want) = st
                .overlay_target
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(obj.name())
                .copied()
            else {
                return;
            };
            let Some(w) = h.get_webview_window(obj.label()) else {
                return;
            };
            if !w.is_visible().unwrap_or(false) {
                return;
            }
            let Ok(actual) = w.outer_position() else {
                return;
            };
            let ok = (actual.x - want.0).abs() <= 2 && (actual.y - want.1).abs() <= 2;
            tracing::info!(
                object = obj.name(),
                requested = %format!("{},{}", want.0, want.1),
                actual = %format!("{},{}", actual.x, actual.y),
                ok,
                "overlay position check"
            );
            if !ok {
                let now = std::time::Instant::now();
                let mut all = st
                    .overlay_corrections
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let c = all.entry(obj.name()).or_default();
                c.retain(|t| now.duration_since(*t).as_secs() < 10);
                if c.len() < 3 {
                    c.push(now);
                    tracing::warn!(
                        object = obj.name(),
                        "moved by the window manager, setting the position again"
                    );
                    let _ = w.set_position(PhysicalPosition::new(want.0, want.1));
                }
            }
        });
    });
}

/// An object reports the measured size of its content (physical pixels). The
/// window size follows only when it really changed (settings, size step, pack
/// image), never for new kill feed lines or notifications.
pub fn set_content_size(app: &AppHandle, obj: Obj, width: u32, height: u32) -> tauri::Result<()> {
    let st = state(app);
    let mut cfg = st.cfg().clone();
    let (_, _, old) = obj.geometry(&cfg.overlay);
    let (w, h) = (width.clamp(40, 4000), height.clamp(20, 4000));
    if old.0.abs_diff(w) <= 1 && old.1.abs_diff(h) <= 1 {
        return Ok(());
    }
    tracing::info!(
        object = obj.name(),
        old = %format!("{}x{}", old.0, old.1),
        new = %format!("{w}x{h}"),
        "overlay size changed (content)"
    );
    obj.set_size(&mut cfg.overlay, (w, h));
    let _ = cfg.save(&st.paths.config_file());
    *st.config.write().unwrap_or_else(|e| e.into_inner()) = cfg;
    apply_object(app, obj, "content size")
}

/// Move mode (arrow-key hotkeys, the overlay page): moves an object in the move
/// layer by (dx, dy) physical pixels, kept on its monitor. Written to the
/// settings only when the session is saved.
pub fn nudge(app: &AppHandle, obj: Obj, dx: i32, dy: i32) -> tauri::Result<()> {
    let st = state(app);
    let in_session = match lock(&st.move_session).as_mut() {
        Some(sess) => {
            sess.object_mut(obj).nudge(dx, dy);
            true
        }
        None => false,
    };
    if !in_session {
        return Ok(());
    }
    emit_move_state(app);
    Ok(())
}

/// Selected object in the move mode (Tab switches, arrow keys move it).
pub fn move_selected(app: &AppHandle) -> Obj {
    if state(app).overlay_selected.load(Ordering::Relaxed) == 1 {
        Obj::Popup
    } else {
        Obj::Stats
    }
}

pub fn select_next(app: &AppHandle) {
    let st = state(app);
    let next = 1 - st.overlay_selected.load(Ordering::Relaxed).min(1);
    st.overlay_selected.store(next, Ordering::Relaxed);
    let name = move_selected(app).name();
    let _ = app.emit("move-selected", name);
    emit_move_state(app);
}

fn window_title(label: &str) -> &'static str {
    match label {
        CROSSHAIR => CROSSHAIR_TITLE,
        POPUP => POPUP_TITLE,
        _ => OVERLAY_TITLE,
    }
}

/// Under X11/XWayland: gives our window the on-screen-display type so KWin keeps
/// it above an active fullscreen game (see `x11.rs`), or the normal type again.
/// The change needs an unmap/map, done once per change in a background thread;
/// afterwards the window is placed again.
fn ensure_window_type(app: &AppHandle, label: &'static str, osd: bool) {
    if !x11::is_x11_backend() || std::env::var_os("PS2_TRACKER_NO_OSD").is_some() {
        return;
    }
    let st = state(app);
    {
        let mut types = st.x11_types.lock().unwrap_or_else(|e| e.into_inner());
        if types.get(label) == Some(&osd) {
            return;
        }
        types.insert(label, osd);
    }
    let app = app.clone();
    std::thread::spawn(
        move || match x11::set_window_type(window_title(label), osd) {
            Ok(win) => {
                tracing::info!(
                    window = label,
                    xid = format!("{win:#x}"),
                    osd,
                    "X11 window type set and window remapped"
                );
                let h = app.clone();
                let _ = app.run_on_main_thread(move || {
                let res = match label {
                    CROSSHAIR => apply_crosshair(&h, "x11 window type"),
                    POPUP => apply_object(&h, Obj::Popup, "x11 window type"),
                    _ => apply_object(&h, Obj::Stats, "x11 window type"),
                };
                if let Err(e) = res {
                    tracing::warn!(window = label, error = %e, "re-show after window type change failed");
                }
            });
            }
            Err(e) => {
                tracing::warn!(window = label, error = %e, "X11 window type not set; the window may end up below a fullscreen game");
            }
        },
    );
}

pub fn toggle_overlay(app: &AppHandle) -> tauri::Result<bool> {
    let st = state(app);
    let mut cfg = st.cfg().clone();
    cfg.overlay.enabled = !cfg.overlay.enabled;
    let _ = cfg.save(&st.paths.config_file());
    let on = cfg.overlay.enabled;
    *st.config.write().unwrap_or_else(|e| e.into_inner()) = cfg;
    apply_overlay(app, "toggle (hotkey/tray/UI)")?;
    let _ = app.emit("config-changed", ());
    Ok(on)
}

// ------------------------------------------------------------------ Move layer

/// Running move session: the objects as they are in the layer, the settings
/// to restore on cancel, where the layer is and what input it got.
pub struct MoveSession {
    pub generation: u64,
    pub backup: OverlaySettings,
    pub stats: overlay::MovedObject,
    pub popup: overlay::MovedObject,
    /// The overlay was put on another monitor ("Move to monitor…").
    pub stats_monitor_changed: bool,
    pub layer: MonitorInfo,
    pub focus: Option<bool>,
    pub pointer_free: Option<bool>,
    /// Last input in the layer (drag, keys, buttons, pointer movement).
    pub last_input: std::time::Instant,
}

impl MoveSession {
    pub fn object_mut(&mut self, obj: Obj) -> &mut overlay::MovedObject {
        self.last_input = std::time::Instant::now();
        match obj {
            Obj::Stats => &mut self.stats,
            Obj::Popup => &mut self.popup,
        }
    }
}

static MOVE_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// What the move layer draws.
#[derive(Serialize, Clone)]
pub struct MoveLayerState {
    pub monitor: MonitorInfo,
    pub monitors: Vec<MonitorInfo>,
    pub stats: overlay::MovedObject,
    pub popup: overlay::MovedObject,
    pub selected: &'static str,
    /// true/false/null (unknown, e.g. Wayland or Windows)
    pub focus: Option<bool>,
    pub pointer_free: Option<bool>,
    pub hotkey_edit: String,
    /// Ends the move mode (saves), registered for the whole session.
    pub hotkey_finish: String,
    /// Hotkeys of the move mode that could not be registered.
    pub failed_hotkeys: Vec<String>,
    /// The move mode ends by itself after this long without input (0 = off).
    pub idle_timeout_secs: u32,
}

pub fn move_layer_state(app: &AppHandle) -> Option<MoveLayerState> {
    let st = state(app);
    let (hotkey_edit, hotkey_finish, idle_timeout_secs) = {
        let o = &st.cfg().overlay;
        (
            o.hotkey_edit.clone(),
            o.hotkey_finish_move.clone(),
            o.move_idle_timeout_secs,
        )
    };
    let failed_hotkeys: Vec<String> = st
        .hotkey_failures
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .map(|(k, _, _)| k.clone())
        .collect();
    let selected = move_selected(app).name();
    // before the lock: from a worker thread this waits for the main thread,
    // and a move-layer command on the main thread needs the session lock
    let monitors = monitors(app);
    let g = lock(&st.move_session);
    let s = g.as_ref()?;
    Some(MoveLayerState {
        monitor: s.layer.clone(),
        monitors,
        stats: s.stats.clone(),
        popup: s.popup.clone(),
        selected,
        focus: s.focus,
        pointer_free: s.pointer_free,
        hotkey_edit,
        hotkey_finish,
        failed_hotkeys,
        idle_timeout_secs,
    })
}

fn emit_move_state(app: &AppHandle) {
    if let Some(s) = move_layer_state(app) {
        let _ = app.emit_to(MOVE_LAYER, "move-layer-update", s);
    }
}

fn moved_object(app: &AppHandle, o: &OverlaySettings, obj: Obj) -> Option<overlay::MovedObject> {
    let m = find_monitor(app, monitor_setting(o, obj))?;
    let (anchor, offset, size) = obj.geometry(o);
    Some(overlay::MovedObject::new(
        m.name().map_or("", |s| s.as_str()),
        area_of(&m),
        &anchor,
        offset,
        size,
    ))
}

fn monitor_info(app: &AppHandle, m: &Monitor) -> MonitorInfo {
    let name = m.name().cloned().unwrap_or_default();
    monitors(app)
        .into_iter()
        .find(|i| i.name == name)
        .unwrap_or(MonitorInfo {
            name,
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            scale_factor: m.scale_factor(),
            primary: false,
        })
}

/// Monitor of the game window (centre), else the overlay's monitor.
fn layer_monitor(app: &AppHandle, o: &OverlaySettings) -> Option<Monitor> {
    let game = crate::focus::GameMatch::from_cfg(&state(app).cfg());
    if let Some((cx, cy)) = crate::focus::game_window_center(&game) {
        let hit = app.available_monitors().ok()?.into_iter().find(|m| {
            let (x, y, w, h) = area_of(m);
            cx >= x
                && cy >= y
                && i64::from(cx) < i64::from(x) + i64::from(w)
                && i64::from(cy) < i64::from(y) + i64::from(h)
        });
        if hit.is_some() {
            return hit;
        }
    }
    find_monitor(app, &o.monitor)
}

/// Move mode on/off (hotkey, default Ctrl+F2): on opens the move layer, off
/// saves (Enter, the hotkey again, "Save"). Esc and "Cancel" call [`end_move`]
/// with `save = false`.
pub fn set_overlay_edit(app: &AppHandle, edit: bool, via: &'static str) -> tauri::Result<()> {
    if edit {
        start_move(app)
    } else {
        end_move(app, true, via)
    }
}

/// Opens the move layer: a borderless, dimmed window over the whole monitor of
/// the game with the same on-screen-display type as the crosshair, so it lies
/// above the fullscreen game. It draws both objects at their real position and
/// size; dragging happens inside it with pointer events (no window-manager
/// move, which KWin refuses for OSD windows: `X11Window::isMovable()` is false
/// for special windows). The real overlay windows are not moved and keep their
/// type; they stay click-through and empty until the session ends. Tab, the
/// arrow keys, Enter and Esc are temporary global hotkeys, so the keyboard
/// works even when the game holds the pointer.
fn start_move(app: &AppHandle) -> tauri::Result<()> {
    let st = state(app);
    if lock(&st.move_session).is_some() {
        return Ok(());
    }
    let mut cfg = st.cfg().clone();
    if !cfg.overlay.enabled {
        cfg.overlay.enabled = true;
        *st.config.write().unwrap_or_else(|e| e.into_inner()) = cfg.clone();
    }
    let o = cfg.overlay.clone();
    let (Some(stats), Some(popup), Some(lm)) = (
        moved_object(app, &o, Obj::Stats),
        moved_object(app, &o, Obj::Popup),
        layer_monitor(app, &o),
    ) else {
        tracing::warn!("move layer not opened: no monitor");
        return Ok(());
    };
    let generation = MOVE_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let layer = monitor_info(app, &lm);
    tracing::info!(monitor = %layer.name, "move mode: opening the move layer");
    *lock(&st.move_session) = Some(MoveSession {
        generation,
        backup: o,
        stats,
        popup,
        stats_monitor_changed: false,
        layer: layer.clone(),
        focus: None,
        pointer_free: None,
        last_input: std::time::Instant::now(),
    });
    st.overlay_edit.store(true, Ordering::Relaxed);
    spawn_idle_watch(app, generation);
    st.overlay_selected.store(0, Ordering::Relaxed);
    // re-register the hotkeys (with the temporary move keys) from another
    // thread: this may run inside a hotkey callback on the main thread, and
    // the plugin waits for the main thread (seen: a hang when Enter ended the
    // move mode)
    let h = app.clone();
    std::thread::spawn(move || crate::hotkeys::register(&h));
    // window work on the main thread, without waiting for it here: this may
    // run on a hotkey thread, and a blocking window call from there raced
    // with move-layer commands (seen as a hang in the E2E test)
    let h = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Err(e) = apply_overlay(&h, "move mode on") {
            tracing::warn!(error = %e, "overlay not placed for the move mode");
        }
        let _ = h.emit("overlay-mode", true);
        if let Err(e) = open_layer(&h, &layer, generation) {
            tracing::warn!(error = %e, "move layer not opened");
        }
    });
    Ok(())
}

fn open_layer(app: &AppHandle, m: &MonitorInfo, generation: u64) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(MOVE_LAYER) {
        let _ = w.destroy();
    }
    let scale = m.scale_factor.max(0.1);
    let w = WebviewWindowBuilder::new(app, MOVE_LAYER, WebviewUrl::App("move.html".into()))
        .title(MOVE_LAYER_TITLE)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .focusable(true)
        .focused(true)
        .visible(false)
        .visible_on_all_workspaces(true)
        .inner_size(f64::from(m.width) / scale, f64::from(m.height) / scale)
        .position(f64::from(m.x) / scale, f64::from(m.y) / scale)
        .build()?;
    place_layer(&w, m)?;
    let _ = w.set_focus();
    let app = app.clone();
    let m = m.clone();
    std::thread::spawn(move || layer_input(&app, &m, generation));
    Ok(())
}

fn place_layer(w: &WebviewWindow, m: &MonitorInfo) -> tauri::Result<()> {
    w.set_size(PhysicalSize::new(m.width, m.height))?;
    w.set_position(PhysicalPosition::new(m.x, m.y))?;
    w.show()?;
    w.set_always_on_top(true)
}

fn session_alive(app: &AppHandle, generation: u64) -> bool {
    lock(&state(app).move_session)
        .as_ref()
        .is_some_and(|s| s.generation == generation)
}

fn yes_no(v: Option<bool>, yes: &'static str, no: &'static str) -> &'static str {
    match v {
        Some(true) => yes,
        Some(false) => no,
        None => "unknown",
    }
}

/// Background: window type, focus request, pointer probe; then, while the
/// game holds the pointer, a probe every 500 ms until it is free.
fn layer_input(app: &AppHandle, m: &MonitorInfo, generation: u64) {
    let x11 = x11::is_x11_backend() && std::env::var_os("PS2_TRACKER_NO_OSD").is_none();
    let mut input = x11::LayerInput {
        focus: None,
        pointer_free: None,
    };
    if x11 {
        match x11::set_window_type(MOVE_LAYER_TITLE, true) {
            Ok(_) => {
                // the remap lets the window manager place it anew
                let h = app.clone();
                let mm = m.clone();
                let _ = app.run_on_main_thread(move || {
                    if let Some(w) = h.get_webview_window(MOVE_LAYER) {
                        let _ = place_layer(&w, &mm);
                    }
                });
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            Err(e) => {
                tracing::warn!(error = %e, "move layer: X11 window type not set; it may stay below a fullscreen game")
            }
        }
        match x11::activate_layer(MOVE_LAYER_TITLE) {
            Ok(i) => input = i,
            Err(e) => tracing::warn!(error = %e, "move layer: focus request failed"),
        }
    } else if let Some(w) = app.get_webview_window(MOVE_LAYER) {
        // Windows: does the game confine the cursor (mouse look)? Read before
        // the layer takes the foreground; the game may release it then.
        #[cfg(windows)]
        {
            input.pointer_free = crate::win_console::pointer_free();
        }
        let _ = w.set_focus();
        std::thread::sleep(std::time::Duration::from_millis(200));
        input.focus = w.is_focused().ok();
    }
    if !session_alive(app, generation) {
        return;
    }
    if let Some(s) = lock(&state(app).move_session).as_mut() {
        s.focus = input.focus;
        s.pointer_free = input.pointer_free;
    }
    tracing::info!(
        monitor = %m.name,
        area = %format!("{},{} {}x{}", m.x, m.y, m.width, m.height),
        focus = yes_no(input.focus, "granted", "denied"),
        pointer = yes_no(input.pointer_free, "free", "grabbed"),
        "move layer opened"
    );
    emit_move_state(app);
    if input.pointer_free == Some(false) {
        crate::hotkeys::register(app);
    }
    let mut free = input.pointer_free;
    // pointer probe: X11 grab attempt, on Windows the cursor clip rectangle
    let probe = || {
        if x11 {
            return x11::layer_pointer_free(MOVE_LAYER_TITLE);
        }
        #[cfg(windows)]
        return crate::win_console::pointer_free();
        #[cfg(not(windows))]
        None
    };
    while (x11 || cfg!(windows)) && free == Some(false) && session_alive(app, generation) {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let now = probe();
        if now.is_some() && now != free {
            free = now;
            if let Some(s) = lock(&state(app).move_session).as_mut() {
                s.pointer_free = now;
            }
            tracing::info!(
                pointer = yes_no(now, "free", "grabbed"),
                "move layer pointer changed"
            );
            crate::hotkeys::register(app);
            emit_move_state(app);
        }
    }
}

/// Safety net: without any input in the layer for the configured time the move
/// mode ends by itself and saves (a toast in the main window says so), so
/// nobody is stuck in it when no key reaches the app.
fn spawn_idle_watch(app: &AppHandle, generation: u64) {
    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let st = state(&app);
            let limit = u64::from(st.cfg().overlay.move_idle_timeout_secs);
            let idle = match lock(&st.move_session).as_ref() {
                Some(s) if s.generation == generation => s.last_input.elapsed().as_secs(),
                _ => return,
            };
            if limit > 0 && idle >= limit {
                if let Err(e) = end_move(&app, true, "timeout") {
                    tracing::warn!(error = %e, "move mode not ended after the timeout");
                }
                return;
            }
        }
    });
}

/// Input in the layer that changes nothing (pointer movement): resets the
/// idle timer.
pub fn move_touch(app: &AppHandle) {
    if let Some(s) = lock(&state(app).move_session).as_mut() {
        s.last_input = std::time::Instant::now();
    }
}

fn close_layer(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MOVE_LAYER) {
        let _ = w.destroy();
    }
}

fn position_text(o: &OverlaySettings, obj: Obj) -> String {
    let (anchor, off, _) = obj.geometry(o);
    let mon = monitor_setting(o, obj);
    format!(
        "{anchor} {},{} on {}",
        off.0,
        off.1,
        if mon.is_empty() { "primary" } else { mon }
    )
}

/// Ends the move session. `save`: the layer positions become the stored
/// anchor offsets (relative to the monitor, as before); otherwise the old
/// positions stay. The layer closes and the real windows are placed once,
/// followed by the usual position check.
pub fn end_move(app: &AppHandle, save: bool, via: &'static str) -> tauri::Result<()> {
    let st = state(app);
    let sess = lock(&st.move_session).take();
    let was = st.overlay_edit.swap(false, Ordering::Relaxed);
    if sess.is_none() && !was {
        return Ok(());
    }
    let mut cfg = st.cfg().clone();
    if let Some(sess) = sess {
        let o = &mut cfg.overlay;
        if save {
            o.anchor = sess.stats.anchor.clone();
            (o.x, o.y) = sess.stats.offset();
            o.popup_anchor = sess.popup.anchor.clone();
            (o.popup_x, o.popup_y) = sess.popup.offset();
            if sess.stats_monitor_changed {
                o.monitor = sess.stats.monitor.clone();
            }
            // the pop-up keeps "same monitor as the overlay" only while it is
            o.popup_monitor = if sess.popup.monitor == sess.stats.monitor {
                String::new()
            } else {
                sess.popup.monitor.clone()
            };
        } else {
            let b = &sess.backup;
            (o.anchor, o.x, o.y) = (b.anchor.clone(), b.x, b.y);
            (o.popup_anchor, o.popup_x, o.popup_y) = (b.popup_anchor.clone(), b.popup_x, b.popup_y);
            (o.monitor, o.popup_monitor) = (b.monitor.clone(), b.popup_monitor.clone());
        }
    }
    if save {
        if let Err(e) = cfg.save(&st.paths.config_file()) {
            tracing::warn!(error = %e, "overlay positions not saved");
        }
        tracing::info!(
            stats = %position_text(&cfg.overlay, Obj::Stats),
            popup = %position_text(&cfg.overlay, Obj::Popup),
            "move layer saved"
        );
    } else {
        tracing::info!(
            stats = %position_text(&cfg.overlay, Obj::Stats),
            popup = %position_text(&cfg.overlay, Obj::Popup),
            "move layer cancelled, old positions kept"
        );
    }
    *st.config.write().unwrap_or_else(|e| e.into_inner()) = cfg;
    tracing::info!(via, saved = save, "move layer finished");
    let _ = app.emit(
        "move-finished",
        serde_json::json!({ "via": via, "saved": save }),
    );
    let reason = if save {
        "move layer saved"
    } else {
        "move layer cancelled"
    };
    // close the layer and place the real windows once, on the main thread
    // (see start_move)
    let h = app.clone();
    let _ = app.run_on_main_thread(move || {
        close_layer(&h);
        if let Err(e) = apply_overlay(&h, reason) {
            tracing::warn!(error = %e, "overlay not placed after the move mode");
        }
        let _ = h.emit("overlay-mode", false);
        let _ = h.emit("config-changed", ());
    });
    let h = app.clone();
    std::thread::spawn(move || crate::hotkeys::register(&h));
    Ok(())
}

/// Drag end in the layer: absolute physical position of the object.
pub fn move_set(app: &AppHandle, obj: Obj, x: i32, y: i32) {
    if let Some(s) = lock(&state(app).move_session).as_mut() {
        s.object_mut(obj).set_pos((x, y));
    }
    emit_move_state(app);
}

pub fn move_select(app: &AppHandle, obj: Obj) {
    move_touch(app);
    state(app)
        .overlay_selected
        .store(u8::from(obj == Obj::Popup), Ordering::Relaxed);
    let _ = app.emit("move-selected", obj.name());
    emit_move_state(app);
}

/// "Reset": default anchor and offset of the object (on its current monitor).
pub fn move_reset(app: &AppHandle, obj: Obj) {
    let d = OverlaySettings::default();
    let (anchor, off, _) = obj.geometry(&d);
    if let Some(s) = lock(&state(app).move_session).as_mut() {
        s.object_mut(obj).reset(&anchor, off);
    }
    emit_move_state(app);
}

/// "Move to monitor…": same anchor and offset on another monitor.
pub fn move_to_monitor(app: &AppHandle, obj: Obj, monitor: &str) -> Result<(), String> {
    let m = app
        .available_monitors()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|m| m.name().is_some_and(|n| n == monitor))
        .ok_or_else(|| format!("no monitor {monitor}"))?;
    if let Some(s) = lock(&state(app).move_session).as_mut() {
        s.object_mut(obj).to_monitor(monitor, area_of(&m));
        if obj == Obj::Stats {
            s.stats_monitor_changed = true;
        }
    }
    emit_move_state(app);
    Ok(())
}

// ------------------------------------------------------------------ Crosshair

#[derive(Serialize, Clone)]
pub struct CrosshairRender {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    pub opacity: f32,
    pub tint: String,
}

/// A PNG from the crosshair library as data URL (hit feedback "swap"); only
/// plain file names, only valid crosshair images.
pub fn crosshair_data_url(st: &crate::state::AppState, file: &str) -> Option<String> {
    if file.is_empty() || file.contains(['/', '\\', ':']) || file.contains("..") {
        return None;
    }
    let bytes = std::fs::read(st.paths.crosshairs_dir().join(file)).ok()?;
    crosshair::inspect(&bytes).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    ))
}

pub fn crosshair_render(app: &AppHandle) -> Option<CrosshairRender> {
    let st = state(app);
    let c = st.cfg().crosshair.clone();
    let path = st.paths.crosshairs_dir().join(&c.file);
    let bytes = std::fs::read(path).ok()?;
    let (width, height) = crosshair::inspect(&bytes).ok()?;
    Some(CrosshairRender {
        data_url: format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        ),
        width,
        height,
        scale: c.scale.clamp(1, 8),
        opacity: c.opacity,
        tint: c.tint,
    })
}

fn crosshair_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(CROSSHAIR) {
        return Ok(w);
    }
    WebviewWindowBuilder::new(app, CROSSHAIR, WebviewUrl::App("crosshair.html".into()))
        .title(CROSSHAIR_TITLE)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .visible_on_all_workspaces(true)
        .inner_size(
            f64::from(CROSSHAIR_MIN_WINDOW),
            f64::from(CROSSHAIR_MIN_WINDOW),
        )
        .build()
}

/// Pure visibility rule. Only a clearly different active window hides the
/// crosshair when "only while the game is active" is on (fail open).
pub fn crosshair_wanted(enabled: bool, visible: bool, auto_hide: bool, focus: FocusState) -> bool {
    enabled && visible && !(auto_hide && focus == FocusState::Other)
}

fn log_crosshair(app: &AppHandle, shown: bool, reason: &str, detail: &str) {
    let st = state(app);
    let new = if shown { 2 } else { 1 };
    let old = st.crosshair_last.swap(new, Ordering::Relaxed);
    if old != new {
        tracing::info!(
            shown,
            reason,
            backend = x11::backend_name(),
            "crosshair {detail}"
        );
    } else {
        tracing::debug!(shown, reason, "crosshair {detail}");
    }
}

/// Shows the crosshair exactly centred (physical pixels) or hides it. `reason`
/// only goes to the log (hotkey, focus watcher, config, …).
pub fn apply_crosshair(app: &AppHandle, reason: &str) -> tauri::Result<()> {
    let st = state(app);
    let c = st.cfg().crosshair.clone();
    let focus = FocusState::from_u8(st.focus_state.load(Ordering::Relaxed));
    let visible = st.crosshair_visible.load(Ordering::Relaxed);
    let want = crosshair_wanted(c.enabled, visible, c.auto_hide, focus);
    let render = if want { crosshair_render(app) } else { None };
    let Some(render) = render else {
        let detail = if want {
            tracing::warn!(file = %c.file, "crosshair image missing or not a valid PNG");
            "hidden: image missing".to_string()
        } else {
            format!(
                "hidden (enabled={}, toggled_on={visible}, auto_hide={}, focus={focus:?})",
                c.enabled, c.auto_hide
            )
        };
        log_crosshair(app, false, reason, &detail);
        if let Some(w) = app.get_webview_window(CROSSHAIR) {
            w.hide()?;
        }
        return Ok(());
    };
    let w = crosshair_window(app)?;
    let size = crosshair_window_size(render.width * render.scale, render.height * render.scale);
    w.set_size(PhysicalSize::new(size.0, size.1))?;
    let mut placed = String::from("no monitor");
    if let Some(m) = find_monitor(app, &c.monitor) {
        let (x, y) = crosshair::centered_position(
            (m.position().x, m.position().y),
            (m.size().width, m.size().height),
            size,
            (c.offset_x, c.offset_y),
        );
        w.set_position(PhysicalPosition::new(x, y))?;
        placed = format!(
            "monitor {} at {},{} {}x{} scale {}, window {}x{} at {x},{y}",
            m.name().map_or("?", |s| s.as_str()),
            m.position().x,
            m.position().y,
            m.size().width,
            m.size().height,
            m.scale_factor(),
            size.0,
            size.1
        );
    }
    w.show()?;
    w.set_always_on_top(true)?;
    w.set_ignore_cursor_events(true)?;
    log_crosshair(
        app,
        true,
        reason,
        &format!("shown ({placed}, focus={focus:?})"),
    );
    ensure_window_type(app, CROSSHAIR, true);
    let _ = app.emit_to(CROSSHAIR, "crosshair-render", render);
    Ok(())
}

/// Minimum size of the crosshair window (some window managers/WebKit enforce a
/// minimum size). The image is centred inside; equal parity of window and image
/// size keeps the centre exactly on a pixel.
pub const CROSSHAIR_MIN_WINDOW: u32 = 256;

pub fn crosshair_window_size(w: u32, h: u32) -> (u32, u32) {
    let fit = |img: u32| {
        let mut s = img.max(CROSSHAIR_MIN_WINDOW);
        if (s - img) % 2 == 1 {
            s += 1;
        }
        s
    };
    (fit(w), fit(h))
}

pub fn toggle_crosshair(app: &AppHandle) -> tauri::Result<()> {
    let st = state(app);
    let enabled = st.cfg().crosshair.enabled;
    if !enabled {
        let mut cfg = st.cfg().clone();
        cfg.crosshair.enabled = true;
        let _ = cfg.save(&st.paths.config_file());
        *st.config.write().unwrap_or_else(|e| e.into_inner()) = cfg;
        st.crosshair_visible.store(true, Ordering::Relaxed);
        let _ = app.emit("config-changed", ());
    } else {
        let v = st.crosshair_visible.load(Ordering::Relaxed);
        st.crosshair_visible.store(!v, Ordering::Relaxed);
    }
    apply_crosshair(app, "toggle (hotkey/tray/UI)")
}

pub fn next_crosshair(app: &AppHandle) -> tauri::Result<()> {
    let st = state(app);
    let list = crosshair::list(&st.paths.crosshairs_dir());
    if list.is_empty() {
        return Ok(());
    }
    let mut cfg = st.cfg().clone();
    let names: Vec<String> = list
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    let i = names.iter().position(|n| *n == cfg.crosshair.file);
    cfg.crosshair.file = names[i.map_or(0, |i| (i + 1) % names.len())].clone();
    let _ = cfg.save(&st.paths.config_file());
    *st.config.write().unwrap_or_else(|e| e.into_inner()) = cfg;
    let _ = app.emit("config-changed", ());
    apply_crosshair(app, "next crosshair")
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::crosshair_window_size;

    #[test]
    fn crosshair_window_keeps_parity() {
        assert_eq!(crosshair_window_size(23, 23), (257, 257));
        assert_eq!(crosshair_window_size(46, 46), (256, 256));
        assert_eq!(crosshair_window_size(300, 9), (300, 257));
    }
}
