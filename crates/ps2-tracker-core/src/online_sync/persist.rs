//! The upload queue in SQLite (table `sync_queue`, see `store::SCHEMA`).
//! Deleting a session deletes its row (foreign key, ON DELETE CASCADE).

use std::collections::HashMap;

use rusqlite::{OptionalExtension, params};

use super::queue::{QueueEntry, SyncState};
use crate::store::{Store, StoreError};

/// A due entry with what the upload needs to address it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueEntry {
    pub entry: QueueEntry,
    pub generation: i64,
    pub character_id: u64,
    pub started_at: i64,
}

/// Number of sessions per state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct QueueCounts {
    pub queued: u32,
    pub failed: u32,
    pub synced: u32,
}

fn entry_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<QueueEntry> {
    let state: String = r.get(1)?;
    Ok(QueueEntry {
        session_id: r.get(0)?,
        state: SyncState::parse(&state).unwrap_or(SyncState::Queued),
        attempts: r.get(2)?,
        next_attempt_at: r.get(3)?,
        last_error: r.get(4)?,
        synced_at: r.get(5)?,
    })
}

const ENTRY_COLUMNS: &str =
    "q.session_id, q.state, q.attempts, q.next_attempt_at, q.last_error, q.synced_at";

impl Store {
    /// Queues a session (again): due now, counters reset, new generation.
    /// Returns false if there is no such session.
    pub fn sync_enqueue(&self, session_id: i64, now: i64) -> Result<bool, StoreError> {
        let c = self.conn();
        let exists: bool = c
            .prepare("SELECT 1 FROM sessions WHERE id=?1")?
            .exists([session_id])?;
        if !exists {
            return Ok(false);
        }
        c.execute(
            "INSERT INTO sync_queue (session_id, state, attempts, next_attempt_at, generation, updated_at)
             VALUES (?1, 'queued', 0, ?2, 1, ?2)
             ON CONFLICT(session_id) DO UPDATE SET state='queued', attempts=0,
               next_attempt_at=?2, last_error=NULL, generation=generation+1, updated_at=?2",
            params![session_id, now],
        )?;
        Ok(true)
    }

