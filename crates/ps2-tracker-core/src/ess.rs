//! Client for the Event Streaming Service (WebSocket) with reconnect and backoff.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

use crate::events::{CharacterId, EssMessage, GameEvent, parse_message};
use crate::ratelimit::Backoff;

pub const ESS_URL: &str = "wss://push.planetside2.com/streaming";

/// Events the tracker subscribes to per character.
pub const CHARACTER_EVENTS: &[&str] = &[
    "Death",
    "VehicleDestroy",
    "GainExperience",
    "PlayerLogin",
    "PlayerLogout",
    "PlayerFacilityCapture",
    "PlayerFacilityDefend",
    "BattleRankUp",
    "AchievementEarned",
    "ItemAdded",
];

#[derive(Debug, Clone, PartialEq)]
pub enum EssUpdate {
    Connected,
    Disconnected { reason: String, retry_in: Duration },
    Event(GameEvent),
}

#[derive(Clone)]
pub struct EssConfig {
    pub service_id: String,
    pub characters: Vec<CharacterId>,
    /// No message (not even a heartbeat) for this long → reconnect.
    pub idle_timeout: Duration,
    pub url: String,
    /// Diagnostics only: subscribe to all characters of a server instead of single ones.
    pub probe_world: Option<u32>,
}

/// Without the service ID (logs).
impl std::fmt::Debug for EssConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EssConfig")
            .field("service_id", &"***")
            .field("characters", &self.characters)
            .field("idle_timeout", &self.idle_timeout)
            .field("url", &self.url)
            .field("probe_world", &self.probe_world)
            .finish()
    }
}

/// Release builds always use [`ESS_URL`]. Debug builds can point the
/// connection at a local test server with `PS2_TRACKER_ESS_URL` (end-to-end
/// tests with simulated events; tools/fake_ess.py).
fn default_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(u) = std::env::var("PS2_TRACKER_ESS_URL")
        && !u.is_empty()
    {
        return u;
    }
    ESS_URL.to_owned()
}

impl EssConfig {
    pub fn new(service_id: impl Into<String>, characters: Vec<CharacterId>) -> Self {
        Self {
            service_id: service_id.into(),
            characters,
            idle_timeout: Duration::from_secs(90),
            url: default_url(),
            probe_world: None,
        }
    }

    pub fn subscribe(&self) -> String {
        match self.probe_world {
            Some(w) => json!({
                "service": "event",
                "action": "subscribe",
                "characters": ["all"],
                "worlds": [w.to_string()],
                "eventNames": CHARACTER_EVENTS,
                "logicalAndCharactersWithWorlds": true,
            })
            .to_string(),
            None => subscribe_message(&self.characters),
        }
    }

    pub fn endpoint(&self) -> String {
        format!(
            "{}?environment=ps2&service-id=s:{}",
            self.url, self.service_id
        )
    }
}

/// Subscribe message for the given characters.
pub fn subscribe_message(characters: &[CharacterId]) -> String {
    json!({
        "service": "event",
        "action": "subscribe",
        "characters": characters.iter().map(u64::to_string).collect::<Vec<_>>(),
        "eventNames": CHARACTER_EVENTS,
    })
    .to_string()
}

/// Stops the events of these characters on the running connection.
pub fn clear_message(characters: &[CharacterId]) -> String {
    json!({
        "service": "event",
        "action": "clearSubscribe",
        "characters": characters.iter().map(u64::to_string).collect::<Vec<_>>(),
    })
    .to_string()
}

/// Characters to subscribe (`added`) and to clear (`removed`) when the list
/// changes from `old` to `new` (order and duplicates do not matter).
pub fn subscription_changes(
    old: &[CharacterId],
    new: &[CharacterId],
) -> (Vec<CharacterId>, Vec<CharacterId>) {
    use std::collections::BTreeSet;
    let o: BTreeSet<_> = old.iter().copied().collect();
    let n: BTreeSet<_> = new.iter().copied().collect();
    (
        n.difference(&o).copied().collect(),
        o.difference(&n).copied().collect(),
    )
}

