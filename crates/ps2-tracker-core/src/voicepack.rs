//! Open voice pack format: a folder (or ZIP) with `voicepack.toml` and audio files.
//! Format description: `docs/wiki/Voice-Pack-Format.md`.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tones;

pub const MANIFEST: &str = "voicepack.toml";
pub const AUDIO_EXTENSIONS: &[&str] = &["ogg", "mp3", "wav", "flac"];
/// Import limits (protection against zip bombs and mistakes).
pub const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_PACK_BYTES: u64 = 200 * 1024 * 1024;
pub const MAX_FILES: usize = 1000;

/// Event key, display label, default priority (0–100).
pub const VOICE_EVENTS: &[(&str, &str, u8)] = &[
    ("kill", "Kill", 10),
    ("headshot", "Headshot kill", 25),
    ("multikill", "Multikill (count = 2, 3, 4 ...)", 70),
    ("streak", "Kill streak (count = 5, 10, 15 ...)", 60),
    ("streak_ended", "Own streak ended", 45),
    ("death", "Death", 40),
    ("suicide", "Suicide", 45),
    ("teamkill", "Teamkill committed", 55),
    ("teamkilled", "Teamkilled", 50),
    ("revive", "Revive given", 30),
    ("revived", "Revived", 30),
    ("revenge", "Revenge (kill on your last killer)", 65),
    (
        "nemesis",
        "Nemesis (killed repeatedly by the same player)",
        60,
    ),
    ("vehicle_kill", "Vehicle destroyed", 50),
    ("achievement", "Achievement unlocked", 80),
    ("login", "Login", 20),
    ("session_start", "Session start", 20),
];

/// Our own events, Recursion keys and further Recursion-style keys
/// (UPPERCASE_WITH_UNDERSCORES, e.g. from newer packs) are allowed.
pub fn is_known_event(e: &str) -> bool {
    VOICE_EVENTS.iter().any(|(k, _, _)| *k == e) || is_recursion_style_key(e)
}

