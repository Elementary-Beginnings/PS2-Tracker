//! PS2 Session Tracker – desktop app (Tauri v2).
//!
//! Does not touch the game: no injection, no hooks, no memory reading, no Vulkan
//! layer. All game data comes from the public Census API.

// Windows release builds: GUI subsystem, no console window at start (output
// from a terminal and the debug mode: see console.rs).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod audio;
mod commands;
mod console;
mod discord;
mod engine;
mod focus;
mod hotkeys;
mod obs;
mod state;
mod tray;
mod update;
mod win_console;
mod windows;
mod x11;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8};
use std::sync::{Arc, Mutex, RwLock};

use ps2_tracker_core::census::NameCache;
use ps2_tracker_core::voice::VoiceSelector;
use ps2_tracker_core::{CensusClient, Config, Paths, Store, Tracker, crosshair, voicepack};
use tauri::{Manager, RunEvent, WindowEvent};
use tokio::sync::broadcast;

use crate::state::{AppState, EssStatus, VoiceState, lock};

fn init_state() -> Result<AppState, Box<dyn std::error::Error>> {
    let paths = Paths::xdg()?;
    std::fs::create_dir_all(&paths.config_dir)?;
    std::fs::create_dir_all(&paths.data_dir)?;
    let config = Config::load(&paths.config_file())?;
    if !paths.config_file().exists() {
        config.save(&paths.config_file())?;
    }
    if let Err(e) = voicepack::install_standard_pack(&paths.voicepacks_dir()) {
        tracing::warn!(error = %e, "default voice pack not installed");
    }
    let purged = voicepack::purge_trash(&paths.voicepacks_dir());
    if purged > 0 {
        tracing::info!(
            purged,
            "removed voice packs left in the trash folder deleted"
        );
    }
    if let Err(e) = crosshair::install_standard(&paths.crosshairs_dir()) {
        tracing::warn!(error = %e, "default crosshairs not installed");
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let (store, recovered) = Store::open_or_recover(&paths.database(), now)?;
    let store = Arc::new(store);
    let mut notices = Vec::new();
    if let Some(aside) = recovered {
        tracing::warn!(file = %aside.display(), "session database was damaged: set aside, a new one was created");
        notices.push(state::Notice {
            kind: "database_recovered",
            detail: aside.display().to_string(),
        });
    }
    match store.close_dangling(config.session.session_config()) {
        Ok(n) if n > 0 => tracing::info!(n, "closed sessions left open by the last run"),
        Err(e) => tracing::warn!(error = %e, "open sessions not closed"),
        _ => {}
    }
    let mut names = NameCache::default();
    store.load_names(&mut names)?;
    let tracker = Tracker::new(
        config.session.session_config(),
        i64::from(config.session.idle_timeout_minutes.max(1)) * 60,
        ps2_tracker_core::RuleSet::default_rules(),
    );
    let sid = config.census.resolve()?;
    tracing::info!(source = sid.source().as_str(), "Census service ID");
    let census = CensusClient::new(sid.expose())?;
    let audio = audio::AudioHandle::spawn(
        config.voice.output_device.clone(),
        config.voice.volume,
        config.voice.max_concurrent,
    );
    let (obs_tx, _) = broadcast::channel(16);
    let seed = state::now_ms() | 1;
    Ok(AppState {
        paths,
        store,
        names: Arc::new(Mutex::new(names)),
        item_categories: Mutex::new(HashMap::new()),
        category_names: Mutex::new(HashMap::new()),
        tracker: Mutex::new(tracker),
        last_played: Mutex::new(HashMap::new()),
        lifetime: Mutex::new(HashMap::new()),
        census: RwLock::new(census),
        status: Mutex::new(EssStatus {
            state: "idle".into(),
            ..Default::default()
        }),
        ess_stop: Mutex::new(None),
        ess_characters: Mutex::new(None),
        hotkey_failures: Mutex::new(Vec::new()),
        overlay_edit: AtomicBool::new(false),
        crosshair_visible: AtomicBool::new(true),
        focus_state: AtomicU8::new(focus::FocusState::Unknown.to_u8()),
        crosshair_last: AtomicU8::new(0),
        overlay_last: Default::default(),
        overlay_target: Default::default(),
        overlay_corrections: Default::default(),
        overlay_selected: AtomicU8::new(0),
        x11_types: Mutex::new(HashMap::new()),
        move_session: Mutex::new(None),
        notices: Mutex::new(notices),
        voice: Mutex::new(VoiceState {
            packs: vec![],
            background: None,
            selector: VoiceSelector::new(seed),
            detectors: HashMap::new(),
            triggers: ps2_tracker_core::triggers::TriggerSet::default_set(),
            engines: HashMap::new(),
        }),
        audio,
        obs_tx,
        obs_stop: Mutex::new(None),
        config: RwLock::new(config),
    })
}

/// GPU driver as far as the kernel shows it (for the WebKit workaround and the log).
#[cfg(target_os = "linux")]
fn gpu_driver() -> &'static str {
    if std::path::Path::new("/sys/module/nvidia").exists()
        || std::path::Path::new("/proc/driver/nvidia/version").exists()
    {
        "nvidia"
    } else if std::path::Path::new("/sys/module/amdgpu").exists() {
        "amdgpu"
    } else if std::path::Path::new("/sys/module/i915").exists()
        || std::path::Path::new("/sys/module/xe").exists()
    {
        "intel"
    } else {
        "unknown"
    }
}

