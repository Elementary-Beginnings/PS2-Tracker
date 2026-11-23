//! Session files: the versioned JSON format (export/import, schema in
//! docs/session-file.schema.json) and the Excel export, which carries the same
//! JSON inside the .xlsx so it can be imported again.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

use base64::Engine as _;
use rust_xlsxwriter::{
    ExcelDateTime, Format, Table, TableColumn, TableStyle, Workbook, Worksheet, XlsxError,
};
use serde::{Deserialize, Serialize};

use crate::analysis::{Analysis, analyse};
use crate::census::NameCache;
use crate::session::{Rates, SESSION_DATA_VERSION, Session};

/// `format` value of a session file.
pub const FORMAT: &str = "ps2-tracker-session";
/// Version of the file format written by this build.
pub const FILE_VERSION: u32 = 1;
/// Largest file accepted for import (JSON or .xlsx).
pub const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;
/// Largest JSON inside an .xlsx after unpacking.
pub const MAX_JSON_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 200_000;
/// Part inside the .xlsx (zip) that carries the session JSON (base64).
const XLSX_PART: &str = "customXml/ps2-tracker-session.xml";

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("file: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a session file: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Excel: {0}")]
    Xlsx(#[from] XlsxError),
    #[error("Excel file: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Invalid(String),
}

fn invalid(s: impl Into<String>) -> FileError {
    FileError::Invalid(s.into())
}

/// Names needed to read the session on another computer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Names {
    #[serde(default)]
    pub characters: BTreeMap<String, String>,
    #[serde(default)]
    pub items: BTreeMap<u32, String>,
    #[serde(default)]
    pub vehicles: BTreeMap<u32, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionFile {
    pub format: String,
    pub version: u32,
    pub generator: String,
    pub session: Session,
    /// Derived values (kd, kd_revived, kpm, hsr, xp_per_hour); ignored on import.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rates: Option<Rates>,
    #[serde(default)]
    pub names: Names,
}

impl SessionFile {
    pub fn new(session: &Session, names: &NameCache) -> Self {
        let mut n = Names::default();
        let mut chars = vec![session.character_id];
        for e in session.timeline.iter().chain(&session.events) {
            if let Some((c, w)) = e.outcome.other_and_weapon() {
                chars.push(c);
                if let Some(x) = names.items.get(&w) {
                    n.items.insert(w, x.clone());
                }
            }
        }
        for c in chars {
            if let Some((x, _)) = names.characters.get(&c) {
                n.characters.insert(c.to_string(), x.clone());
            }
        }
        for id in session.weapons.keys() {
            if let Some(x) = names.items.get(id) {
                n.items.insert(*id, x.clone());
            }
        }
        for id in session.vehicle_kills_by_type.keys() {
            if let Some(x) = names.vehicles.get(id) {
                n.vehicles.insert(*id, x.clone());
            }
        }
        Self {
            format: FORMAT.into(),
            version: FILE_VERSION,
            generator: concat!("ps2-tracker ", env!("CARGO_PKG_VERSION")).into(),
            rates: Some(session.rates(session.ended_at.unwrap_or(session.last_event_at))),
            session: session.clone(),
            names: n,
        }
    }

    /// Names of the file merged over the local cache (local names win).
    pub fn name_cache(&self, local: &NameCache) -> NameCache {
        let mut c = NameCache::default();
        for (id, n) in &self.names.characters {
            if let Ok(id) = id.parse() {
                c.characters.insert(id, (n.clone(), 0));
            }
        }
        c.items.extend(self.names.items.clone());
        c.vehicles.extend(self.names.vehicles.clone());
        c.characters.extend(local.characters.clone());
        c.items.extend(local.items.clone());
        c.vehicles.extend(local.vehicles.clone());
        c
    }