pub fn is_recursion_style_key(e: &str) -> bool {
    !e.is_empty()
        && e.len() <= 64
        && e.starts_with(|c: char| c.is_ascii_uppercase())
        && e.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Event list for the UI: our own events, then all Recursion keys.
pub fn all_events() -> Vec<(String, String)> {
    VOICE_EVENTS
        .iter()
        .map(|(k, l, _)| ((*k).to_owned(), (*l).to_owned()))
        .chain(
            crate::recursion_mapping::KEYS
                .iter()
                .map(|k| ((*k).to_owned(), crate::recursion_mapping::label(k))),
        )
        .collect()
}

pub fn default_priority(e: &str) -> u8 {
    VOICE_EVENTS
        .iter()
        .find(|(k, _, _)| *k == e)
        .map(|(_, _, p)| *p)
        .unwrap_or(50)
}

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("file: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid voicepack.toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("writing voicepack.toml: {0}")]
    TomlWrite(#[from] toml::ser::Error),
    #[error("ZIP: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PackMeta {
    /// Where the pack came from when converted on import (e.g.
    /// "recursion: protobuf"); empty for packs in our own format.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default = "v1")]
    pub version: String,
    /// SPDX identifier or free text, e.g. "CC0-1.0".
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub description: String,
}

fn v1() -> String {
    "1.0.0".into()
}

/// Optional conditions for an entry. All given ones must match.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Conditions {
    /// For multikill/streak/nemesis: minimum count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headshot: Option<bool>,
    /// Census weapon category, e.g. "Sniper Rifle", "Knife" (case-insensitive).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub weapon_category: Vec<String>,
    /// Vehicle name, e.g. "Sunderer", "Galaxy".
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub vehicle: Vec<String>,
}

impl Conditions {
    pub fn specificity(&self) -> u32 {
        u32::from(self.min_count.is_some())
            + u32::from(self.max_count.is_some())
            + u32::from(self.headshot.is_some())
            + u32::from(!self.weapon_category.is_empty())
            + u32::from(!self.vehicle.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackEntry {
    pub event: String,
    /// Empty = no sound in this pack (fall back to the next pack, e.g. Recursion "Default").
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    #[serde(default)]
    pub cooldown_ms: u64,
    #[serde(default = "one")]
    pub volume: f32,
    #[serde(default, skip_serializing_if = "is_default_cond")]
    pub when: Conditions,
    /// No sound for this event – no fallback either (Recursion SOUND=false).
    #[serde(default, skip_serializing_if = "is_false")]
    pub silent: bool,
    /// Display in the overlay (Recursion IMAGE_INGAME).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub overlay: bool,
    /// Display in the OBS browser source (Recursion IMAGE_STREAMING).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub stream: bool,
}

fn yes() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}
fn is_false(b: &bool) -> bool {
    !*b
}

impl PackEntry {
    pub fn new(event: &str, files: Vec<String>) -> Self {
        Self {
            event: event.into(),
            files,
            priority: None,
            cooldown_ms: 0,
            volume: 1.0,
            when: Conditions::default(),
            silent: false,
            overlay: true,
            stream: true,
        }
    }
}

fn one() -> f32 {
    1.0
}
fn is_default_cond(c: &Conditions) -> bool {
    *c == Conditions::default()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub pack: PackMeta,
    #[serde(rename = "event", default)]
    pub events: Vec<PackEntry>,
}

impl Manifest {
    /// Whether the pack has a sound for every kill (`PER_KILL`).
    pub fn has_per_kill(&self) -> bool {
        self.events
            .iter()
            .any(|e| e.event == "PER_KILL" && !e.files.is_empty() && !e.silent)
    }

    /// Proposal for "Use this pack key for every kill" when the pack has no
    /// `PER_KILL`: the pack key with the most sounds (most variety), ties in
    /// alphabetical order. `None` with `PER_KILL` or without any pack key.
    pub fn suggested_kill_key(&self) -> Option<String> {
        if self.has_per_kill() {
            return None;
        }
        self.events
            .iter()
            .filter(|e| is_recursion_style_key(&e.event) && !e.files.is_empty() && !e.silent)
            .max_by(|a, b| {
                a.files
                    .len()
                    .cmp(&b.files.len())
                    .then_with(|| b.event.cmp(&a.event))
            })
            .map(|e| e.event.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoicePack {
    /// Folder name in the library (= ID).
    pub id: String,
    pub dir: PathBuf,
    pub manifest: Manifest,
    /// Notes from loading (missing files, rejected paths ...).
    #[serde(default)]
    pub warnings: Vec<String>,
    /// What an import found (only set right after an import).
    #[serde(default, skip)]
    pub stats: ImportStats,
}

/// Summary of an import for the import dialog.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportStats {
    /// Sounds taken from data embedded in the definition file.
    pub embedded: usize,
    /// Sounds taken from loose files next to it.
    pub loose: usize,
    /// Distinct embedded entries the achievements refer to (found).
    pub referenced: usize,
    /// Embedded entries no achievement refers to (images not counted).
    pub unreferenced: usize,
    pub background: bool,
}

/// Checks a relative path: no absolute paths, no `..`. A backslash is a
/// separator on every platform (manifests written on Windows), so the result
/// and the checks are the same on Linux and Windows.
pub fn safe_relative(p: &str) -> Option<PathBuf> {
    let norm = p.replace('\\', "/");
    let path = Path::new(&norm);
    if norm.is_empty() || path.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(x) => out.push(x),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// The manifest path as it is spelled on disk, with `/`. Packs made on
/// Windows may differ in case from the files (NTFS ignores it, Linux does
/// not); the exact spelling wins, otherwise a case-insensitive match. No
/// symlinks, nothing outside the pack (see [`crate::recursion::resolve_path`]).
fn on_disk(dir: &Path, f: &str) -> Option<String> {
    let rel = safe_relative(f)?;
    if dir.join(&rel).is_file() {
        return Some(slash(&rel));
    }
    let rel = crate::recursion::resolve_path(dir, f).ok()?;
    dir.join(&rel).is_file().then(|| slash(&rel))
}

fn slash(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn has_audio_ext(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Self, PackError> {
        let m: Manifest = toml::from_str(text)?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<(), PackError> {
        if self.pack.name.trim().is_empty() {
            return Err(PackError::Invalid("pack.name missing".into()));
        }
        for e in &self.events {
            if !is_known_event(&e.event) {
                return Err(PackError::Invalid(format!("unknown event: {}", e.event)));
            }
            for f in &e.files {
                let p = safe_relative(f)
                    .ok_or_else(|| PackError::Invalid(format!("unsafe path: {f}")))?;
                if !has_audio_ext(&p) {
                    return Err(PackError::Invalid(format!("unsupported audio format: {f}")));
                }
            }
            if !(0.0..=2.0).contains(&e.volume) {
                return Err(PackError::Invalid(format!(
                    "{}: volume must be 0-2",
                    e.event
                )));
            }
            if e.priority.is_some_and(|p| p > 100) {
                return Err(PackError::Invalid(format!(
                    "{}: priority must be 0-100",
                    e.event
                )));
            }
        }
        Ok(())
    }

    pub fn to_toml(&self) -> Result<String, PackError> {
        Ok(toml::to_string_pretty(self)?)
    }
}

impl VoicePack {
    pub fn load(dir: &Path) -> Result<Self, PackError> {
        let text = std::fs::read_to_string(dir.join(MANIFEST))?;
        let mut manifest = Manifest::parse(&text)?;
        for e in &mut manifest.events {
            for f in &mut e.files {
                *f = on_disk(dir, f)
                    .ok_or_else(|| PackError::Invalid(format!("file missing: {f}")))?;
            }
        }
        Ok(Self {
            warnings: vec![],
            stats: Default::default(),
            id: dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            dir: dir.to_owned(),
            manifest,
        })
    }

    pub fn save(&self) -> Result<(), PackError> {
        self.manifest.validate()?;
        std::fs::write(self.dir.join(MANIFEST), self.manifest.to_toml()?)?;
        Ok(())
    }

    pub fn file_path(&self, rel: &str) -> Option<PathBuf> {
        safe_relative(rel).map(|p| self.dir.join(p))
    }
}

/// All valid packs of the library; broken ones are reported with a reason.
pub fn list_packs(library: &Path) -> (Vec<VoicePack>, Vec<(String, String)>) {
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    let Ok(rd) = std::fs::read_dir(library) else {
        return (ok, bad);
    };
    let mut dirs: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && !p
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .filter(|p| crate::packloader::detect(p).is_some())
        .collect();
    dirs.sort();
    for d in dirs {
        match crate::packloader::load_any(&d).map(|(_, p)| p) {
            Ok(p) => ok.push(p),
            Err(e) => bad.push((d.display().to_string(), e.to_string())),
        }
    }
    (ok, bad)
}

/// Builds a folder name from a pack name.
pub fn slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .replace('ä', "ae")
        .replace('ö', "oe")
        .replace('ü', "ue")
        .replace('ß', "ss")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s
        .split('-')
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if s.is_empty() { "pack".into() } else { s }
}

fn unique_dir(library: &Path, base: &str) -> PathBuf {
    let mut p = library.join(base);
    let mut i = 2;
    while p.exists() {
        p = library.join(format!("{base}-{i}"));
        i += 1;
    }
    p
}

pub const STANDARD_PACK_ID: &str = "standard-tones";
const STANDARD_PACK_VERSION: &str = "1.1.0";

const STANDARD_RECURSION_TONES: &[(&str, &str)] = &[
    ("HEAD_SHOT", "headshot.wav"),
    ("ROAD_KILL", "vehicle_kill.wav"),
    ("FIRST_BLOOD", "multikill_2.wav"),
    ("KNIFE_FIGHT", "headshot.wav"),
    ("TEAM_KILLER", "teamkill.wav"),
    ("REVENGE", "revenge.wav"),
    ("MUTUAL", "death.wav"),
    ("REPEAT_CUSTOMER", "kill.wav"),
    ("DOMINATION", "streak.wav"),
    ("BEING_FARMED", "nemesis.wav"),
    ("DEATH_STREAK", "death.wav"),
    ("ACE", "achievement.wav"),
    ("KILLING_SPREE", "streak.wav"),
    ("DOMINATING", "streak.wav"),
    ("UNSTOPPABLE", "streak.wav"),
    ("WICKED_SICK", "streak.wav"),
    ("GOD_LIKE", "streak.wav"),
    ("DOUBLE_KILL", "multikill_2.wav"),
    ("TRIPLE_KILL", "multikill_3.wav"),
    ("MULTI_KILL", "multikill_4.wav"),
    ("MEGA_KILL", "multikill_4.wav"),
    ("ULTRA_KILL", "multikill_4.wav"),
    ("MONSTER_KILL", "multikill_4.wav"),
    ("LUDICROUS_KILL", "multikill_4.wav"),
    ("HOLY_SHIT", "multikill_4.wav"),
    ("HOLY_SHIT_2", "multikill_4.wav"),
];

/// Installs the default pack (self-generated tones, CC0) if it is missing.
pub fn install_standard_pack(library: &Path) -> Result<PathBuf, PackError> {
    let dir = library.join(STANDARD_PACK_ID);
    // present and current? (older versions are rewritten)
    if let Ok(p) = VoicePack::load(&dir)
        && p.manifest.pack.version == STANDARD_PACK_VERSION
    {
        return Ok(dir);
    }
    std::fs::create_dir_all(dir.join("sounds"))?;
    let mut events: Vec<PackEntry> = Vec::new();
    for (event, file, notes) in tones::standard_set() {
        std::fs::write(
            dir.join("sounds").join(file),
            tones::wav(&tones::synth(&notes, 0.6)),
        )?;
        let rel = format!("sounds/{file}");
        let when = match file {
            "multikill_2.wav" => Conditions {
                max_count: Some(2),
                ..Default::default()
            },
            "multikill_3.wav" => Conditions {
                min_count: Some(3),
                max_count: Some(3),
                ..Default::default()
            },
            "multikill_4.wav" => Conditions {
                min_count: Some(4),
                ..Default::default()
            },
            _ => Conditions::default(),
        };
        events.push(PackEntry {
            cooldown_ms: if event == "kill" { 150 } else { 0 },
            when,
            ..PackEntry::new(event, vec![rel])
        });
    }
    // Recursion keys with a trigger get matching default tones so "Default"
    // entries of imported packs are not silent.
    for (key, file) in STANDARD_RECURSION_TONES {
        events.push(PackEntry::new(key, vec![format!("sounds/{file}")]));
    }
    let pack = VoicePack {
        warnings: vec![],
        stats: Default::default(),
        id: STANDARD_PACK_ID.into(),
        dir: dir.clone(),
        manifest: Manifest {
            pack: PackMeta {
                source: String::new(),
                name: "Standard tones".into(),
                author: "PS2 Session Tracker".into(),
                version: STANDARD_PACK_VERSION.into(),
                license: "CC0-1.0".into(),
                description: "Self-generated sine tones, no voice lines.".into(),
            },
            events,
        },
    };
    pack.save()?;
    Ok(dir)
}

/// Per-file limit: binary `.rtst_vpk` files carry their sounds.
fn file_limit(p: &Path) -> u64 {
    let vpk = p
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(crate::recursion_mapping::VPK_EXTENSION));
    if vpk {
        crate::recursion_pak::MAX_PAK_BYTES
    } else {
        MAX_FILE_BYTES
    }
}

/// New empty staging folder in the library.
/// Folder in the library that holds removed packs until the undo time is
/// over (dot folders are not listed as packs).
pub const TRASH_DIR: &str = ".trash";

/// Checked folder of pack `id` in the library: a plain directory directly in
/// the library (no symlink, no `..`, no separators); the built-in standard
/// pack cannot be removed.
pub fn removable_pack_dir(library: &Path, id: &str) -> Result<PathBuf, PackError> {
    let bad = |m: &str| PackError::Invalid(m.into());
    if id.is_empty()
        || id.starts_with('.')
        || id.contains(['/', '\\', '\0'])
        || Path::new(id).components().count() != 1
    {
        return Err(bad("invalid pack ID"));
    }
    if id == STANDARD_PACK_ID {
        return Err(bad("the standard tones are built in and cannot be removed"));
    }
    let dir = library.join(id);
    let meta = std::fs::symlink_metadata(&dir)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(bad("pack folder is not a plain directory"));
    }
    let lib = library.canonicalize()?;
    let real = dir.canonicalize()?;
    if real.parent() != Some(lib.as_path()) {
        return Err(bad("pack folder is outside the library"));
    }
    Ok(real)
}

/// Moves pack `id` into the trash folder of the library (same file system,
/// one rename); returns the trash path for [`restore_pack`]. The source of
/// the original import is never touched.
pub fn trash_pack(library: &Path, id: &str) -> Result<PathBuf, PackError> {
    let dir = removable_pack_dir(library, id)?;
    let trash = library.join(TRASH_DIR);
    std::fs::create_dir_all(&trash)?;
    let to = unique_dir(&trash, id);
    std::fs::rename(&dir, &to)?;
    Ok(to)
}

/// Undo: moves a trashed pack back, unless its ID was taken in the meantime.
pub fn restore_pack(library: &Path, trashed: &Path, id: &str) -> Result<(), PackError> {
    let trash = library.join(TRASH_DIR).canonicalize()?;
    let real = trashed.canonicalize()?;
    if real.parent() != Some(trash.as_path()) {
        return Err(PackError::Invalid("not in the trash folder".into()));
    }
    let dst = library.join(id);
    if dst.exists() {
        return Err(PackError::Invalid(format!(
            "a pack named {id} exists again"
        )));
    }
    std::fs::rename(real, dst)?;
    Ok(())
}

/// Deletes one trashed pack for good (`remove_dir_all` removes symlinks inside
/// as links, it does not follow them).
pub fn purge_trashed(library: &Path, trashed: &Path) -> Result<(), PackError> {
    let trash = library.join(TRASH_DIR).canonicalize()?;
    let meta = std::fs::symlink_metadata(trashed)?;
    if meta.file_type().is_symlink()
        || trashed
            .parent()
            .map(Path::canonicalize)
            .transpose()?
            .as_deref()
            != Some(trash.as_path())
    {
        return Err(PackError::Invalid("not in the trash folder".into()));
    }
    std::fs::remove_dir_all(trashed)?;
    Ok(())
}

/// Removes everything left in the trash folder (start-up clean-up).
pub fn purge_trash(library: &Path) -> usize {
    let Ok(rd) = std::fs::read_dir(library.join(TRASH_DIR)) else {
        return 0;
    };
    rd.filter_map(Result::ok)
        .filter(|e| purge_trashed(library, &e.path()).is_ok())
        .count()
}

/// Size of a pack folder: (audio files, bytes); symlinks are not followed.
pub fn pack_size(dir: &Path) -> (usize, u64) {
    let (mut files, mut bytes) = (0, 0);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.filter_map(Result::ok) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                stack.push(e.path());
            } else if ft.is_file() {
                bytes += e.metadata().map_or(0, |m| m.len());
                let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                if AUDIO_EXTENSIONS
                    .iter()
                    .any(|x| n.ends_with(&format!(".{x}")))
                {
                    files += 1;
                }
            }
        }
    }
    (files, bytes)
}

pub fn new_import_dir(library: &Path) -> Result<PathBuf, PackError> {
    let d = unique_dir(library, ".import");
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

fn copy_tree(src: &Path, dst: &Path, budget: &mut (u64, usize)) -> Result<(), PackError> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let ft = e.file_type()?;
        let to = dst.join(e.file_name());
        if ft.is_dir() {
            copy_tree(&e.path(), &to, budget)?;
        } else if ft.is_file() {
            let len = e.metadata()?.len();
            budget.0 += len;
            budget.1 += 1;
            if len > file_limit(&to) || budget.0 > MAX_PACK_BYTES || budget.1 > MAX_FILES {
                return Err(PackError::Invalid("pack too large".into()));
            }
            std::fs::copy(e.path(), to)?;
        }
        // symlinks are deliberately not copied
    }
    Ok(())
}

/// Unpacks or copies an import (folder, ZIP or .rtst_vpk) format-neutrally into a
/// temporary `.import-*` folder of the library and returns the pack root.
/// If the archive contains exactly one folder, that folder is the root.
pub fn stage_import(src: &Path, library: &Path) -> Result<PathBuf, PackError> {
    std::fs::create_dir_all(library)?;
    let tmp = unique_dir(library, ".import");
    std::fs::create_dir_all(&tmp)?;
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let res = if src.is_dir() {
        copy_tree(src, &tmp, &mut (0, 0))
    } else if ext == crate::recursion_mapping::VPK_EXTENSION {
        match crate::recursion_pak::detect_file(src)? {
            Some(crate::recursion_pak::Variant::Xml) => crate::recursion::stage_vpk(src, &tmp),
            Some(_) => crate::recursion_pak::import_file(src, &tmp).map(|_| ()),
            None => Err(PackError::Invalid("not a Recursion pack".into())),
        }
    } else if ext == "rar" {
        Err(PackError::Invalid(
            "RAR is not supported (no Rust library without unRAR licence restrictions). \
             Please unpack it and import the folder or the .rtst_vpk, or use a ZIP."
                .into(),
        ))
    } else {
        extract_zip(src, &tmp)
    };
    if let Err(e) = res {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let entries: Vec<PathBuf> = std::fs::read_dir(&tmp)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    if entries.len() == 1 && entries[0].is_dir() {
        return Ok(entries[0].clone());
    }
    Ok(tmp)
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), PackError> {
    let mut ar = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
    if ar.len() > MAX_FILES {
        return Err(PackError::Invalid("too many files in the ZIP".into()));
    }
    let mut total = 0u64;
    for i in 0..ar.len() {
        let mut f = ar.by_index(i)?;
        let Some(rel) = f.enclosed_name() else {
            return Err(PackError::Invalid("unsafe path in the ZIP".into()));
        };
        if f.is_dir() {
            continue;
        }
        let limit = file_limit(&rel);
        if f.size() > limit {
            return Err(PackError::Invalid(format!("{} too large", rel.display())));
        }
        let out = dest.join(&rel);
        if let Some(d) = out.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut buf = Vec::new();
        // read at most the limit, whatever the header claims (zip bombs)
        (&mut f).take(limit + 1).read_to_end(&mut buf)?;
        total += buf.len() as u64;
        if buf.len() as u64 > limit || total > MAX_PACK_BYTES {
            return Err(PackError::Invalid("pack too large".into()));
        }
        std::fs::write(out, buf)?;
    }
    Ok(())
}

fn import_tmp_root(staged: &Path) -> PathBuf {
    let is_tmp = |p: &Path| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with(".import"))
    };
    if is_tmp(staged) {
        staged.to_owned()
    } else {
        staged
            .parent()
            .filter(|p| is_tmp(p))
            .unwrap_or(staged)
            .to_owned()
    }
}