    /// Queues every ended session of these characters that has no queue row
    /// yet ("Upload existing sessions"). Returns how many were queued.
    pub fn sync_enqueue_missing(
        &self,
        character_ids: &[u64],
        now: i64,
    ) -> Result<usize, StoreError> {
        let mut c = self.conn();
        let tx = c.transaction()?;
        let mut n = 0;
        {
            let mut st = tx.prepare(
                "INSERT INTO sync_queue (session_id, state, attempts, next_attempt_at, generation, updated_at)
                 SELECT s.id, 'queued', 0, ?2, 1, ?2 FROM sessions s
                 WHERE s.character_id=?1 AND s.ended_at IS NOT NULL
                   AND NOT EXISTS (SELECT 1 FROM sync_queue q WHERE q.session_id=s.id)",
            )?;
            for id in character_ids {
                n += st.execute(params![id.to_string(), now])?;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    /// Queued entries that are due at `now`, of ended sessions only (a
    /// session opened again waits for its new end), oldest first.
    pub fn sync_due(&self, now: i64, limit: u32) -> Result<Vec<DueEntry>, StoreError> {
        let c = self.conn();
        let mut st = c.prepare(&format!(
            "SELECT {ENTRY_COLUMNS}, q.generation, s.character_id, s.started_at
             FROM sync_queue q JOIN sessions s ON s.id = q.session_id
             WHERE q.state='queued' AND q.next_attempt_at <= ?1 AND s.ended_at IS NOT NULL
             ORDER BY q.next_attempt_at, s.started_at LIMIT ?2"
        ))?;
        let rows = st.query_map(params![now, limit], |r| {
            let cid: String = r.get(7)?;
            Ok(DueEntry {
                entry: entry_row(r)?,
                generation: r.get(6)?,
                character_id: cid.parse().unwrap_or(0),
                started_at: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Earliest `next_attempt_at` of a queued entry (for the worker's sleep).
    pub fn sync_next_due(&self) -> Result<Option<i64>, StoreError> {
        Ok(self.conn().query_row(
            "SELECT MIN(q.next_attempt_at) FROM sync_queue q JOIN sessions s ON s.id=q.session_id
             WHERE q.state='queued' AND s.ended_at IS NOT NULL",
            [],
            |r| r.get(0),
        )?)
    }

    /// Writes the result of an upload attempt, unless the session was queued
    /// again meanwhile (other generation). Returns whether it was written.
    pub fn sync_record(
        &self,
        e: &QueueEntry,
        generation: i64,
        now: i64,
    ) -> Result<bool, StoreError> {
        let n = self.conn().execute(
            "UPDATE sync_queue SET state=?2, attempts=?3, next_attempt_at=?4, last_error=?5,
             synced_at=?6, updated_at=?7 WHERE session_id=?1 AND generation=?8",
            params![
                e.session_id,
                e.state.as_str(),
                e.attempts,
                e.next_attempt_at,
                e.last_error,
                e.synced_at,
                now,
                generation
            ],
        )?;
        Ok(n == 1)
    }

    pub fn sync_entry(&self, session_id: i64) -> Result<Option<QueueEntry>, StoreError> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {ENTRY_COLUMNS} FROM sync_queue q WHERE q.session_id=?1"),
                [session_id],
                entry_row,
            )
            .optional()?)
    }

    /// All rows by session ID (the history shows the state per session).
    pub fn sync_entries(&self) -> Result<HashMap<i64, QueueEntry>, StoreError> {
        let c = self.conn();
        let mut st = c.prepare(&format!("SELECT {ENTRY_COLUMNS} FROM sync_queue q"))?;
        let rows = st.query_map([], entry_row)?;
        let mut out = HashMap::new();
        for r in rows {
            let e = r?;
            out.insert(e.session_id, e);
        }
        Ok(out)
    }

    /// Character of a stored session.
    pub fn session_character(&self, session_id: i64) -> Result<Option<u64>, StoreError> {
        let id: Option<String> = self
            .conn()
            .query_row(
                "SELECT character_id FROM sessions WHERE id=?1",
                [session_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(id.and_then(|s| s.parse().ok()))
    }

    /// Forgets every queue row (the server copies were deleted).
    pub fn sync_clear(&self) -> Result<usize, StoreError> {
        Ok(self.conn().execute("DELETE FROM sync_queue", [])?)
    }

    pub fn sync_counts(&self) -> Result<QueueCounts, StoreError> {
        let c = self.conn();
        let mut st = c.prepare("SELECT state, COUNT(*) FROM sync_queue GROUP BY state")?;
        let mut rows = st.query([])?;
        let mut out = QueueCounts::default();
        while let Some(r) = rows.next()? {
            let state: String = r.get(0)?;
            let n: u32 = r.get(1)?;
            match SyncState::parse(&state) {
                Some(SyncState::Queued) => out.queued = n,
                Some(SyncState::Failed) => out.failed = n,
                Some(SyncState::Synced) => out.synced = n,
                None => {}
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::online_sync::queue::{Attempt, apply};
    use crate::session::{Session, SessionConfig};

    fn ended(st: &Store, cid: u64, start: i64) -> i64 {
        let mut s = Session::new(cid, "Me", Some(1), start, SessionConfig::default());
        s.end(start + 100);
        st.save_session(&mut s).unwrap()
    }

    #[test]
    fn enqueue_due_record_and_counts() {
        let st = Store::in_memory().unwrap();
        let a = ended(&st, 1, 1_700_000_000);
        let b = ended(&st, 1, 1_700_001_000);
        assert!(!st.sync_enqueue(999, 10).unwrap(), "unknown session");
        assert!(st.sync_enqueue(a, 10).unwrap());
        assert!(st.sync_enqueue(b, 20).unwrap());
        assert!(st.sync_due(5, 10).unwrap().is_empty());
        let due = st.sync_due(20, 10).unwrap();
        assert_eq!(due.len(), 2);
        assert_eq!((due[0].entry.session_id, due[0].character_id), (a, 1));
        assert_eq!(due[0].started_at, 1_700_000_000);
        assert_eq!(st.sync_next_due().unwrap(), Some(10));
        // upload of a succeeds; b fails transiently
        let ok = apply(due[0].entry.clone(), &Attempt::Uploaded, 30);
        assert!(st.sync_record(&ok, due[0].generation, 30).unwrap());
        let retry = apply(
            due[1].entry.clone(),
            &Attempt::Retry {
                reason: "HTTP 503".into(),
                retry_after: None,
            },
            30,
        );
        assert!(st.sync_record(&retry, due[1].generation, 30).unwrap());
        assert!(st.sync_due(30, 10).unwrap().is_empty());
        assert_eq!(st.sync_due(60, 10).unwrap().len(), 1);
        assert_eq!(
            st.sync_counts().unwrap(),
            QueueCounts {
                queued: 1,
                failed: 0,
                synced: 1
            }
        );
        let all = st.sync_entries().unwrap();
        assert_eq!(all[&a].state, SyncState::Synced);
        assert_eq!(all[&b].last_error.as_deref(), Some("HTTP 503"));
        // deleting a session removes its row
        st.delete_session(b).unwrap();
        assert_eq!(st.sync_entry(b).unwrap(), None);
    }

    #[test]
    fn a_requeue_during_an_upload_wins() {
        let st = Store::in_memory().unwrap();
        let a = ended(&st, 1, 1_700_000_000);
        st.sync_enqueue(a, 10).unwrap();
        let due = st.sync_due(10, 1).unwrap().remove(0);
        // the session was opened again, ended and queued while uploading
        st.sync_enqueue(a, 15).unwrap();
        let ok = apply(due.entry.clone(), &Attempt::Uploaded, 20);
        assert!(!st.sync_record(&ok, due.generation, 20).unwrap());
        assert_eq!(st.sync_entry(a).unwrap().unwrap().state, SyncState::Queued);
    }

    #[test]
    fn running_sessions_are_not_due_and_existing_ones_are_queued_on_request() {
        let st = Store::in_memory().unwrap();
        let mut running = Session::new(1, "Me", Some(1), 1_700_000_000, SessionConfig::default());
        let r = st.save_session(&mut running).unwrap();
        st.sync_enqueue(r, 10).unwrap();
        assert!(st.sync_due(100, 10).unwrap().is_empty());
        assert_eq!(st.sync_next_due().unwrap(), None);
        let old = ended(&st, 1, 1_600_000_000);
        let other = ended(&st, 2, 1_600_000_000);
        let synced = ended(&st, 1, 1_650_000_000);
        st.sync_enqueue(synced, 1).unwrap();
        // only character 1, only ended sessions without a row
        assert_eq!(st.sync_enqueue_missing(&[1], 50).unwrap(), 1);
        assert!(st.sync_entry(old).unwrap().is_some());
        assert!(st.sync_entry(other).unwrap().is_none());
        assert_eq!(st.sync_enqueue_missing(&[1], 60).unwrap(), 0);
    }
}