/// Runs until `shutdown` becomes `true` or the receiver is dropped. A change
/// of `characters` (squad member added or removed) is applied to the running
/// connection with `subscribe`/`clearSubscribe`, without reconnecting.
pub async fn run(
    cfg: EssConfig,
    tx: mpsc::Sender<EssUpdate>,
    mut shutdown: watch::Receiver<bool>,
    characters: watch::Receiver<Vec<CharacterId>>,
) {
    let mut backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(60));
    loop {
        if *shutdown.borrow() {
            return;
        }
        let started = tokio::time::Instant::now();
        let reason = tokio::select! {
            r = session(&cfg, &tx, characters.clone()) => r,
            _ = shutdown.changed() => return,
        };
        // errors of the WebSocket library could quote the URL (logs, UI)
        let reason = crate::service_id::redact(&reason, &cfg.service_id);
        if tx.is_closed() {
            return;
        }
        // A connection that was stable for a while resets the backoff.
        if started.elapsed() > Duration::from_secs(60) {
            backoff.reset();
        }
        let retry_in = backoff.next_delay();
        tracing::warn!(%reason, ?retry_in, "ESS disconnected");
        if tx
            .send(EssUpdate::Disconnected { reason, retry_in })
            .await
            .is_err()
        {
            return;
        }
        tokio::select! {
            _ = tokio::time::sleep(retry_in) => {}
            _ = shutdown.changed() => return,
        }
    }
}