    /// Checks a file before it is imported: format, version, sizes and values.
    pub fn validate(&self, now: i64) -> Result<(), FileError> {
        if self.format != FORMAT {
            return Err(invalid(format!("unknown format {:?}", self.format)));
        }
        if self.version == 0 || self.version > FILE_VERSION {
            return Err(invalid(format!(
                "file version {} is newer than this app supports ({FILE_VERSION}) – update the tracker",
                self.version
            )));
        }
        let s = &self.session;
        if s.data_version > SESSION_DATA_VERSION {
            return Err(invalid("session data is newer than this app supports"));
        }
        if s.character_id == 0 || s.character_name.trim().is_empty() || s.character_name.len() > 64
        {
            return Err(invalid("missing or invalid character"));
        }
        // PlanetSide 2 launched in November 2012
        let earliest = 1_350_000_000;
        if s.started_at < earliest || s.started_at > now + 86_400 {
            return Err(invalid("start time out of range"));
        }
        if s.ended_at
            .is_some_and(|e| e < s.started_at || e > now + 86_400)
            || s.last_event_at < s.started_at
        {
            return Err(invalid("end time out of range"));
        }
        if s.timeline.len() > MAX_ENTRIES
            || s.events.len() > MAX_ENTRIES
            || s.weapons.len() > 10_000
            || s.xp_by_minute.len() > 100_000
            || s.achievements.len() > 10_000
        {
            return Err(invalid("too many entries"));
        }
        let names = &self.names;
        if names.characters.len() + names.items.len() + names.vehicles.len() > 3 * MAX_ENTRIES
            || names
                .characters
                .values()
                .chain(names.items.values())
                .chain(names.vehicles.values())
                .any(|n| n.len() > 128)
        {
            return Err(invalid("invalid names"));
        }
        Ok(())
    }
}

/// The JSON export (pretty, versioned).
pub fn to_json(file: &SessionFile) -> Result<String, FileError> {
    Ok(serde_json::to_string_pretty(file)?)
}

/// Parses a session file: the current format, or the JSON export of 0.1.x
/// (`generator` + `session`, no `format`).
pub fn from_json(text: &str) -> Result<SessionFile, FileError> {
    let v: serde_json::Value = serde_json::from_str(text)?;
    if v.get("format").is_none() && v.get("session").is_some() {
        let legacy = v.get("generator").and_then(|g| g.as_str()).unwrap_or("");
        if !legacy.starts_with("ps2-tracker") {
            return Err(invalid("not a PS2 Session Tracker file"));
        }
        let session: Session = serde_json::from_value(v["session"].clone())?;
        return Ok(SessionFile {
            format: FORMAT.into(),
            version: 1,
            generator: legacy.into(),
            session,
            rates: None,
            names: Names::default(),
        });
    }
    Ok(serde_json::from_value(v)?)
}

/// Reads a .json or .xlsx session file (detected by content, not the name).
pub fn read_file(path: &Path) -> Result<SessionFile, FileError> {
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() {
        return Err(invalid("not a file"));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(invalid(format!(
            "file larger than {} MB",
            MAX_FILE_BYTES / 1024 / 1024
        )));
    }
    let bytes = std::fs::read(path)?;
    if bytes.starts_with(b"PK") {
        return from_xlsx(&bytes);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid("neither JSON nor .xlsx"))?;
    from_json(text)
}

fn from_xlsx(bytes: &[u8]) -> Result<SessionFile, FileError> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut part = zip.by_name(XLSX_PART).map_err(|_| {
        invalid(
            "this .xlsx has no embedded session data – only files exported by the tracker can be \
             imported (a spreadsheet program that re-saves the file drops it)",
        )
    })?;
    if part.size() > MAX_JSON_BYTES {
        return Err(invalid("embedded data too large"));
    }
    let mut xml = String::new();
    part.by_ref()
        .take(MAX_JSON_BYTES)
        .read_to_string(&mut xml)?;
    let b64 = xml
        .split_once("<data>")
        .and_then(|(_, r)| r.split_once("</data>"))
        .map(|(d, _)| d.trim())
        .ok_or_else(|| invalid("embedded session data unreadable"))?;
    let json = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|_| invalid("embedded session data unreadable"))?;
    from_json(std::str::from_utf8(&json).map_err(|_| invalid("embedded session data unreadable"))?)
}

// ------------------------------------------------------------------ Excel

enum Cell {
    Text(String),
    Num(f64),
    Time(i64),
}

struct Fmt {
    head: Format,
    int: Format,
    dec: Format,
    pct: Format,
    time: Format,
}