/// Moves a staged import into the library under a name.
pub fn finalize_import(staged: &Path, library: &Path, name: &str) -> Result<PathBuf, PackError> {
    let dst = unique_dir(library, &slug(name));
    std::fs::rename(staged, &dst)?;
    let tmp = import_tmp_root(staged);
    if tmp != staged && tmp.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    Ok(dst)
}

/// Discards a staged import.
pub fn discard_import(staged: &Path) {
    let _ = std::fs::remove_dir_all(import_tmp_root(staged));
}

/// Imports a pack in our own format (folder or ZIP).
pub fn import_open(src: &Path, library: &Path) -> Result<VoicePack, PackError> {
    let staged = stage_import(src, library)?;
    let res = (|| {
        let manifest = Manifest::parse(&std::fs::read_to_string(staged.join(MANIFEST))?)?;
        let dst = finalize_import(&staged, library, &manifest.pack.name)?;
        VoicePack::load(&dst).inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&dst);
        })
    })();
    if res.is_err() {
        discard_import(&staged);
    }
    res
}

pub fn import_folder(src: &Path, library: &Path) -> Result<VoicePack, PackError> {
    import_open(src, library)
}

pub fn import_zip(zip_path: &Path, library: &Path) -> Result<VoicePack, PackError> {
    import_open(zip_path, library)
}