async fn session(
    cfg: &EssConfig,
    tx: &mpsc::Sender<EssUpdate>,
    mut characters: watch::Receiver<Vec<CharacterId>>,
) -> String {
    let (mut ws, _) = match tokio_tungstenite::connect_async(cfg.endpoint()).await {
        Ok(x) => x,
        Err(e) => return format!("connection failed: {e}"),
    };
    // the current list, not the one from the start of the task
    let mut current = characters.borrow_and_update().clone();
    if current.is_empty() && cfg.probe_world.is_none() {
        return "no characters configured".into();
    }
    let first = match cfg.probe_world {
        Some(_) => cfg.subscribe(),
        None => subscribe_message(&current),
    };
    if let Err(e) = ws.send(Message::text(first)).await {
        return format!("subscribe failed: {e}");
    }
    if tx.send(EssUpdate::Connected).await.is_err() {
        return "receiver closed".into();
    }
    let mut list_open = true;
    loop {
        let msg = tokio::select! {
            r = tokio::time::timeout(cfg.idle_timeout, ws.next()) => match r {
                Err(_) => return "no message within the time limit".into(),
                Ok(None) => return "connection closed by server".into(),
                Ok(Some(Err(e))) => return format!("WebSocket error: {e}"),
                Ok(Some(Ok(m))) => m,
            },
            changed = characters.changed(), if list_open => {
                if changed.is_err() {
                    list_open = false;
                    continue;
                }
                let new = characters.borrow_and_update().clone();
                let (added, removed) = subscription_changes(&current, &new);
                if !added.is_empty()
                    && let Err(e) = ws.send(Message::text(subscribe_message(&added))).await
                {
                    return format!("subscribe failed: {e}");
                }
                if !removed.is_empty()
                    && let Err(e) = ws.send(Message::text(clear_message(&removed))).await
                {
                    return format!("unsubscribe failed: {e}");
                }
                tracing::info!(
                    added = added.len(),
                    removed = removed.len(),
                    total = new.len(),
                    "ESS subscription updated"
                );
                current = new;
                continue;
            }
        };
        match msg {
            Message::Text(t) => match parse_message(t.as_str()) {
                Ok(EssMessage::Event(ev)) => {
                    if tx.send(EssUpdate::Event(ev)).await.is_err() {
                        return "receiver closed".into();
                    }
                }
                Ok(EssMessage::ConnectionState { connected: false }) => {
                    return "server reports disconnected".into();
                }
                Ok(_) => {}
                Err(e) => tracing::debug!(error = %e, "ESS message not understood"),
            },
            Message::Ping(p) => {
                let _ = ws.send(Message::Pong(p)).await;
            }
            Message::Close(_) => return "close frame received".into(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_diff_and_clear_payload() {
        assert_eq!(
            subscription_changes(&[1, 2], &[2, 3, 3]),
            (vec![3], vec![1])
        );
        assert_eq!(subscription_changes(&[1], &[1]), (vec![], vec![]));
        let m: serde_json::Value = serde_json::from_str(&clear_message(&[7])).unwrap();
        assert_eq!(m["action"], "clearSubscribe");
        assert_eq!(m["characters"], json!(["7"]));
    }

    /// A squad member added at run time is subscribed on the open connection
    /// (no reconnect); a removed one is cleared.
    #[tokio::test]
    async fn live_subscription_changes() {
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (got_tx, mut got) = mpsc::channel::<(usize, serde_json::Value)>(16);
        tokio::spawn(async move {
            let mut conn = 0;
            loop {
                let (sock, _) = listener.accept().await.unwrap();
                conn += 1;
                let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
                let got_tx = got_tx.clone();
                let n = conn;
                tokio::spawn(async move {
                    while let Some(Ok(Message::Text(t))) = ws.next().await {
                        let v: serde_json::Value = serde_json::from_str(t.as_str()).unwrap();
                        let _ = got_tx.send((n, v)).await;
                    }
                });
            }
        });
        let mut cfg = EssConfig::new("x", vec![1]);
        cfg.url = format!("ws://{addr}/streaming");
        let (tx, mut rx) = mpsc::channel(16);
        let (_stop_tx, stop_rx) = watch::channel(false);
        let (chars_tx, chars_rx) = watch::channel(vec![1u64]);
        tokio::spawn(run(cfg, tx, stop_rx, chars_rx));
        assert!(matches!(rx.recv().await, Some(EssUpdate::Connected)));
        let (c, first) = got.recv().await.unwrap();
        assert_eq!(
            (c, first["action"].as_str(), first["characters"].clone()),
            (1, Some("subscribe"), json!(["1"]))
        );
        chars_tx.send(vec![1, 77]).unwrap();
        let (c, add) = got.recv().await.unwrap();
        assert_eq!(
            (c, add["action"].as_str(), add["characters"].clone()),
            (1, Some("subscribe"), json!(["77"]))
        );
        chars_tx.send(vec![77]).unwrap();
        let (c, clear) = got.recv().await.unwrap();
        assert_eq!(
            (c, clear["action"].as_str(), clear["characters"].clone()),
            (1, Some("clearSubscribe"), json!(["1"]))
        );
    }

    #[test]
    fn subscribe_payload() {
        let m: serde_json::Value = serde_json::from_str(&subscribe_message(&[1, 2])).unwrap();
        assert_eq!(m["action"], "subscribe");
        assert_eq!(m["characters"], json!(["1", "2"]));
        assert!(
            m["eventNames"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e == "Death")
        );
    }

    #[test]
    fn endpoint_contains_service_id() {
        let c = EssConfig::new("abc", vec![]);
        assert_eq!(
            c.endpoint(),
            "wss://push.planetside2.com/streaming?environment=ps2&service-id=s:abc"
        );
    }

    /// Neither the debug output nor a disconnect reason carries the ID.
    #[tokio::test]
    async fn service_id_not_in_logs_or_reasons() {
        let mut c = EssConfig::new("secret0sid", vec![1]);
        assert!(!format!("{c:?}").contains("secret0sid"));
        // nothing listens on port 9: the connection fails at once
        c.url = "ws://127.0.0.1:9/streaming".into();
        let (tx, mut rx) = mpsc::channel(4);
        let (stop_tx, stop_rx) = watch::channel(false);
        let (_chars_tx, chars_rx) = watch::channel(vec![1]);
        let task = tokio::spawn(run(c, tx, stop_rx, chars_rx));
        match rx.recv().await {
            Some(EssUpdate::Disconnected { reason, .. }) => {
                assert!(!reason.contains("secret0sid"), "{reason}")
            }
            other => panic!("{other:?}"),
        }
        stop_tx.send(true).unwrap();
        task.await.unwrap();
    }
}