/// Environment the app needs before GTK/WebKit start; applied by restarting
/// itself once (`std::env::set_var` would need `unsafe`, which this crate
/// denies). Variables the user has set are never overridden.
///
/// - `GDK_BACKEND=x11` in a Wayland session: KWin ignores the position and
///   "keep above" of native Wayland clients, and only an X11 window can ask for
///   the on-screen-display layer above a fullscreen game (see x11.rs). Off with
///   `ui.x11_backend = false` or `PS2_TRACKER_WAYLAND=1`.
/// - `WEBKIT_DISABLE_DMABUF_RENDERER=1` with the NVIDIA driver: WebKitGTK's
///   DMA-BUF renderer fails there ("Failed to create GBM buffer", grey window);
///   the Tauri docs list this workaround for NVIDIA but advise against setting
///   it unconditionally (https://v2.tauri.app/develop/debug/linux-graphics/).
///   Off with `ui.webkit_dmabuf_workaround = false` or `PS2_TRACKER_NO_DMABUF_FIX=1`.
///
/// `PS2_TRACKER_SET_ENV` tells the restarted process what was set, for the log.
#[cfg(target_os = "linux")]
fn prepare_environment(args: &[String]) {
    use std::os::unix::process::CommandExt;
    let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    if set("PS2_TRACKER_SET_ENV") {
        return; // already restarted once
    }
    let cfg = Paths::xdg()
        .ok()
        .and_then(|p| Config::load(&p.config_file()).ok());
    let mut add: Vec<(&str, &str)> = Vec::new();
    if set("WAYLAND_DISPLAY")
        && set("DISPLAY")
        && !set("GDK_BACKEND")
        && std::env::var("PS2_TRACKER_WAYLAND").map_or(true, |v| v != "1")
        && cfg.as_ref().is_none_or(|c| c.ui.x11_backend)
    {
        add.push(("GDK_BACKEND", "x11"));
    }
    if gpu_driver() == "nvidia"
        && !set("WEBKIT_DISABLE_DMABUF_RENDERER")
        && std::env::var("PS2_TRACKER_NO_DMABUF_FIX").map_or(true, |v| v != "1")
        && cfg.as_ref().is_none_or(|c| c.ui.webkit_dmabuf_workaround)
    {
        add.push(("WEBKIT_DISABLE_DMABUF_RENDERER", "1"));
    }
    if add.is_empty() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let names: Vec<&str> = add.iter().map(|(k, _)| *k).collect();
    let err = std::process::Command::new(exe)
        .args(args.iter().skip(1))
        .envs(add.iter().copied())
        .env("PS2_TRACKER_SET_ENV", names.join(","))
        .exec();
    eprintln!("ps2-tracker: restart with {names:?} failed ({err}), continuing without");
}

