//! Upload queue: states and transitions of one queued session (pure logic;
//! the rows live in SQLite, see `Store::sync_*`).
//!
//! A session without a row is "not synced". Ending a session (or "Upload
//! now") puts it in the queue; the worker uploads due entries and records the
//! result here. Uploads are idempotent (`PUT` by character and start time), so
//! an interrupted upload is simply repeated and a session that changed after
//! its upload ("Resume last session") replaces the server copy.

use serde::{Deserialize, Serialize};

/// First retry after a transient failure.
pub const BACKOFF_START_SECS: i64 = 30;
/// Longest wait between two attempts.
pub const BACKOFF_MAX_SECS: i64 = 3600;
/// Upper bound for a server's `Retry-After`.
pub const RETRY_AFTER_MAX_SECS: i64 = 6 * 3600;
/// Longest error text kept per entry.
const MAX_ERROR_LEN: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    /// Waiting for the (next) upload attempt.
    Queued,
    /// The server has the current version.
    Synced,
    /// Rejected for a reason a retry does not fix (403/413/422, character
    /// not allowed); only "Retry" queues it again.
    Failed,
}

impl SyncState {
    pub fn as_str(self) -> &'static str {
        match self {
            SyncState::Queued => "queued",
            SyncState::Synced => "synced",
            SyncState::Failed => "failed",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(SyncState::Queued),
            "synced" => Some(SyncState::Synced),
            "failed" => Some(SyncState::Failed),
            _ => None,
        }
    }
}

/// One row of the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueEntry {
    pub session_id: i64,
    pub state: SyncState,
    /// Failed attempts since it was last queued.
    pub attempts: u32,
    /// Not before this time (Unix seconds).
    pub next_attempt_at: i64,
    /// Reason of the last failure (no tokens, no server bodies beyond a short
    /// message).
    pub last_error: Option<String>,
    pub synced_at: Option<i64>,
}

impl QueueEntry {
    /// Freshly queued: due now, counters reset.
    pub fn queued(session_id: i64, now: i64) -> Self {
        Self {
            session_id,
            state: SyncState::Queued,
            attempts: 0,
            next_attempt_at: now,
            last_error: None,
            synced_at: None,
        }
    }

    pub fn is_due(&self, now: i64) -> bool {
        self.state == SyncState::Queued && self.next_attempt_at <= now
    }
}

/// What an upload attempt came to (after the worker's one token refresh on
/// a 401).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    Uploaded,
    /// Worth repeating: network error, 5xx, 429, 401 after a refresh.
    Retry {
        reason: String,
        /// Seconds from `Retry-After`, if the server sent one.
        retry_after: Option<i64>,
    },
    /// A retry does not help (403, 413, 422, character not allowed).
    Rejected {
        reason: String,
    },
}

/// Wait after the `attempts`-th failure: 30 s, 60 s, 2 min … at most 1 h.
pub fn backoff_secs(attempts: u32) -> i64 {
    let shift = attempts.saturating_sub(1).min(16);
    (BACKOFF_START_SECS << shift).min(BACKOFF_MAX_SECS)
}

fn short(reason: &str) -> String {
    let mut s: String = reason.chars().take(MAX_ERROR_LEN).collect();
    if s.len() < reason.len() {
        s.push('…');
    }
    s
}

/// The entry after an attempt at `now`.
pub fn apply(mut e: QueueEntry, attempt: &Attempt, now: i64) -> QueueEntry {
    match attempt {
        Attempt::Uploaded => {
            e.state = SyncState::Synced;
            e.attempts = 0;
            e.last_error = None;
            e.synced_at = Some(now);
            e.next_attempt_at = now;
        }
        Attempt::Retry {
            reason,
            retry_after,
        } => {
            e.state = SyncState::Queued;
            e.attempts = e.attempts.saturating_add(1);
            let wait = match retry_after {
                Some(s) => (*s).clamp(1, RETRY_AFTER_MAX_SECS),
                None => backoff_secs(e.attempts),
            };
            e.next_attempt_at = now + wait;
            e.last_error = Some(short(reason));
        }
        Attempt::Rejected { reason } => {
            e.state = SyncState::Failed;
            e.attempts = e.attempts.saturating_add(1);
            e.last_error = Some(short(reason));
        }
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_is_capped() {
        assert_eq!(backoff_secs(0), 30);
        assert_eq!(backoff_secs(1), 30);
        assert_eq!(backoff_secs(2), 60);
        assert_eq!(backoff_secs(3), 120);
        assert_eq!(backoff_secs(8), 3600);
        assert_eq!(backoff_secs(u32::MAX), 3600);
    }

    #[test]
    fn transitions() {
        let e = QueueEntry::queued(7, 100);
        assert!(e.is_due(100));
        assert!(!e.is_due(99));
        let r = Attempt::Retry {
            reason: "HTTP 503".into(),
            retry_after: None,
        };
        let e = apply(e, &r, 100);
        assert_eq!(
            (e.state, e.attempts, e.next_attempt_at),
            (SyncState::Queued, 1, 130)
        );
        let e = apply(e, &r, 130);
        assert_eq!((e.attempts, e.next_attempt_at), (2, 190));
        // Retry-After wins over the backoff, within bounds
        let e = apply(
            e,
            &Attempt::Retry {
                reason: "HTTP 429".into(),
                retry_after: Some(5),
            },
            200,
        );
        assert_eq!(e.next_attempt_at, 205);
        let huge = apply(
            e.clone(),
            &Attempt::Retry {
                reason: "x".into(),
                retry_after: Some(i64::MAX),
            },
            0,
        );
        assert_eq!(huge.next_attempt_at, RETRY_AFTER_MAX_SECS);
        let done = apply(e.clone(), &Attempt::Uploaded, 300);
        assert_eq!(
            (
                done.state,
                done.attempts,
                done.synced_at,
                done.last_error.clone()
            ),
            (SyncState::Synced, 0, Some(300), None)
        );
        assert!(!done.is_due(10_000));
        let bad = apply(
            e,
            &Attempt::Rejected {
                reason: "HTTP 422: bad".into(),
            },
            300,
        );
        assert_eq!(bad.state, SyncState::Failed);
        assert!(
            !bad.is_due(10_000),
            "a rejected entry is not retried by itself"
        );
        assert_eq!(bad.last_error.as_deref(), Some("HTTP 422: bad"));
    }

    #[test]
    fn long_reasons_are_cut() {
        let e = apply(
            QueueEntry::queued(1, 0),
            &Attempt::Rejected {
                reason: "x".repeat(5000),
            },
            0,
        );
        assert!(e.last_error.unwrap().chars().count() <= MAX_ERROR_LEN + 1);
    }

    #[test]
    fn state_names_roundtrip() {
        for s in [SyncState::Queued, SyncState::Synced, SyncState::Failed] {
            assert_eq!(SyncState::parse(s.as_str()), Some(s));
        }
        assert_eq!(SyncState::parse("nope"), None);
    }
}
