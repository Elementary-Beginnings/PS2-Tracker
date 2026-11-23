//! SQLite persistence: saved sessions and the name cache.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::census::NameCache;
use crate::session::{Outcome, Session, SessionConfig};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("SQLite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("CSV: {0}")]
    Csv(#[from] csv::Error),
    #[error("file: {0}")]
    Io(#[from] std::io::Error),
    #[error("session {0} not found")]
    NotFound(i64),
}

/// Short form for the session list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: i64,
    pub character_id: String,
    pub character_name: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub kills: u32,
    pub deaths: u32,
    pub headshots: u32,
    pub xp: u64,
    /// Session data version (1 = recorded before 0.2.0: only kills, deaths
    /// and vehicles in detail).
    #[serde(default)]
    pub data_version: u32,
    #[serde(default)]
    pub faction_id: Option<u32>,
    /// Last server/continent of the session (data version 3, else `None`).
    #[serde(default)]
    pub world_id: Option<u32>,
    #[serde(default)]
    pub zone_id: Option<u32>,
    /// Seconds logged out within the resume window (from the session JSON);
    /// the history shows `ended_at - started_at - paused_secs` as duration.
    #[serde(default)]
    pub paused_secs: i64,
}

impl StoreError {
    /// The file is not a (readable) SQLite database.
    pub fn is_corruption(&self) -> bool {
        matches!(
            self,
            StoreError::Sql(rusqlite::Error::SqliteFailure(e, _))
                if matches!(e.code, rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt)
        )
    }
}

/// Current database schema (`PRAGMA user_version`).
pub const SCHEMA_VERSION: i32 = 3;

pub struct Store {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS sessions (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id   TEXT    NOT NULL,
    character_name TEXT    NOT NULL,
    started_at     INTEGER NOT NULL,
    ended_at       INTEGER,
    kills          INTEGER NOT NULL DEFAULT 0,
    deaths         INTEGER NOT NULL DEFAULT 0,
    headshots      INTEGER NOT NULL DEFAULT 0,
    xp             INTEGER NOT NULL DEFAULT 0,
    data           TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_started ON sessions(started_at DESC);
CREATE TABLE IF NOT EXISTS name_cache (
    kind       TEXT    NOT NULL,
    id         TEXT    NOT NULL,
    name       TEXT    NOT NULL,
    extra      INTEGER,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (kind, id)
);
"#;

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    /// Like [`open`](Self::open), but a damaged database file ("file is not a
    /// database", "database disk image is malformed") is renamed to
    /// `tracker.sqlite3.corrupt-<unix time>` (with its -wal/-shm files) and a
    /// new, empty one is created; the new name is returned. Other errors
    /// (permissions, full disk) stay errors.
    pub fn open_or_recover(
        path: &Path,
        now: i64,
    ) -> Result<(Self, Option<std::path::PathBuf>), StoreError> {
        match Self::open(path) {
            Ok(s) => Ok((s, None)),
            Err(e) if e.is_corruption() => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let aside = path.with_file_name(format!("{name}.corrupt-{now}"));
                std::fs::rename(path, &aside)?;
                for ext in ["-wal", "-shm"] {
                    let side = path.with_file_name(format!("{name}{ext}"));
                    if side.exists() {
                        let _ = std::fs::rename(
                            &side,
                            aside.with_file_name(format!("{name}.corrupt-{now}{ext}")),
                        );
                    }
                }
                Ok((Self::open(path)?, Some(aside)))
            }
            Err(e) => Err(e),
        }
    }

    pub fn in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.execute_batch(SCHEMA)?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Brings an older database to [`SCHEMA_VERSION`]. Idempotent; version 1
    /// databases keep all rows (their sessions get `data_version` 1).
    ///
    /// 1 → 2: column `data_version`, faction column for the session list and an
    /// index for duplicate detection on import (character + start time).
    /// 2 → 3: columns `world_id` and `zone_id` (server and continent).
    fn migrate(conn: &Connection) -> Result<(), StoreError> {
        let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version >= SCHEMA_VERSION {
            return Ok(());
        }
        let has = |col: &str| -> Result<bool, StoreError> {
            let mut st =
                conn.prepare("SELECT 1 FROM pragma_table_info('sessions') WHERE name=?1")?;
            Ok(st.exists([col])?)
        };
        let tx = conn.unchecked_transaction()?;
        if !has("data_version")? {
            tx.execute_batch(
                "ALTER TABLE sessions ADD COLUMN data_version INTEGER NOT NULL DEFAULT 1;",
            )?;
        }
        if !has("faction_id")? {
            tx.execute_batch("ALTER TABLE sessions ADD COLUMN faction_id INTEGER;")?;
        }
        for col in ["world_id", "zone_id"] {
            if !has(col)? {
                tx.execute_batch(&format!("ALTER TABLE sessions ADD COLUMN {col} INTEGER;"))?;
            }
        }
        tx.execute_batch(&format!(
            "CREATE INDEX IF NOT EXISTS sessions_char_start ON sessions(character_id, started_at);
             PRAGMA user_version = {SCHEMA_VERSION};"
        ))?;
        tx.commit()?;
        Ok(())
    }

    /// ID of a stored session of this character with this start time.
    pub fn find_session(
        &self,
        character_id: &str,
        started_at: i64,
    ) -> Result<Option<i64>, StoreError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT id FROM sessions WHERE character_id=?1 AND started_at=?2",
                params![character_id, started_at],
                |r| r.get(0),
            )
            .optional()?)
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Inserts or updates the session; sets `session.id`.
    pub fn save_session(&self, session: &mut Session) -> Result<i64, StoreError> {
        let data = serde_json::to_string(&*session)?;
        let c = self.conn();
        let s = &session.stats;
        let id = match session.id {
            Some(id) => {
                c.execute(
                    "UPDATE sessions SET character_name=?2, ended_at=?3, kills=?4, deaths=?5,
                     headshots=?6, xp=?7, data=?8, data_version=?9, faction_id=?10,
                     world_id=?11, zone_id=?12 WHERE id=?1",
                    params![
                        id,
                        session.character_name,
                        session.ended_at,
                        s.kills,
                        s.deaths,
                        s.headshots,
                        s.xp as i64,
                        data,
                        session.version(),
                        session.faction_id,
                        session.world_id,
                        session.zone_id
                    ],
                )?;
                id
            }
            None => {
                c.execute(
                    "INSERT INTO sessions (character_id, character_name, started_at, ended_at,
                     kills, deaths, headshots, xp, data, data_version, faction_id, world_id, zone_id)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                    params![
                        session.character_id.to_string(),
                        session.character_name,
                        session.started_at,
                        session.ended_at,
                        s.kills,
                        s.deaths,
                        s.headshots,
                        s.xp as i64,
                        data,
                        session.version(),
                        session.faction_id,
                        session.world_id,
                        session.zone_id
                    ],
                )?;
                c.last_insert_rowid()
            }
        };
        session.id = Some(id);
        Ok(id)
    }

    pub fn list_sessions(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<SessionSummary>, StoreError> {
        let c = self.conn();
        let mut st = c.prepare(&format!(
            "SELECT {SUMMARY_COLUMNS} FROM sessions ORDER BY started_at DESC LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = st.query_map(params![limit, offset], summary_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The most recently ended session of a character ("Resume last
    /// session"), if any.
    pub fn last_ended_session(
        &self,
        character_id: &str,
    ) -> Result<Option<SessionSummary>, StoreError> {
        Ok(self
            .conn()
            .query_row(
                &format!(
                    "SELECT {SUMMARY_COLUMNS} FROM sessions
                     WHERE character_id=?1 AND ended_at IS NOT NULL
                     ORDER BY ended_at DESC, id DESC LIMIT 1"
                ),
                [character_id],
                summary_row,
            )
            .optional()?)
    }

    pub fn load_session(&self, id: i64, config: SessionConfig) -> Result<Session, StoreError> {
        let data: Option<String> = self
            .conn()
            .query_row("SELECT data FROM sessions WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        let mut s: Session = serde_json::from_str(&data.ok_or(StoreError::NotFound(id))?)?;
        s.id = Some(id);
        s.restore(config);
        Ok(s)
    }

    /// Closes sessions that were still running at the last exit (crash, kill):
    /// end = last event. Returns the count.
    pub fn close_dangling(&self, config: SessionConfig) -> Result<usize, StoreError> {
        let ids: Vec<i64> = {
            let c = self.conn();
            let mut st = c.prepare("SELECT id FROM sessions WHERE ended_at IS NULL")?;
            st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
        };
        for id in &ids {
            let mut s = self.load_session(*id, config)?;
            s.end(s.last_event_at);
            self.save_session(&mut s)?;
        }
        Ok(ids.len())
    }

    /// Deletes all stored sessions of one character (squad: "remove and delete
    /// data"). Returns the number of sessions deleted.
    pub fn delete_sessions_of(&self, character_id: &str) -> Result<usize, StoreError> {
        Ok(self
            .conn()
            .execute("DELETE FROM sessions WHERE character_id=?1", [character_id])?)
    }

    pub fn delete_session(&self, id: i64) -> Result<(), StoreError> {
        let n = self
            .conn()
            .execute("DELETE FROM sessions WHERE id=?1", [id])?;
        if n == 0 {
            return Err(StoreError::NotFound(id));
        }
        Ok(())
    }

    pub fn load_names(&self, cache: &mut NameCache) -> Result<(), StoreError> {
        let c = self.conn();
        let mut st = c.prepare("SELECT kind, id, name, extra FROM name_cache")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let kind: String = r.get(0)?;
            let id: String = r.get(1)?;
            let name: String = r.get(2)?;
            let extra: Option<i64> = r.get(3)?;
            match kind.as_str() {
                "item" => {
                    if let Ok(i) = id.parse() {
                        cache.items.insert(i, name);
                    }
                }
                "vehicle" => {
                    if let Ok(i) = id.parse() {
                        cache.vehicles.insert(i, name);
                    }
                }
                "character" => {
                    if let Ok(i) = id.parse() {
                        cache
                            .characters
                            .insert(i, (name, extra.unwrap_or(0) as u32));
                    }
                }
                "world" => {
                    if let Ok(i) = id.parse() {
                        cache.worlds.insert(i, name);
                    }
                }
                "zone" => {
                    if let Ok(i) = id.parse() {
                        cache.zones.insert(i, name);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// item_id → item_category_id from the cache.
    pub fn item_category_ids(&self) -> Result<std::collections::HashMap<u32, u32>, StoreError> {
        let c = self.conn();
        let mut st =
            c.prepare("SELECT id, extra FROM name_cache WHERE kind='item' AND extra IS NOT NULL")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = std::collections::HashMap::new();
        for r in rows {
            let (id, cat) = r?;
            if let Ok(id) = id.parse() {
                out.insert(id, cat as u32);
            }
        }
        Ok(out)
    }

    pub fn save_names<'a>(
        &self,
        kind: &str,
        entries: impl IntoIterator<Item = (String, &'a str, Option<i64>)>,
        now: i64,
    ) -> Result<(), StoreError> {
        let mut c = self.conn();
        let tx = c.transaction()?;
        {
            let mut st = tx.prepare(
                "INSERT INTO name_cache (kind, id, name, extra, updated_at) VALUES (?1,?2,?3,?4,?5)
                 ON CONFLICT(kind, id) DO UPDATE SET name=excluded.name, extra=excluded.extra,
                 updated_at=excluded.updated_at",
            )?;
            for (id, name, extra) in entries {
                st.execute(params![kind, id, name, extra, now])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

/// Exports a session as JSON (complete, with resolved names).
/// Versioned session file (format `ps2-tracker-session`, see
/// docs/session-file.schema.json); can be imported again.
const SUMMARY_COLUMNS: &str =
    "id, character_id, character_name, started_at, ended_at, kills, deaths,
     headshots, xp, data_version, faction_id, world_id, zone_id,
     COALESCE(json_extract(data, '$.paused_secs'), 0)";

fn summary_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SessionSummary> {
    Ok(SessionSummary {
        id: r.get(0)?,
        character_id: r.get(1)?,
        character_name: r.get(2)?,
        started_at: r.get(3)?,
        ended_at: r.get(4)?,
        kills: r.get(5)?,
        deaths: r.get(6)?,
        headshots: r.get(7)?,
        xp: r.get::<_, i64>(8)? as u64,
        data_version: r.get(9)?,
        faction_id: r.get(10)?,
        world_id: r.get(11)?,
        zone_id: r.get(12)?,
        paused_secs: r.get(13)?,
    })
}

pub fn export_json(session: &Session, names: &NameCache) -> Result<String, StoreError> {
    let f = crate::sessionfile::SessionFile::new(session, names);
    Ok(serde_json::to_string_pretty(&f)?)
}

/// Exports the timeline as CSV (one row per kill/death/vehicle event).
pub fn export_csv(session: &Session, names: &NameCache) -> Result<String, StoreError> {
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record([
        "timestamp",
        "type",
        "other_id",
        "other_name",
        "weapon_id",
        "weapon_name",
        "headshot",
        "vehicle_id",
    ])?;
    for e in &session.timeline {
        let (kind, other, weapon, hs, vehicle): (&str, u64, u32, bool, u32) = match &e.outcome {
            Outcome::Kill {
                victim,
                weapon_id,
                vehicle_id,
                headshot,
            } => ("kill", *victim, *weapon_id, *headshot, *vehicle_id),
            Outcome::Death {
                killer,
                weapon_id,
                vehicle_id,
                headshot,
            } => ("death", *killer, *weapon_id, *headshot, *vehicle_id),
            Outcome::Teamkill { victim, weapon_id } => ("teamkill", *victim, *weapon_id, false, 0),
            Outcome::Teamkilled { killer, weapon_id } => {
                ("teamkilled", *killer, *weapon_id, false, 0)
            }
            Outcome::Suicide { weapon_id } => {
                ("suicide", session.character_id, *weapon_id, false, 0)
            }
            Outcome::VehicleKill {
                vehicle_id,
                owner,
                weapon_id,
            } => ("vehicle_kill", *owner, *weapon_id, false, *vehicle_id),
            Outcome::VehicleLost {
                vehicle_id,
                attacker,
            } => ("vehicle_lost", *attacker, 0, false, *vehicle_id),
            _ => continue,
        };
        let other_name = names
            .characters
            .get(&other)
            .map(|(n, _)| n.as_str())
            .unwrap_or("");
        let weapon_name = names.items.get(&weapon).map(String::as_str).unwrap_or("");
        w.write_record([
            e.timestamp.to_string(),
            kind.to_owned(),
            other.to_string(),
            other_name.to_owned(),
            weapon.to_string(),
            weapon_name.to_owned(),
            u8::from(hs).to_string(),
            vehicle.to_string(),
        ])?;
    }
    let bytes = w
        .into_inner()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Death, GameEvent};

    fn sample() -> Session {
        let mut s = Session::new(1, "Me", Some(3), 100, SessionConfig::default());
        s.apply(&GameEvent::Death(Death {
            timestamp: 110,
            world_id: 1,
            zone_id: 2,
            character_id: 2,
            attacker_character_id: 1,
            attacker_weapon_id: 80,
            attacker_vehicle_id: 0,
            attacker_loadout_id: 0,
            character_loadout_id: 0,
            is_headshot: true,
            team_id: Some(1),
            attacker_team_id: Some(3),
        }));
        s
    }

    #[test]
    fn session_roundtrip_and_list() {
        let st = Store::in_memory().unwrap();
        let mut s = sample();
        let id = st.save_session(&mut s).unwrap();
        s.end(200);
        st.save_session(&mut s).unwrap();
        let list = st.list_sessions(10, 0).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kills, 1);
        assert_eq!(list[0].ended_at, Some(200));
        let back = st.load_session(id, SessionConfig::default()).unwrap();
        assert_eq!(back.stats, s.stats);
        assert_eq!(back.killfeed().count(), 1);
        let mut open = sample();
        st.save_session(&mut open).unwrap();
        assert_eq!(st.close_dangling(SessionConfig::default()).unwrap(), 1);
        let l = st.list_sessions(10, 0).unwrap();
        assert!(l.iter().all(|x| x.ended_at.is_some()));
        assert_eq!(
            l.iter()
                .find(|x| x.id == open.id.unwrap())
                .unwrap()
                .ended_at,
            Some(110)
        );
        assert_eq!(st.delete_sessions_of("999").unwrap(), 0);
        st.delete_session(id).unwrap();
        assert!(matches!(
            st.load_session(id, SessionConfig::default()),
            Err(StoreError::NotFound(_))
        ));
    }

    #[test]
    fn last_ended_session_of_a_character() {
        let st = Store::in_memory().unwrap();
        assert!(st.last_ended_session("1").unwrap().is_none());
        let mut a = sample();
        a.end(200);
        st.save_session(&mut a).unwrap();
        let mut b = sample();
        b.started_at = 300;
        b.paused_secs = 30;
        b.end(400);
        st.save_session(&mut b).unwrap();
        // still running: not a candidate
        let mut running = sample();
        running.started_at = 500;
        st.save_session(&mut running).unwrap();
        // another character
        let mut other = Session::new(2, "Other", Some(1), 600, SessionConfig::default());
        other.end(700);
        st.save_session(&mut other).unwrap();
        let last = st.last_ended_session("1").unwrap().unwrap();
        assert_eq!(last.id, b.id.unwrap());
        assert_eq!(last.ended_at, Some(400));
        // the history duration leaves out the time logged out
        assert_eq!(last.paused_secs, 30);
        assert_eq!(
            st.last_ended_session("2").unwrap().unwrap().id,
            other.id.unwrap()
        );
    }

    #[test]
    fn migrates_a_version_1_database() {
        let conn = Connection::open_in_memory().unwrap();
        // schema as written by 0.1.x
        conn.execute_batch(
            "CREATE TABLE sessions (id INTEGER PRIMARY KEY AUTOINCREMENT, character_id TEXT NOT NULL,
             character_name TEXT NOT NULL, started_at INTEGER NOT NULL, ended_at INTEGER,
             kills INTEGER NOT NULL DEFAULT 0, deaths INTEGER NOT NULL DEFAULT 0,
             headshots INTEGER NOT NULL DEFAULT 0, xp INTEGER NOT NULL DEFAULT 0, data TEXT NOT NULL);
             PRAGMA user_version = 1;",
        )
        .unwrap();
        // a 0.1.x session: no data_version/events/xp_by_minute in the JSON
        let mut old = serde_json::to_value(sample()).unwrap();
        for k in ["data_version", "events", "xp_by_minute", "achievements"] {
            old.as_object_mut().unwrap().remove(k);
        }
        conn.execute(
            "INSERT INTO sessions (character_id, character_name, started_at, kills, data)
             VALUES ('1', 'Old', 100, 1, ?1)",
            [old.to_string()],
        )
        .unwrap();
        let st = Store::init(conn).unwrap();
        let v: i32 = st
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        let list = st.list_sessions(10, 0).unwrap();
        assert_eq!((list[0].data_version, list[0].kills), (1, 1));
        let s = st
            .load_session(list[0].id, SessionConfig::default())
            .unwrap();
        assert_eq!(s.version(), 1);
        assert!(s.events.is_empty());
        // running the migration again changes nothing
        Store::migrate(&st.conn()).unwrap();
        assert_eq!(st.find_session("1", 100).unwrap(), Some(list[0].id));
        assert_eq!(st.find_session("1", 101).unwrap(), None);
        // new sessions are written as version 2
        let mut n = sample();
        st.save_session(&mut n).unwrap();
        let l = st.list_sessions(10, 0).unwrap();
        assert!(
            l.iter()
                .any(|x| x.data_version == crate::session::SESSION_DATA_VERSION)
        );
    }

    #[test]
    fn migrates_a_version_2_database() {
        let conn = Connection::open_in_memory().unwrap();
        // schema as written by the 0.2.0 betas (user_version 2)
        conn.execute_batch(
            "CREATE TABLE sessions (id INTEGER PRIMARY KEY AUTOINCREMENT, character_id TEXT NOT NULL,
             character_name TEXT NOT NULL, started_at INTEGER NOT NULL, ended_at INTEGER,
             kills INTEGER NOT NULL DEFAULT 0, deaths INTEGER NOT NULL DEFAULT 0,
             headshots INTEGER NOT NULL DEFAULT 0, xp INTEGER NOT NULL DEFAULT 0, data TEXT NOT NULL,
             data_version INTEGER NOT NULL DEFAULT 1, faction_id INTEGER);
             PRAGMA user_version = 2;",
        )
        .unwrap();
        // a session of data version 2: no server/continent in the JSON
        let mut old = serde_json::to_value(sample()).unwrap();
        let o = old.as_object_mut().unwrap();
        o.insert("data_version".into(), 2.into());
        o.remove("world_id");
        o.remove("zone_id");
        conn.execute(
            "INSERT INTO sessions (character_id, character_name, started_at, kills, data, data_version, faction_id)
             VALUES ('1', 'Beta', 100, 1, ?1, 2, 3)",
            [old.to_string()],
        )
        .unwrap();
        let st = Store::init(conn).unwrap();
        let v: i32 = st
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        let list = st.list_sessions(10, 0).unwrap();
        assert_eq!(
            (list[0].data_version, list[0].world_id, list[0].zone_id),
            (2, None, None)
        );
        let s = st
            .load_session(list[0].id, SessionConfig::default())
            .unwrap();
        assert_eq!((s.version(), s.world_id), (2, None));
        Store::migrate(&st.conn()).unwrap();
        // a new session keeps server and continent
        let mut n = sample();
        n.world_id = Some(10);
        n.zone_id = Some(2);
        let id = st.save_session(&mut n).unwrap();
        let l = st.list_sessions(10, 0).unwrap();
        let row = l.iter().find(|x| x.id == id).unwrap();
        assert_eq!(
            (row.data_version, row.world_id, row.zone_id),
            (crate::session::SESSION_DATA_VERSION, Some(10), Some(2))
        );
        let back = st.load_session(id, SessionConfig::default()).unwrap();
        assert_eq!((back.world_id, back.zone_id), (Some(10), Some(2)));
    }

    #[test]
    fn damaged_database_is_set_aside() {
        let dir = std::env::temp_dir().join(format!("ps2t-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tracker.sqlite3");
        std::fs::write(&path, b"this is not a database at all, just text").unwrap();
        assert!(Store::open(&path).err().is_some_and(|e| e.is_corruption()));
        let (st, aside) = Store::open_or_recover(&path, 1234).unwrap();
        let aside = aside.expect("set aside");
        assert_eq!(aside.file_name().unwrap(), "tracker.sqlite3.corrupt-1234");
        assert!(std::fs::read(&aside).unwrap().starts_with(b"this is not"));
        // the new database works and a healthy one is opened as it is
        let mut s = sample();
        st.save_session(&mut s).unwrap();
        drop(st);
        let (again, none) = Store::open_or_recover(&path, 5678).unwrap();
        assert!(none.is_none());
        assert_eq!(again.list_sessions(10, 0).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_roundtrip() {
        let st = Store::in_memory().unwrap();
        st.save_names("item", [("80".to_string(), "Gauss SAW", None)], 1)
            .unwrap();
        st.save_names("character", [("2".to_string(), "Foe", Some(1))], 1)
            .unwrap();
        st.save_names("world", [("10".to_string(), "Miller", None)], 1)
            .unwrap();
        st.save_names("zone", [("2".to_string(), "Indar", None)], 1)
            .unwrap();
        let mut c = NameCache::default();
        st.load_names(&mut c).unwrap();
        assert_eq!(
            (c.worlds[&10].as_str(), c.zones[&2].as_str()),
            ("Miller", "Indar")
        );
        assert_eq!(c.items[&80], "Gauss SAW");
        assert_eq!(c.characters[&2], ("Foe".to_string(), 1));
    }

    #[test]
    fn exports() {
        let s = sample();
        let mut names = NameCache::default();
        names.items.insert(80, "Gauss SAW".into());
        let csv = export_csv(&s, &names).unwrap();
        assert!(
            csv.lines()
                .nth(1)
                .unwrap()
                .starts_with("110,kill,2,,80,Gauss SAW,1,0")
        );
        let json = export_json(&s, &names).unwrap();
        assert!(json.contains("\"Gauss SAW\""));
    }
}
