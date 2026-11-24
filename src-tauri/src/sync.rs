//! Online sync glue: the core worker (`ps2_tracker_core::online_sync`) with
//! the OS keyring, the system browser and the UI events.

use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use ps2_tracker_core::census::SharedNames;
use ps2_tracker_core::online_sync::secret::{KeyringSecret, SecretStore};
use ps2_tracker_core::online_sync::worker::{Deps, SyncWorker};
use ps2_tracker_core::{Config, Store};
use tauri::{AppHandle, Emitter};

use crate::state::{AppState, now};

/// Set once the app runs; the worker's change hook emits through it.
static APP: OnceLock<AppHandle> = OnceLock::new();

/// The user's own characters (squad members never upload).
pub fn own_ids(cfg: &Config) -> HashSet<u64> {
    cfg.characters.iter().filter_map(|c| c.id).collect()
}

pub fn create(store: Arc<Store>, names: SharedNames, cfg: &Config) -> Arc<SyncWorker> {
    let deps = Deps {
        store,
        names,
        secrets: Arc::new(|account| Arc::new(KeyringSecret::new(account)) as Arc<dyn SecretStore>),
        open_browser: Arc::new(open_url),
        on_change: Arc::new(|| {
            if let Some(app) = APP.get() {
                let _ = app.emit("sync-changed", ());
            }
        }),
    };
    SyncWorker::new(deps, cfg.online_sync.clone(), own_ids(cfg))
}

/// Starts the background loop (it does nothing while the sync is off).
pub fn start(app: &AppHandle, st: &AppState) {
    let _ = APP.set(app.clone());
    tauri::async_runtime::spawn(st.sync.clone().run(now));
}

/// After the settings or the own characters changed.
pub fn configure(st: &AppState) {
    let (settings, own) = {
        let cfg = st.cfg();
        (cfg.online_sync.clone(), own_ids(&cfg))
    };
    st.sync.configure(settings, own);
}

/// A session ended and was saved: the worker queues it if the sync is on
/// and the character is one of the user's own.
pub fn session_finished(st: &AppState, session: &ps2_tracker_core::Session) {
    if let Some(id) = session.id {
        st.sync.session_finished(id, session.character_id, now());
    }
}

/// Opens an https URL in the system browser: xdg-open on Linux (inside
/// Flatpak the OpenURI portal), ShellExecuteW ("open") on Windows. No shell
/// is involved. explorer.exe is not used for URLs: with the long OAuth URL it
/// opened a File Explorer window instead of the browser.
pub fn open_url(url: &str) -> Result<(), String> {
    let parsed = tauri::Url::parse(url).map_err(|_| "invalid URL".to_string())?;
    if parsed.scheme() != "https" {
        return Err("only https links are opened".into());
    }
    open_https(parsed.as_str())
}

#[cfg(windows)]
fn open_https(url: &str) -> Result<(), String> {
    crate::win_console::shell_open(std::ffi::OsStr::new(url))
}

#[cfg(not(windows))]
fn open_https(url: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|mut child| {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        })
        .map_err(|err| format!("xdg-open: {err}"))
}