/// Exports a pack as ZIP (manifest and referenced files only).
pub fn export_zip(pack: &VoicePack, out: &Path) -> Result<(), PackError> {
    use zip::write::SimpleFileOptions;
    let mut w = zip::ZipWriter::new(std::fs::File::create(out)?);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let root = slug(&pack.manifest.pack.name);
    w.start_file(format!("{root}/{MANIFEST}"), opts)?;
    w.write_all(pack.manifest.to_toml()?.as_bytes())?;
    let mut files: Vec<&String> = pack.manifest.events.iter().flat_map(|e| &e.files).collect();
    files.sort();
    files.dedup();
    for f in files {
        let Some(p) = pack.file_path(f) else { continue };
        w.start_file(format!("{root}/{f}"), opts)?;
        w.write_all(&std::fs::read(p)?)?;
    }
    w.finish()?;
    Ok(())
}

// ------------------------------------------------------------ mapping assistant

/// Assistant proposal: file → event (+ optional count).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MappingProposal {
    pub file: String,
    pub event: Option<String>,
    pub count: Option<u32>,
}

/// Guesses the event from the file name. Deliberately simple; the UI allows corrections.
pub fn guess_event(file_name: &str) -> (Option<&'static str>, Option<u32>) {
    let stem = Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let n: String = stem.replace(['-', '_', ' ', '.'], "");
    let number = || -> Option<u32> {
        let digits: String = stem
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(char::is_ascii_digit)
            .collect();
        digits.parse().ok()
    };
    let has = |k: &str| n.contains(k);
    let multi = [
        ("double", 2),
        ("triple", 3),
        ("quad", 4),
        ("penta", 5),
        ("multi", 0),
        ("mega", 5),
        ("ultra", 6),
        ("monster", 7),
    ];
    for (k, c) in multi {
        if has(k) && !has("streak") {
            let c = if c == 0 { number().unwrap_or(2) } else { c };
            return (Some("multikill"), Some(c));
        }
    }
    if has("teamkilled") || has("tkd") || has("killedbyteam") {
        return (Some("teamkilled"), None);
    }
    if has("teamkill") || has("friendlyfire") {
        return (Some("teamkill"), None);
    }
    if has("streakend") || has("streakover") || has("shutdown") || has("streaklost") {
        return (Some("streak_ended"), None);
    }
    if has("streak") || has("spree") || has("rampage") || has("unstoppable") {
        return (Some("streak"), number());
    }
    if has("headshot") || has("hs") && n.len() <= 6 {
        return (Some("headshot"), None);
    }
    if has("revenge") || has("payback") {
        return (Some("revenge"), None);
    }
    if has("nemesis") || has("dominat") {
        return (Some("nemesis"), None);
    }
    if has("suicide") {
        return (Some("suicide"), None);
    }
    if has("revived") || has("gotrevived") {
        return (Some("revived"), None);
    }
    if has("revive") || has("heal") {
        return (Some("revive"), None);
    }
    if has("vehicle") || has("tank") || has("aircraft") || has("destroyed") {
        return (Some("vehicle_kill"), None);
    }
    if has("achievement") || has("unlock") || has("medal") || has("ribbon") {
        return (Some("achievement"), None);
    }
    if has("login") || has("welcome") || has("hello") {
        return (Some("login"), None);
    }
    if has("sessionstart") || has("start") {
        return (Some("session_start"), None);
    }
    if has("death") || has("died") || has("dead") {
        return (Some("death"), None);
    }
    if has("kill") {
        return (Some("kill"), None);
    }
    (None, None)
}