fn table(
    ws: &mut Worksheet,
    name: &str,
    f: &Fmt,
    cols: &[(&str, &Format)],
    rows: Vec<Vec<Cell>>,
) -> Result<(), XlsxError> {
    ws.set_name(name)?;
    let n = rows.len().max(1) as u32;
    for (r, row) in rows.into_iter().enumerate() {
        let r = r as u32 + 1;
        for (c, cell) in row.into_iter().enumerate() {
            let c = c as u16;
            let fmt = cols.get(c as usize).map(|x| x.1).unwrap_or(&f.int);
            match cell {
                Cell::Text(s) => ws.write_string(r, c, s).map(|_| ())?,
                Cell::Num(v) => ws.write_number_with_format(r, c, v, fmt).map(|_| ())?,
                Cell::Time(t) => ws
                    .write_datetime_with_format(r, c, ExcelDateTime::from_timestamp(t)?, &f.time)
                    .map(|_| ())?,
            }
        }
    }
    let columns: Vec<TableColumn> = cols
        .iter()
        .map(|(h, _)| TableColumn::new().set_header(*h).set_header_format(&f.head))
        .collect();
    let t = Table::new()
        .set_name(
            name.chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect::<String>(),
        )
        .set_style(TableStyle::Medium2)
        .set_columns(&columns);
    ws.add_table(0, 0, n, cols.len() as u16 - 1, &t)?;
    ws.set_freeze_panes(1, 0)?;
    ws.autofit();
    Ok(())
}

/// Class name for the export; "unknown" for events without a known loadout.
fn class_name(c: Option<crate::classes::Class>) -> String {
    c.map_or_else(|| "unknown".into(), |c| c.name().into())
}

fn team_name(id: Option<u32>) -> String {
    match id {
        Some(1) => "VS".into(),
        Some(2) => "NC".into(),
        Some(3) => "TR".into(),
        Some(4) => "NSO".into(),
        Some(x) => format!("team {x}"),
        None => String::new(),
    }
}