/// File log of the primary instance. Until [`open_log_file`] runs (in
/// `setup`, which a second instance that only forwards `--toggle-edit` & co.
/// never reaches) the lines stay in memory. Before, every forwarding call
/// rotated and recreated the file under the running app, whose log then ended
/// up in `.log.1` and was lost on the next start.
struct DeferredLog {
    file: Option<std::fs::File>,
    buf: Vec<u8>,
}

static LOG: Mutex<DeferredLog> = Mutex::new(DeferredLog {
    file: None,
    buf: Vec::new(),
});

/// Up to this much is kept before the file is open (start-up lines only).
const LOG_BUFFER_MAX: usize = 1 << 20;

#[derive(Clone, Copy)]
struct LogWriter;

impl std::io::Write for LogWriter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let mut g = LOG.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(f) = g.file.as_mut() {
            return f.write(b);
        }
        if g.buf.len() + b.len() <= LOG_BUFFER_MAX {
            g.buf.extend_from_slice(b);
        }
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        let mut g = LOG.lock().unwrap_or_else(|e| e.into_inner());
        match g.file.as_mut() {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

/// Primary instance only: keeps the previous run as `.log.1`, creates the log
/// file and writes the lines buffered since the start.
fn open_log_file(path: &std::path::Path) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if path.exists() {
        let _ = std::fs::rename(path, path.with_extension("log.1"));
    }
    let mut f = std::fs::File::create(path)?;
    let mut g = LOG.lock().unwrap_or_else(|e| e.into_inner());
    f.write_all(&std::mem::take(&mut g.buf))?;
    g.file = Some(f);
    Ok(())
}

/// Log to stderr and to `$XDG_STATE_HOME/ps2-tracker/ps2-tracker.log` (the previous
/// run is kept as `.log.1`; the file is opened in `setup`, see [`DeferredLog`]).
/// Level via `RUST_LOG`, default `info`.
fn init_logging(debug: bool) -> Option<std::path::PathBuf> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| console::log_filter(debug).into());
    let stderr = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(|| LogWriter);
    tracing_subscriber::registry()
        .with(filter)
        .with(stderr)
        .with(file_layer)
        .init();
    Paths::xdg().ok().map(|p| p.log_file())
}

fn log_environment(log_file: Option<&std::path::Path>) {
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        commit = env!("PS2_BUILD_COMMIT"),
        build_date = env!("PS2_BUILD_DATE"),
        log_file = %log_file.map(|p| p.display().to_string()).unwrap_or_else(|| "(none)".into()),
        session = %var("XDG_SESSION_TYPE"),
        desktop = %var("XDG_CURRENT_DESKTOP"),
        wayland_display = %var("WAYLAND_DISPLAY"),
        display = %var("DISPLAY"),
        gdk_backend = %var("GDK_BACKEND"),
        backend = x11::backend_name(),
        "PS2 Session Tracker starting"
    );
    #[cfg(target_os = "linux")]
    {
        let ours = var("PS2_TRACKER_SET_ENV");
        let src = |k: &str| {
            if var(k).is_empty() {
                "unset"
            } else if ours.split(',').any(|x| x == k) {
                "set by ps2-tracker"
            } else {
                "set by the user"
            }
        };
        tracing::info!(
            gpu_driver = gpu_driver(),
            webkit_disable_dmabuf_renderer = %var("WEBKIT_DISABLE_DMABUF_RENDERER"),
            dmabuf_source = src("WEBKIT_DISABLE_DMABUF_RENDERER"),
            webkit_disable_compositing_mode = %var("WEBKIT_DISABLE_COMPOSITING_MODE"),
            gdk_backend_source = src("GDK_BACKEND"),
            "graphics environment (DMA-BUF renderer is switched off for the NVIDIA driver, see the wiki page Troubleshooting-and-FAQ)"
        );
    }
}

