//! Opt-in in-app updates (Tauri updater v2, signed). Only for the AppImage and
//! the Windows installer; package-managed installs (Arch/AUR, Flatpak) are
//! updated by their package manager. Without a public key in tauri.conf.json
//! (`plugins.updater.pubkey`, entered by the maintainer) the feature is off.
//! The app only contacts the update endpoint after the user opted in
//! (Settings → Updates) or pressed "Check now".

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Pending update found by the last check (installed only after confirmation).
#[derive(Default)]
pub struct Pending(pub Mutex<Option<Update>>);

#[derive(Serialize, Clone)]
pub struct UpdateStatus {
    /// The updater can be used in this installation.
    pub available: bool,
    /// "ok" | "no_key" | "flatpak" | "package" | "dev"
    pub reason: String,
    pub current: String,
}

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub date: Option<String>,
    /// Changelog excerpt from latest.json.
    pub notes: String,
}

fn pubkey(app: &AppHandle) -> String {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|v| v.get("pubkey"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_owned()
}

/// Can this installation update itself?
pub fn status(app: &AppHandle) -> UpdateStatus {
    let current = app.package_info().version.to_string();
    let reason = if pubkey(app).is_empty() {
        "no_key"
    } else if cfg!(target_os = "windows") {
        "ok"
    } else if std::env::var_os("FLATPAK_ID").is_some()
        || std::path::Path::new("/.flatpak-info").exists()
    {
        "flatpak"
    } else if std::env::var_os("APPIMAGE").is_some() {
        "ok"
    } else if std::path::Path::new("/usr/share/ps2-tracker/package-managed").exists() {
        "package"
    } else {
        // cargo run / tauri dev or a self-built binary: nothing to replace safely
        "dev"
    };
    UpdateStatus {
        available: reason == "ok",
        reason: reason.into(),
        current,
    }
}

pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let s = status(app);
    if !s.available {
        return Err(format!("updates not available here ({})", s.reason));
    }
    let updater = app.updater().map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| e.to_string())?;
    let info = found.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        date: u.date.map(|d| d.to_string()),
        notes: u
            .body
            .clone()
            .unwrap_or_default()
            .chars()
            .take(2000)
            .collect(),
    });
    tracing::info!(
        found = info.as_ref().map(|i| i.version.as_str()),
        "update check"
    );
    *app.state::<Pending>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = found;
    Ok(info)
}

/// Downloads, verifies (signature) and installs the pending update, then restarts.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let update = app
        .state::<Pending>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or("no update pending – check first")?;
    tracing::info!(version = %update.version, "installing update");
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}

/// Background check at start, only if the user opted in.
pub fn check_on_start(app: &AppHandle, enabled: bool) {
    if !enabled || !status(app).available {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match check(&app).await {
            Ok(Some(info)) => {
                let _ = app.emit_to("main", "update-available", info);
            }
            Ok(None) => {}
            Err(e) => tracing::info!(error = %e, "update check failed"),
        }
    });
}