/// Builds the Excel workbook (8 sheets with real tables) and embeds the
/// session JSON for a later import.
pub fn to_xlsx(file: &SessionFile, local: &NameCache) -> Result<Vec<u8>, FileError> {
    let names = file.name_cache(local);
    let a: Analysis = analyse(&file.session, &names);
    let mut wb = Workbook::new();
    let f = Fmt {
        head: Format::new().set_bold(),
        int: Format::new().set_num_format("0"),
        dec: Format::new().set_num_format("0.00"),
        pct: Format::new().set_num_format("0.0%"),
        time: Format::new().set_num_format("yyyy-mm-dd hh:mm:ss"),
    };
    let txt = Format::new();
    let s = &a.stats;
    let r = &a.rates;
    let dur = (a.ended_at.unwrap_or(file.session.last_event_at) - a.started_at).max(0);
    let summary: Vec<(&str, Cell, &Format)> = vec![
        ("Character", Cell::Text(a.character_name.clone()), &txt),
        ("Faction", Cell::Text(team_name(a.faction_id)), &txt),
        ("Start (UTC)", Cell::Time(a.started_at), &f.time),
        (
            "End (UTC)",
            a.ended_at.map_or(Cell::Text(String::new()), Cell::Time),
            &f.time,
        ),
        ("Duration (min)", Cell::Num(dur as f64 / 60.0), &f.dec),
        ("Kills", Cell::Num(s.kills.into()), &f.int),
        ("Deaths", Cell::Num(s.deaths.into()), &f.int),
        ("Headshots", Cell::Num(s.headshots.into()), &f.int),
        ("K/D", Cell::Num(r.kd), &f.dec),
        ("K/D incl. revives", Cell::Num(r.kd_revived), &f.dec),
        ("KPM", Cell::Num(r.kpm), &f.dec),
        ("HSR", Cell::Num(r.hsr), &f.pct),
        ("XP", Cell::Num(s.xp as f64), &f.int),
        ("XP per hour", Cell::Num(r.xp_per_hour), &f.int),
        ("Best streak", Cell::Num(a.best_streak.into()), &f.int),
        ("Revives given", Cell::Num(s.revives.into()), &f.int),
        ("Revived", Cell::Num(s.revived.into()), &f.int),
        ("Assists", Cell::Num(s.assists.into()), &f.int),
        ("Teamkills", Cell::Num(s.teamkills.into()), &f.int),
        ("Suicides", Cell::Num(s.suicides.into()), &f.int),
        (
            "Vehicles destroyed",
            Cell::Num(s.vehicle_kills.into()),
            &f.int,
        ),
        ("Vehicles lost", Cell::Num(s.vehicles_lost.into()), &f.int),
        ("Data version", Cell::Num(a.data_version.into()), &f.int),
    ];
    {
        let ws = wb.add_worksheet();
        ws.set_name("Summary")?;
        ws.write_string_with_format(0, 0, "Value", &f.head)?;
        ws.write_string_with_format(0, 1, "", &f.head)?;
        let n = summary.len() as u32;
        for (i, (label, cell, fmt)) in summary.into_iter().enumerate() {
            let row = i as u32 + 1;
            ws.write_string(row, 0, label)?;
            match cell {
                Cell::Text(t) => ws.write_string(row, 1, t).map(|_| ())?,
                Cell::Num(v) => ws.write_number_with_format(row, 1, v, fmt).map(|_| ())?,
                Cell::Time(t) => ws
                    .write_datetime_with_format(row, 1, ExcelDateTime::from_timestamp(t)?, fmt)
                    .map(|_| ())?,
            }
        }
        let t = Table::new()
            .set_name("Summary")
            .set_style(TableStyle::Medium2)
            .set_columns(&[
                TableColumn::new()
                    .set_header("Value")
                    .set_header_format(&f.head),
                TableColumn::new()
                    .set_header("Session")
                    .set_header_format(&f.head),
            ]);
        ws.add_table(0, 0, n, 1, &t)?;
        if a.data_version < 2 {
            ws.write_string(n + 2, 0, "Recorded before 0.2.0: revives, assists, achievements, XP over time and opponent factions were not stored.")?;
        }
        ws.autofit();
    }
    let ev_cols = [
        ("Time (UTC)", &f.time),
        ("Type", &txt),
        ("Player", &txt),
        ("Player ID", &txt),
        ("Weapon", &txt),
        ("Headshot", &txt),
        ("Vehicle", &txt),
        ("Faction", &txt),
        ("Enemy class", &txt),
        ("My class", &txt),
    ];
    let ev_rows = |list: &[crate::analysis::EventLine]| -> Vec<Vec<Cell>> {
        list.iter()
            .map(|e| {
                vec![
                    Cell::Time(e.t),
                    Cell::Text(e.kind.clone()),
                    Cell::Text(e.other_name.clone()),
                    Cell::Text(e.other_id.clone()),
                    Cell::Text(e.weapon.clone()),
                    Cell::Text(if e.headshot {
                        "yes".into()
                    } else {
                        String::new()
                    }),
                    Cell::Text(e.vehicle.clone()),
                    Cell::Text(team_name(e.team)),
                    Cell::Text(class_name(e.enemy_class)),
                    Cell::Text(class_name(e.my_class)),
                ]
            })
            .collect()
    };
    table(wb.add_worksheet(), "Kills", &f, &ev_cols, ev_rows(&a.kills))?;
    table(
        wb.add_worksheet(),
        "Deaths",
        &f,
        &ev_cols,
        ev_rows(&a.deaths),
    )?;
    // by class: enemy classes, then own class use (empty classes: sessions
    // before data version 4)
    let mut class_rows: Vec<Vec<Cell>> = a
        .classes
        .enemy
        .iter()
        .map(|r| {
            vec![
                Cell::Text("Enemy".into()),
                Cell::Text(class_name(r.class)),
                Cell::Num(r.kills.into()),
                Cell::Num(r.deaths.into()),
                Cell::Text(String::new()),
            ]
        })
        .collect();
    class_rows.extend(a.classes.own.iter().map(|r| {
        vec![
            Cell::Text("Me".into()),
            Cell::Text(class_name(r.class)),
            Cell::Num(r.kills.into()),
            Cell::Num(r.deaths.into()),
            Cell::Num(r.secs as f64 / 60.0),
        ]
    }));
    table(
        wb.add_worksheet(),
        "Classes",
        &f,
        &[
            ("Side", &txt),
            ("Class", &txt),
            ("Kills", &f.int),
            ("Deaths", &f.int),
            ("Minutes (approx.)", &f.dec),
        ],
        class_rows,
    )?;
    table(
        wb.add_worksheet(),
        "Weapons",
        &f,
        &[
            ("Weapon", &txt),
            ("ID", &f.int),
            ("Kills", &f.int),
            ("Headshots", &f.int),
            ("HSR", &f.pct),
            ("Deaths by", &f.int),
        ],
        a.weapons
            .iter()
            .map(|w| {
                vec![
                    Cell::Text(w.name.clone()),
                    Cell::Num(w.id.into()),
                    Cell::Num(w.kills.into()),
                    Cell::Num(w.headshots.into()),
                    Cell::Num(w.hsr),
                    Cell::Num(w.deaths_by.into()),
                ]
            })
            .collect(),
    )?;
    table(
        wb.add_worksheet(),
        "Vehicles",
        &f,
        &[("Vehicle", &txt), ("ID", &f.int), ("Destroyed", &f.int)],
        a.vehicles
            .iter()
            .map(|v| {
                vec![
                    Cell::Text(v.name.clone()),
                    Cell::Num(v.id.into()),
                    Cell::Num(v.kills.into()),
                ]
            })
            .collect(),
    )?;
    table(
        wb.add_worksheet(),
        "Revives & Assists",
        &f,
        &[
            ("Time (UTC)", &f.time),
            ("Type", &txt),
            ("Player", &txt),
            ("Player ID", &txt),
            ("Detail", &txt),
        ],
        a.support
            .iter()
            .map(|e| {
                vec![
                    Cell::Time(e.t),
                    Cell::Text(e.kind.clone()),
                    Cell::Text(e.other_name.clone()),
                    Cell::Text(e.other_id.clone()),
                    Cell::Text(e.detail.clone()),
                ]
            })
            .collect(),
    )?;
    table(
        wb.add_worksheet(),
        "Achievements",
        &f,
        &[
            ("Time (UTC)", &f.time),
            ("Achievement", &txt),
            ("Description", &txt),
        ],
        a.achievements
            .iter()
            .map(|u| {
                vec![
                    Cell::Time(u.timestamp),
                    Cell::Text(u.name.clone()),
                    Cell::Text(u.description.clone()),
                ]
            })
            .collect(),
    )?;
    table(
        wb.add_worksheet(),
        "Timeline",
        &f,
        &[
            ("Minute", &f.int),
            ("Kills", &f.int),
            ("Deaths", &f.int),
            ("Headshots", &f.int),
            ("XP", &f.int),
            ("K/D", &f.dec),
            ("HSR", &f.pct),
        ],
        a.series
            .iter()
            .map(|p| {
                vec![
                    Cell::Num(p.minute as f64),
                    Cell::Num(p.kills.into()),
                    Cell::Num(p.deaths.into()),
                    Cell::Num(p.headshots.into()),
                    Cell::Num(p.xp as f64),
                    Cell::Num(p.kd),
                    Cell::Num(p.hsr),
                ]
            })
            .collect(),
    )?;
    let book = wb.save_to_buffer()?;
    embed(&book, &to_json(file)?)
}