/// Debug builds only, for the end-to-end test: `--debug-squad-add <id> <name>`
/// adds a squad member through the same path as the Squad page (save_config),
/// without a Census lookup. Release builds do not know the flag.
#[cfg(debug_assertions)]
fn debug_squad_add(app: &tauri::AppHandle, argv: &[String]) -> bool {
    let Some(i) = argv.iter().position(|a| a == "--debug-squad-add") else {
        return false;
    };
    let (Some(id), Some(name)) = (
        argv.get(i + 1).and_then(|s| s.parse().ok()),
        argv.get(i + 2).cloned(),
    ) else {
        tracing::warn!("--debug-squad-add needs <id> <name>");
        return true;
    };
    let mut cfg = engine::state(app).cfg().clone();
    cfg.squad.push(ps2_tracker_core::config::CharacterSettings {
        name,
        id: Some(id),
        faction_id: None,
        enabled: true,
    });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match commands::apply_config(&app, cfg).await {
            Ok(()) => tracing::info!(id, "debug: squad member added"),
            Err(e) => tracing::warn!(error = %e, "debug: squad member not added"),
        }
    });
    true
}

fn main() {
    // rustls is built with two crypto backends (aws-lc-rs via reqwest, ring
    // via the updater plugin); without an explicit choice the first TLS
    // connection panics ("Could not automatically determine the process-level
    // CryptoProvider"). Found by the start-up smoke test.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let args: Vec<String> = std::env::args().collect();
    // debug mode: own console (Windows) or the terminal; otherwise output only
    // goes to a terminal the app was started from
    let debug = console::debug_wanted(&args);
    let _visible = if debug {
        console::open_debug()
    } else {
        console::attach_parent()
    };
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{}", hotkeys::HELP);
        return;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!(
            "ps2-tracker {} ({}, {})",
            env!("CARGO_PKG_VERSION"),
            env!("PS2_BUILD_COMMIT"),
            env!("PS2_BUILD_DATE")
        );
        return;
    }
    for a in hotkeys::from_args(&args) {
        eprintln!(
            "ps2-tracker: {} (sent to the running instance, or applied after startup)",
            a.describe()
        );
    }
    #[cfg(target_os = "linux")]
    prepare_environment(&args);
    let log_file = init_logging(debug);
    if debug {
        tracing::info!("debug mode: log level debug, output in this console");
    }
    // panics in any thread (also audio/cpal threads) go to the log file
    std::panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        tracing::error!(thread = thread.name().unwrap_or("?"), "panic: {info}");
    }));
    log_environment(log_file.as_deref());
    let log_path = log_file.clone();
    audio::log_devices();

    let state = match init_state() {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("PS2 Session Tracker: startup failed: {e}");
            std::process::exit(1);
        }
    };
    let start_args = args;
    let start_hidden = start_args.iter().any(|a| a == "--hidden") || state.cfg().ui.start_minimized;

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            #[cfg(debug_assertions)]
            if debug_squad_add(app, &argv) {
                return;
            }
            let actions = hotkeys::from_args(&argv);
            if actions.is_empty() {
                windows::show_main(app);
            }
            for a in actions {
                hotkeys::run(app, a, "cli");
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        // registered always (it only reads its config); it never contacts the
        // endpoint unless update::check runs (opt-in or "Check now")
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(update::Pending::default())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, ev| hotkeys::on_shortcut(app, shortcut, ev.state()))
                .build(),
        )
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::lookup_character,
            commands::get_status,
            commands::get_snapshot,
            commands::list_sessions,
            commands::get_session,
            commands::delete_session,
            commands::export_session,
            commands::start_session,
            commands::stop_session,
            commands::get_overlay_state,
            commands::overlay_set_edit,
            commands::overlay_content_size,
            commands::startup_notices,
            commands::hotkey_status,
            commands::service_id_source,
            commands::class_icons,
            commands::app_info,
            commands::move_layer_activity,
            commands::move_layer_state,
            commands::move_layer_set,
            commands::move_layer_select,
            commands::move_layer_end,
            commands::move_layer_reset,
            commands::move_layer_monitor,
            commands::list_monitors,
            commands::get_paths,
            commands::voice_overview,
            commands::voice_get_pack,
            commands::voice_save_manifest,
            commands::voice_new_pack,
            commands::voice_add_file,
            commands::voice_import,
            commands::voice_set_override,
            commands::voice_clear_override,
            commands::voice_pack_size,
            commands::voice_remove_pack,
            commands::voice_undo_remove,
            commands::voice_export,
            commands::voice_preview_file,
            commands::voice_preview_event,
            commands::voice_output_devices,
            commands::toast_background,
            commands::open_folder,
            commands::game_windows,
            commands::game_status,
            commands::voice_propose_mapping,
            commands::voice_create_from_mapping,
            commands::voice_preview_foreign,
            commands::crosshair_list,
            commands::crosshair_import,
            commands::crosshair_render_info,
            commands::crosshair_fx_preview,
            commands::import_session,
            commands::session_analysis,
            commands::save_png,
            commands::squad_rows,
            commands::outfit_members,
            commands::squad_remove,
            commands::update_status,
            commands::update_check,
            commands::update_install,
            commands::crosshair_toggle,
            commands::focus_probe,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            {
                let st = engine::state(&handle);
                let rules = engine::load_rules(&st);
                lock(&st.tracker).set_rules(rules);
                lock(&st.voice).triggers = engine::load_triggers(&st);
                engine::reload_voice_packs(&st);
            }
            if let Err(e) = tray::create(&handle) {
                tracing::warn!(error = %e, "tray icon not available");
            }
            // only the primary instance gets here (a forwarding call exits
            // in the single-instance plugin): now the log file may rotate
            if let Some(path) = &log_path
                && let Err(e) = open_log_file(path)
            {
                tracing::warn!(error = %e, "log file not opened");
            }
            hotkeys::register(&handle);
            if start_hidden && let Some(w) = app.get_webview_window("main") {
                let _ = w.hide();
            }
            if let Err(e) = windows::apply_overlay(&handle, "start") {
                tracing::warn!(error = %e, "overlay not shown");
            }
            for m in windows::monitors(&handle) {
                tracing::info!(name = %m.name, x = m.x, y = m.y, width = m.width, height = m.height, scale = m.scale_factor, primary = m.primary, "monitor");
            }
            if let Err(e) = windows::apply_crosshair(&handle, "startup") {
                tracing::warn!(error = %e, "crosshair not shown");
            }
            for a in hotkeys::from_args(&start_args) {
                hotkeys::run(&handle, a, "cli");
            }
            engine::spawn_ticker(&handle);
            focus::spawn_watcher(&handle);
            discord::spawn(&handle);
            update::check_on_start(&handle, state.cfg().updates.check_on_start);
            obs::restart(&handle);
            let h = handle.clone();
            tauri::async_runtime::spawn(async move {
                engine::sync_characters(&h).await;
                engine::restart_ess(&h);
                engine::warm_up(&h).await;
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            // closing the main window minimizes to the tray
            if window.label() == "main"
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build the Tauri app");

    app.run(|handle, event| {
        if let RunEvent::ExitRequested { .. } = event {
            engine::shutdown(handle);
        }
    });
}

#[cfg(test)]
mod log_tests {
    #[test]
    fn log_lines_wait_for_the_primary_instance() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("ps2t-log-{}", std::process::id()));
        let path = dir.join("ps2-tracker.log");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "previous run\n").unwrap();
        // a forwarding call writes only to the buffer: the file stays as it is
        super::LogWriter.write_all(b"early line\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "previous run\n");
        // the primary instance rotates once and writes the buffer first
        super::open_log_file(&path).unwrap();
        super::LogWriter.write_all(b"later line\n").unwrap();
        super::LogWriter.flush().unwrap();
        let now = std::fs::read_to_string(&path).unwrap();
        assert!(now.contains("early line\nlater line\n"), "{now}");
        assert_eq!(
            std::fs::read_to_string(path.with_extension("log.1")).unwrap(),
            "previous run\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
