//! Discord Rich Presence over the local Discord IPC (Unix socket
//! `discord-ipc-N` in the runtime directory, named pipe on Windows). Off by
//! default; uses the user's own application ID, or the built-in ELME
//! application of official builds (never logged). Quiet when Discord
//! is not running: reconnects with backoff, logs state changes only.

use std::time::{Duration, Instant};

use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use ps2_tracker_core::presence::{MIN_UPDATE_SECS, Presence, presence};
use tauri::AppHandle;

use crate::engine::{current_snapshot, state};

pub fn spawn(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("discord-rpc".into())
        .spawn(move || run(&app));
}

fn run(app: &AppHandle) {
    let mut client: Option<(String, DiscordIpcClient)> = None;
    let mut last: Option<Presence> = None;
    let mut backoff = Duration::from_secs(MIN_UPDATE_SECS);
    let mut next_try = Instant::now();
    let mut warned = false;
    loop {
        std::thread::sleep(Duration::from_secs(MIN_UPDATE_SECS));
        let st = state(app);
        let cfg = st.cfg().discord.clone();
        let app_id = cfg.effective_app_id();
        let (true, Some(app_id)) = (cfg.enabled, app_id) else {
            if let Some((_, mut c)) = client.take() {
                let _ = c.clear_activity();
                let _ = c.close();
                tracing::info!("discord: presence off");
            }
            last = None;
            continue;
        };
        // new application ID: start over
        if client.as_ref().is_some_and(|(id, _)| *id != app_id) {
            if let Some((_, mut c)) = client.take() {
                let _ = c.close();
            }
            last = None;
        }
        if client.is_none() {
            if Instant::now() < next_try {
                continue;
            }
            let mut c = DiscordIpcClient::new(&app_id);
            match c.connect() {
                Ok(()) => {
                    tracing::info!("discord: connected");
                    client = Some((app_id.clone(), c));
                    backoff = Duration::from_secs(MIN_UPDATE_SECS);
                    warned = false;
                }
                Err(e) => {
                    if !warned {
                        tracing::info!(error = %e, "discord: not reachable (is Discord running?), retrying quietly");
                        warned = true;
                    }
                    next_try = Instant::now() + backoff;
                    backoff = (backoff * 2).min(Duration::from_secs(300));
                    continue;
                }
            }
        }
        let p = presence(current_snapshot(&st).as_ref(), &cfg);
        if last.as_ref() == Some(&p) {
            continue;
        }
        let Some((_, c)) = client.as_mut() else {
            continue;
        };
        let mut a = activity::Activity::new()
            .details(p.details.clone())
            .state(p.state.clone());
        if let Some(start) = p.start {
            a = a.timestamps(activity::Timestamps::new().start(start));
        }
        match c.set_activity(a) {
            Ok(()) => last = Some(p),
            Err(e) => {
                tracing::info!(error = %e, "discord: connection lost");
                client = None;
                last = None;
                next_try = Instant::now() + backoff;
            }
        }
    }
}