/// Copies the workbook and adds the session JSON as an extra XML part. It is
/// not referenced by the workbook, so spreadsheet programs ignore it.
fn embed(book: &[u8], json: &str) -> Result<Vec<u8>, FileError> {
    let mut src = zip::ZipArchive::new(std::io::Cursor::new(book))?;
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let f = src.by_index_raw(i)?;
        out.raw_copy_file(f)?;
    }
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    out.start_file(XLSX_PART, opts)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(json.as_bytes());
    write!(
        out,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<session xmlns=\"urn:ps2-tracker:session\" format=\"{FORMAT}\" version=\"{FILE_VERSION}\"><data>{b64}</data></session>"
    )?;
    Ok(out.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Death, GameEvent};
    use crate::session::SessionConfig;

    const ME: u64 = 5428000000000000042;
    const FOE: u64 = 5429000000000000001;
    const NOW: i64 = 1_795_300_000;

    fn sample() -> Session {
        let mut s = Session::new(
            ME,
            "SampleSoldier",
            Some(3),
            1_795_200_000,
            SessionConfig::default(),
        );
        s.apply(&GameEvent::Death(Death {
            timestamp: 1_795_200_010,
            world_id: 10,
            zone_id: 2,
            character_id: FOE,
            attacker_character_id: ME,
            attacker_weapon_id: 80,
            attacker_vehicle_id: 0,
            // TR Heavy Assault kills a VS Combat Medic
            attacker_loadout_id: 13,
            character_loadout_id: 18,
            is_headshot: true,
            team_id: Some(1),
            attacker_team_id: Some(3),
        }));
        s.end(1_795_200_600);
        s
    }

    /// Stored form (the kill feed is rebuilt after loading).
    fn same(s: &Session) -> serde_json::Value {
        serde_json::to_value(s).unwrap()
    }

    fn names() -> NameCache {
        let mut n = NameCache::default();
        n.items.insert(80, "Gauss SAW".into());
        n.characters.insert(FOE, ("Enemy".into(), 1));
        n
    }

    #[test]
    fn json_roundtrip_with_names() {
        let f = SessionFile::new(&sample(), &names());
        assert_eq!(f.names.items[&80], "Gauss SAW");
        assert_eq!(f.names.characters[&FOE.to_string()], "Enemy");
        let back = from_json(&to_json(&f).unwrap()).unwrap();
        assert_eq!(same(&back.session), same(&f.session));
        back.validate(NOW).unwrap();
        // names travel with the file
        let c = back.name_cache(&NameCache::default());
        assert_eq!(c.characters[&FOE].0, "Enemy");
    }

    #[test]
    fn xlsx_has_sheets_and_imports_again() {
        let f = SessionFile::new(&sample(), &names());
        let bytes = to_xlsx(&f, &names()).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])).unwrap();
        let mut wb = String::new();
        z.by_name("xl/workbook.xml")
            .unwrap()
            .read_to_string(&mut wb)
            .unwrap();
        for sheet in [
            "Summary",
            "Kills",
            "Deaths",
            "Classes",
            "Weapons",
            "Vehicles",
            "Revives &amp; Assists",
            "Achievements",
            "Timeline",
        ] {
            assert!(wb.contains(&format!("name=\"{sheet}\"")), "{sheet} missing");
        }
        assert!(z.by_name("xl/tables/table1.xml").is_ok(), "real tables");
        // class columns and the class sheet carry the names
        let mut strings = String::new();
        z.by_name("xl/sharedStrings.xml")
            .unwrap()
            .read_to_string(&mut strings)
            .unwrap();
        for s in ["Enemy class", "My class", "Combat Medic", "Heavy Assault"] {
            assert!(strings.contains(s), "{s} missing");
        }
        let back = from_xlsx(&bytes).unwrap();
        assert_eq!(same(&back.session), same(&f.session));
    }

    #[test]
    fn rejects_bad_files() {
        let mut f = SessionFile::new(&sample(), &names());
        f.version = FILE_VERSION + 1;
        assert!(f.validate(NOW).is_err());
        let mut f = SessionFile::new(&sample(), &names());
        f.session.started_at = 5;
        assert!(f.validate(NOW).is_err());
        let mut f = SessionFile::new(&sample(), &names());
        f.session.ended_at = Some(f.session.started_at - 1);
        assert!(f.validate(NOW).is_err());
        let mut f = SessionFile::new(&sample(), &names());
        f.format = "other".into();
        assert!(f.validate(NOW).is_err());
        assert!(from_json("{\"session\":{}}").is_err());
        assert!(from_json("[1,2]").is_err());
        // an .xlsx without embedded data
        let plain = Workbook::new().save_to_buffer().unwrap();
        assert!(
            from_xlsx(&plain)
                .unwrap_err()
                .to_string()
                .contains("no embedded session data")
        );
    }

    #[test]
    fn reads_the_0_1_json_export() {
        let s = sample();
        let legacy = serde_json::json!({ "generator": "ps2-tracker 0.1.0", "session": s, "weapon_names": {} });
        let f = from_json(&legacy.to_string()).unwrap();
        assert_eq!(f.session.character_id, ME);
        f.validate(NOW).unwrap();
    }
}