/// Scans a foreign folder (recursively, depth ≤ 3) for audio files and proposes a mapping.
pub fn propose_mapping(folder: &Path) -> Result<Vec<MappingProposal>, PackError> {
    fn walk(
        dir: &Path,
        base: &Path,
        depth: u32,
        out: &mut Vec<MappingProposal>,
    ) -> std::io::Result<()> {
        if depth > 3 || out.len() >= MAX_FILES {
            return Ok(());
        }
        let mut entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            let ft = e.file_type()?;
            if ft.is_dir() {
                walk(&p, base, depth + 1, out)?;
            } else if ft.is_file() && has_audio_ext(&p) {
                let rel = p
                    .strip_prefix(base)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .into_owned();
                let (event, count) = guess_event(&rel);
                out.push(MappingProposal {
                    file: rel,
                    event: event.map(str::to_owned),
                    count,
                });
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(folder, folder, 0, &mut out)?;
    Ok(out)
}

/// Creates a new pack in the library from a (corrected) mapping.
/// Files are copied; the original stays unchanged.
pub fn create_pack_from_mapping(
    source: &Path,
    library: &Path,
    meta: PackMeta,
    mapping: &[MappingProposal],
) -> Result<VoicePack, PackError> {
    let dst = unique_dir(library, &slug(&meta.name));
    std::fs::create_dir_all(dst.join("sounds"))?;
    let result = (|| {
        let mut events: Vec<PackEntry> = Vec::new();
        let mut total = 0u64;
        for (i, m) in mapping.iter().enumerate() {
            let Some(event) = m.event.as_deref() else {
                continue;
            };
            if !is_known_event(event) {
                return Err(PackError::Invalid(format!("unknown event: {event}")));
            }
            let rel = safe_relative(&m.file)
                .ok_or_else(|| PackError::Invalid(format!("unsafe path: {}", m.file)))?;
            let src = source.join(&rel);
            let len = std::fs::metadata(&src)?.len();
            total += len;
            if len > MAX_FILE_BYTES || total > MAX_PACK_BYTES {
                return Err(PackError::Invalid("pack too large".into()));
            }
            let ext = rel
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("ogg")
                .to_ascii_lowercase();
            let name = format!("{i:03}_{event}.{ext}");
            std::fs::copy(&src, dst.join("sounds").join(&name))?;
            let when = Conditions {
                min_count: m.count,
                max_count: m.count.filter(|_| event == "multikill"),
                ..Default::default()
            };
            let file = format!("sounds/{name}");
            if let Some(e) = events
                .iter_mut()
                .find(|e| e.event == event && e.when == when)
            {
                e.files.push(file);
            } else {
                events.push(PackEntry {
                    when,
                    ..PackEntry::new(event, vec![file])
                });
            }
        }
        let pack = VoicePack {
            id: dst
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            dir: dst.clone(),
            manifest: Manifest { pack: meta, events },
            warnings: vec![],
            stats: Default::default(),
        };
        pack.save()?;
        Ok(pack)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&dst);
    }
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn manifest_from_windows_loads_everywhere() {
        let dir = std::env::temp_dir().join(format!("ps2t-winpack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Sounds")).unwrap();
        std::fs::write(dir.join("Sounds").join("Kill.ogg"), b"OggS").unwrap();
        std::fs::write(dir.join("Sounds").join("death.ogg"), b"OggS").unwrap();
        std::fs::write(
            dir.join(MANIFEST),
            "[pack]\nname = \"Win\"\n\n[[event]]\nevent = \"kill\"\nfiles = [\"Sounds\\\\Kill.ogg\"]\n\n\
             [[event]]\nevent = \"death\"\nfiles = [\"sounds/DEATH.OGG\"]\n",
        )
        .unwrap();
        let p = VoicePack::load(&dir).unwrap();
        assert_eq!(
            p.manifest.events[0].files,
            vec!["Sounds/Kill.ogg".to_string()]
        );
        assert_eq!(
            p.manifest.events[1].files,
            vec!["Sounds/death.ogg".to_string()]
        );
        assert!(
            p.file_path(&p.manifest.events[1].files[0])
                .unwrap()
                .is_file()
        );
        // a backslash never leaves the pack
        assert!(safe_relative("..\\x.ogg").is_none());
        assert!(safe_relative("sounds\\..\\..\\x.ogg").is_none());
        assert_eq!(
            safe_relative("a\\b.ogg").unwrap(),
            Path::new("a").join("b.ogg")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn suggested_kill_key_without_per_kill() {
        let f = |n: usize| (0..n).map(|i| format!("sounds/{i}.ogg")).collect();
        let mut m = Manifest {
            pack: PackMeta::default(),
            events: vec![
                PackEntry::new("HEAD_SHOT", f(2)),
                PackEntry::new("FIRST_BLOOD", f(3)),
                PackEntry::new("DOUBLE_KILL", f(3)),
                PackEntry::new("PER_KILL", vec![]),
                PackEntry::new("kill", f(9)),
            ],
        };
        assert!(!m.has_per_kill());
        // most sounds, ties alphabetically; own event IDs do not count
        assert_eq!(m.suggested_kill_key().as_deref(), Some("DOUBLE_KILL"));
        m.events[3].files = f(1);
        assert!(m.has_per_kill());
        assert_eq!(m.suggested_kill_key(), None);
    }

    #[test]
    fn remove_pack_only_touches_its_folder() {
        let lib = std::env::temp_dir().join(format!("ps2t-rm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&lib);
        let outside = lib.with_extension("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("keep.ogg"), b"x").unwrap();
        std::fs::create_dir_all(lib.join("mine/sounds")).unwrap();
        std::fs::write(lib.join("mine/sounds/a.ogg"), b"abc").unwrap();
        std::fs::create_dir_all(lib.join("other")).unwrap();
        #[cfg(unix)]
        {
            // a symlink inside the pack is removed as a link only
            std::os::unix::fs::symlink(&outside, lib.join("mine/link")).unwrap();
            // a pack ID that is a symlink to elsewhere is refused
            std::os::unix::fs::symlink(&outside, lib.join("evil")).unwrap();
            assert!(trash_pack(&lib, "evil").is_err());
        }
        for bad in [
            "",
            "..",
            "../other",
            "a/b",
            ".trash",
            STANDARD_PACK_ID,
            "missing",
        ] {
            assert!(trash_pack(&lib, bad).is_err(), "{bad}");
        }
        assert_eq!(pack_size(&lib.join("mine")), (1, 3));
        let t = trash_pack(&lib, "mine").unwrap();
        assert!(!lib.join("mine").exists());
        assert!(
            list_packs(&lib)
                .1
                .iter()
                .all(|(d, _)| !d.contains(".trash"))
        );
        restore_pack(&lib, &t, "mine").unwrap();
        let t = trash_pack(&lib, "mine").unwrap();
        purge_trashed(&lib, &t).unwrap();
        assert!(!t.exists());
        assert!(lib.join("other").is_dir());
        assert!(
            outside.join("keep.ogg").is_file(),
            "symlink target untouched"
        );
        assert_eq!(purge_trash(&lib), 0);
        let _ = std::fs::remove_dir_all(&lib);
        let _ = std::fs::remove_dir_all(&outside);
    }

    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ps2t-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn manifest_validation() {
        let ok = r#"
[pack]
name = "Test"
license = "CC0-1.0"

[[event]]
event = "multikill"
files = ["a.ogg", "b.ogg"]
priority = 90
cooldown_ms = 500
[event.when]
min_count = 3
"#;
        let m = Manifest::parse(ok).unwrap();
        assert_eq!(m.events[0].when.min_count, Some(3));
        for bad in [
            "[pack]\nname='x'\n[[event]]\nevent='nope'\nfiles=['a.ogg']\n",
            "[pack]\nname='x'\n[[event]]\nevent='kill'\nfiles=['../a.ogg']\n",
            "[pack]\nname='x'\n[[event]]\nevent='kill'\nfiles=['/etc/a.ogg']\n",
            "[pack]\nname='x'\n[[event]]\nevent='kill'\nfiles=['a.exe']\n",
        ] {
            assert!(Manifest::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn standard_pack_zip_roundtrip() {
        let lib = tmp("lib");
        let dir = install_standard_pack(&lib).unwrap();
        let p = VoicePack::load(&dir).unwrap();
        assert!(p.manifest.events.len() >= 17);
        let zip = lib.join("export.zip");
        export_zip(&p, &zip).unwrap();
        let lib2 = tmp("lib2");
        let back = import_zip(&zip, &lib2).unwrap();
        assert_eq!(back.manifest, p.manifest);
        assert_eq!(back.id, STANDARD_PACK_ID);
        let (packs, bad) = list_packs(&lib2);
        assert_eq!(packs.len(), 1);
        assert!(bad.is_empty());
        let _ = std::fs::remove_dir_all(lib);
        let _ = std::fs::remove_dir_all(lib2);
    }

    #[test]
    fn zip_with_traversal_is_rejected() {
        use zip::write::SimpleFileOptions;
        let lib = tmp("zipbad");
        let zp = lib.join("bad.zip");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
        w.start_file("voicepack.toml", SimpleFileOptions::default())
            .unwrap();
        w.write_all(b"[pack]\nname='x'\n").unwrap();
        w.start_file("../evil.ogg", SimpleFileOptions::default())
            .unwrap();
        w.write_all(b"x").unwrap();
        w.finish().unwrap();
        assert!(import_zip(&zp, &lib).is_err());
        assert!(!lib.parent().unwrap().join("evil.ogg").exists());
        let _ = std::fs::remove_dir_all(lib);
    }

    #[test]
    fn heuristics() {
        assert_eq!(guess_event("Double_Kill.ogg"), (Some("multikill"), Some(2)));
        assert_eq!(guess_event("triplekill.mp3"), (Some("multikill"), Some(3)));
        assert_eq!(guess_event("killstreak_10.wav"), (Some("streak"), Some(10)));
        assert_eq!(guess_event("headshot2.ogg"), (Some("headshot"), None));
        assert_eq!(guess_event("revenge.ogg").0, Some("revenge"));
        assert_eq!(guess_event("team_kill.ogg").0, Some("teamkill"));
        assert_eq!(guess_event("got_revived.ogg").0, Some("revived"));
        assert_eq!(guess_event("death_01.ogg").0, Some("death"));
        assert_eq!(guess_event("ambient.ogg"), (None, None));
    }

    #[test]
    fn mapping_creates_pack() {
        let src = tmp("foreign");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        for f in ["double.ogg", "sub/headshot.ogg", "readme.txt", "music.ogg"] {
            std::fs::write(src.join(f), b"OggS").unwrap();
        }
        let mut m = propose_mapping(&src).unwrap();
        assert_eq!(m.len(), 3);
        // user corrects: music.ogg → login
        for x in &mut m {
            if x.file == "music.ogg" {
                x.event = Some("login".into());
            }
        }
        let lib = tmp("maplib");
        let meta = PackMeta {
            source: String::new(),
            name: "My Pack".into(),
            author: "Someone".into(),
            version: "1.0.0".into(),
            license: "private".into(),
            description: String::new(),
        };
        let p = create_pack_from_mapping(&src, &lib, meta, &m).unwrap();
        assert_eq!(p.id, "my-pack");
        assert_eq!(p.manifest.events.len(), 3);
        assert!(src.join("double.ogg").exists());
        let _ = std::fs::remove_dir_all(src);
        let _ = std::fs::remove_dir_all(lib);
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Standard-Töne"), "standard-toene");
        assert_eq!(slug("  "), "pack");
    }
}
