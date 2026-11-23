//! Briefly subscribes to a server's event stream and prints the parsed events.
//!
//! PS2_SERVICE_ID=<id> cargo run -p ps2-tracker-core --example ess_probe -- [world_id] [sekunden]

use std::time::Duration;

use ps2_tracker_core::ess::{EssConfig, EssUpdate, run};
use tokio::sync::{mpsc, watch};

#[tokio::main]
async fn main() {
    let sid = std::env::var("PS2_SERVICE_ID").unwrap_or_else(|_| "example".into());
    let world: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(10);
    let secs: u64 = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(20);
    let mut cfg = EssConfig::new(sid, vec![]);
    cfg.probe_world = Some(world);
    let (tx, mut rx) = mpsc::channel(1024);
    let (stop_tx, stop_rx) = watch::channel(false);
    let (_chars_tx, chars_rx) = watch::channel(Vec::new());
    let task = tokio::spawn(run(cfg, tx, stop_rx, chars_rx));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut counts = std::collections::BTreeMap::<String, u32>::new();
    while let Ok(Some(u)) = tokio::time::timeout_at(deadline, rx.recv()).await {
        match u {
            EssUpdate::Event(e) => {
                let name = format!("{e:?}").split('(').next().unwrap_or("?").to_owned();
                let n = counts.entry(name).or_default();
                if *n < 2 {
                    println!("{e:?}");
                }
                *n += 1;
            }
            other => println!("{other:?}"),
        }
    }
    let _ = stop_tx.send(true);
    let _ = task.await;
    println!("summary: {counts:?}");
}
