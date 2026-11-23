// App commands are declared via the app manifest: each window gets only the
// commands it needs in `capabilities/` (overlay/crosshair are read-only).
const COMMANDS: &[&str] = &[
    "get_config",
    "save_config",
    "lookup_character",
    "get_status",
    "get_snapshot",
    "list_sessions",
    "get_session",
    "delete_session",
    "export_session",
    "start_session",
    "stop_session",
    "resume_session",
    "last_session",
    "reopen_session",
    "get_overlay_state",
    "overlay_set_edit",
    "list_monitors",
    "get_paths",
    "voice_overview",
    "voice_get_pack",
    "voice_save_manifest",
    "voice_new_pack",
    "voice_add_file",
    "voice_import",
    "overlay_content_size",
    "startup_notices",
    "hotkey_status",
    "service_id_source",
    "class_icons",
    "app_info",
    "move_layer_activity",
    "move_layer_state",
    "move_layer_set",
    "move_layer_select",
    "move_layer_end",
    "move_layer_reset",
    "move_layer_monitor",
    "voice_set_override",
    "voice_clear_override",
    "voice_pack_size",
    "voice_remove_pack",
    "voice_undo_remove",
    "voice_export",
    "voice_preview_file",
    "voice_preview_event",
    "voice_output_devices",
    "voice_propose_mapping",
    "voice_create_from_mapping",
    "voice_preview_foreign",
    "crosshair_list",
    "crosshair_import",
    "crosshair_render_info",
    "crosshair_toggle",
    "focus_probe",
    "toast_background",
    "open_folder",
    "game_windows",
    "game_status",
    "crosshair_fx_preview",
    "import_session",
    "session_analysis",
    "save_png",
    "squad_rows",
    "outfit_members",
    "squad_remove",
    "update_status",
    "update_check",
    "update_install",
];

/// Commit and build date for "About" and bug reports: CI variables first
/// (CI_COMMIT_SHORT_SHA), else `git rev-parse`, else "unknown". The date is
/// SOURCE_DATE_EPOCH (reproducible builds) or today, UTC, as YYYY-MM-DD.
fn build_info() {
    let commit = std::env::var("CI_COMMIT_SHORT_SHA")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "--short=8", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                .filter(|s| !s.is_empty())
        })
        .filter(|s| s.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| "unknown".into());
    let secs = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });
    // civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    println!("cargo:rustc-env=PS2_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=PS2_BUILD_DATE={y:04}-{m:02}-{d:02}");
    println!("cargo:rerun-if-env-changed=CI_COMMIT_SHORT_SHA");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    for p in ["../.git/HEAD", "../.git/index"] {
        if std::path::Path::new(p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }
}

fn main() {
    build_info();
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("tauri-build failed");
}
