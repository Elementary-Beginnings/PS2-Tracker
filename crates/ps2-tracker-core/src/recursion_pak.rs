//! Binary variants of the Recursion `.rtst_vpk` (besides the XML one in
//! `recursion.rs`), recognised from the first bytes:
//!
//! - **SharpSerializer** (binary "burst" mode): starts with element id, name
//!   flag and the property name `Root`, e.g. `02 01 04 52 6f 6f 74 01`.
//! - **.NET BinaryFormatter** ([MS-NRBF]): starts with the SerializedStreamHeader
//!   record `00 01 00 00 00 ff ff ff ff 01 00 00 00 00 00 00 00`.
//! - **protobuf** (protobuf-net): repeated field 1 = component with type name
//!   (field 8) and payload (field 10).
//!
//! All three are read into plain data (no type is instantiated) and mapped onto
//! our own pack model: embedded sounds are written as files into the pack folder,
//! the definition becomes a `voicepack.toml`. Several sounds per key are kept;
//! unknown keys are kept as well.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine as _;

use crate::pakdoc::{Doc, Node};
use crate::protowire::{self, MessageExt, Wire};
use crate::voicepack::{self, Manifest, PackEntry, PackError, PackMeta, VoicePack};
use crate::{nrbf, sharpser};

/// Upper limit for a binary `.rtst_vpk` (embedded sounds).
pub const MAX_PAK_BYTES: u64 = 256 * 1024 * 1024;
/// Upper limit for one embedded file.
pub const MAX_EMBEDDED_BYTES: usize = 32 * 1024 * 1024;
/// Name of the background image in a pack folder (convention of the packs).
pub const BACKGROUND_FILE: &str = "AchievementBG.png";
pub const BACKGROUND_MAX_W: u32 = 2048;
pub const BACKGROUND_MAX_H: u32 = 512;
pub const BACKGROUND_MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Xml,
    SharpSerializer,
    BinaryFormatter,
    Protobuf,
}

impl Variant {
    pub fn label(self) -> &'static str {
        match self {
            Self::Xml => "XML",
            Self::SharpSerializer => "SharpSerializer (binary)",
            Self::BinaryFormatter => ".NET BinaryFormatter",
            Self::Protobuf => "protobuf",
        }
    }
}

pub fn detect(head: &[u8]) -> Option<Variant> {
    let h = head.strip_prefix(b"\xef\xbb\xbf").unwrap_or(head);
    let t = h
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map_or(h, |i| &h[i..]);
    if t.starts_with(b"<") {
        return Some(Variant::Xml);
    }
    if nrbf::looks_like_nrbf(head) {
        return Some(Variant::BinaryFormatter);
    }
    if sharpser::looks_like_sharp(head) {
        return Some(Variant::SharpSerializer);
    }
    if head.first() == Some(&0x0a) {
        return Some(Variant::Protobuf);
    }
    None
}

pub fn detect_file(path: &Path) -> std::io::Result<Option<Variant>> {
    use std::io::Read;
    let mut buf = Vec::with_capacity(64);
    std::fs::File::open(path)?.take(64).read_to_end(&mut buf)?;
    Ok(detect(&buf))
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PakSound {
    /// Name of the embedded data (`x.ogg_System.Byte`).
    pub embedded: Option<String>,
    /// Path to a loose file next to the `.rtst_vpk` (`.\Folder\x.ogg`).
    pub loose: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PakEntry {
    pub key: String,
    pub sounds: Vec<PakSound>,
    pub sound: bool,
    pub overlay: bool,
    pub stream: bool,
    /// Every text below the entry in the file, in order. Packs that keep
    /// their sound list in fields we do not know by name still name the
    /// embedded files somewhere in the entry; [`convert`] uses the texts that
    /// match an embedded sound.
    pub refs: Vec<String>,
}

#[derive(Debug, Default)]
pub struct PakContent {
    pub variant: Option<Variant>,
    pub name: String,
    pub author: String,
    pub description: String,
    pub entries: Vec<PakEntry>,
    /// normalised name → data
    pub blobs: HashMap<String, Vec<u8>>,
    /// (file name, PNG data)
    pub images: Vec<(String, Vec<u8>)>,
    /// `backgroundImage`: name of the embedded/loose background image
    pub background_name: Option<String>,
    /// `backgroundImageDisable = true`: the pack wants no background
    pub background_disabled: bool,
    pub warnings: Vec<String>,
}

fn invalid(msg: impl Into<String>) -> PackError {
    PackError::Invalid(msg.into())
}

/// `in the face.ogg_System.Byte` → `in the face.ogg` (lower case, for lookups).
pub fn blob_key(name: &str) -> String {
    let n = name.trim();
    let n = match n.rfind("_System.") {
        Some(i) => &n[..i],
        None => n,
    };
    let n = n.rsplit(['\\', '/']).next().unwrap_or(n);
    n.to_lowercase()
}

/// Strings in the packs are often Base64 (UTF-8). Decodes when that gives
/// printable text, otherwise keeps the string.
pub fn text(s: &str) -> String {
    let t = s.trim();
    let b64 = !t.is_empty()
        && t.len().is_multiple_of(4)
        && t.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'+' || c == b'/' || c == b'=');
    if b64
        && let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(t)
        && let Ok(d) = String::from_utf8(raw)
        && !d.is_empty()
        && !d.chars().any(|c| c.is_control() && !"\r\n\t".contains(c))
    {
        return d;
    }
    t.to_owned()
}

fn is_png(b: &[u8]) -> bool {
    b.starts_with(b"\x89PNG\r\n\x1a\n")
}

/// Checks a background image: real PNG, at most 2048×512 and 4 MB.
pub fn check_background(bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.len() > BACKGROUND_MAX_BYTES {
        return Err(format!(
            "background too large ({} bytes, max. 4 MB)",
            bytes.len()
        ));
    }
    let r = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| format!("background is not a valid PNG: {e}"))?;
    let i = r.info();
    if i.width == 0 || i.height == 0 || i.width > BACKGROUND_MAX_W || i.height > BACKGROUND_MAX_H {
        return Err(format!(
            "background {}x{} not allowed (max. {BACKGROUND_MAX_W}x{BACKGROUND_MAX_H})",
            i.width, i.height
        ));
    }
    Ok((i.width, i.height))
}

// ------------------------------------------------------------------ reading

/// Reads a binary `.rtst_vpk` into the neutral model.
pub fn read(data: &[u8]) -> Result<PakContent, PackError> {
    if data.len() as u64 > MAX_PAK_BYTES {
        return Err(invalid(format!(
            "definition file too large ({:.1} MB, limit {} MB)",
            data.len() as f64 / 1048576.0,
            MAX_PAK_BYTES / 1048576
        )));
    }
    let variant = detect(data).ok_or_else(|| invalid("not a Recursion pack"))?;
    let mut c = match variant {
        Variant::Xml => return Err(invalid("XML pack, not a binary one")),
        Variant::SharpSerializer => {
            let doc =
                sharpser::parse(data, MAX_EMBEDDED_BYTES).map_err(|e| invalid(e.to_string()))?;
            from_doc(&doc)
        }
        Variant::BinaryFormatter => {
            let g = nrbf::parse(
                data,
                nrbf::Limits {
                    max_total: MAX_PAK_BYTES as usize,
                    max_array_bytes: MAX_EMBEDDED_BYTES,
                    ..Default::default()
                },
            )
            .map_err(|e| invalid(e.to_string()))?;
            from_doc(&crate::pakdoc::from_nrbf(&g))
        }
        Variant::Protobuf => from_protobuf(data)?,
    };
    c.variant = Some(variant);
    if c.entries.is_empty() {
        return Err(invalid(format!(
            "{} pack without achievement entries (structure not recognised)",
            variant.label()
        )));
    }
    Ok(c)
}

fn str_field(doc: &Doc, n: &Node, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|k| doc.field(n, k).and_then(|v| doc.str(v)))
        .map(text)
}

fn bool_field(doc: &Doc, n: &Node, name: &str) -> Option<bool> {
    match doc.field(n, name)? {
        Node::Bool(b) => Some(*b),
        Node::Int(i) => Some(*i != 0),
        Node::Str(s) => Some(text(s).eq_ignore_ascii_case("true")),
        _ => None,
    }
}

fn is_achievement(doc: &Doc, n: &Node) -> bool {
    matches!(n, Node::Obj { .. })
        && (doc.field(n, "fileSoundPath").is_some()
            || doc.field(n, "pakSoundPath").is_some()
            || (doc.field(n, "sounds").is_some() && doc.field(n, "soundEnabled").is_some()))
}

/// Mapping for the object based variants (SharpSerializer, BinaryFormatter).
fn from_doc(doc: &Doc) -> PakContent {
    let mut c = PakContent::default();
    doc.walk(|n, map_key| {
        // component information (name/author/description)
        if c.name.is_empty()
            && let Node::Obj { ty, .. } = n
            && (ty.contains("ComponentInformation")
                || (doc.field(n, "author").is_some() && doc.field(n, "description").is_some()))
        {
            c.name = str_field(doc, n, &["Name", "name"]).unwrap_or_default();
            c.author = str_field(doc, n, &["author"]).unwrap_or_default();
            c.description = str_field(doc, n, &["description"]).unwrap_or_default();
            if let Some(img) = doc.field(n, "sampleImage").map(|v| doc.resolve(v))
                && let Node::Bytes(b) = img
                && is_png(b)
            {
                c.images.push(("sampleImage.png".into(), b.clone()));
            }
        }
        if let Some(bg) = str_field(doc, n, &["backgroundImage"]).filter(|b| !is_default(b))
            && c.background_name.is_none()
        {
            c.background_name = Some(bg);
        }
        if bool_field(doc, n, "backgroundImageDisable") == Some(true) {
            c.background_disabled = true;
        }
        if is_achievement(doc, n) {
            let key = str_field(doc, n, &["Key", "achievementKey", "Name", "id"])
                .or_else(|| map_key.and_then(|k| doc.str(k)).map(text));
            let Some(key) = key.filter(|k| voicepack::is_recursion_style_key(k)) else {
                c.warnings
                    .push("achievement entry without a valid key skipped".into());
                return;
            };
            let mut sounds = Vec::new();
            // the list sits on the entry or in a `dynamicSounds` object
            // (`dynamicSounds.sounds`, as in Deadpool v2)
            let list = doc.field(n, "sounds").or_else(|| {
                doc.field(n, "dynamicSounds")
                    .and_then(|d| doc.field(d, "sounds"))
            });
            if let Some(list) = list {
                for s in doc.list(list) {
                    sounds.push(PakSound {
                        embedded: str_field(doc, s, &["pakSoundFile"]).filter(|x| !x.is_empty()),
                        loose: str_field(doc, s, &["soundFile"]).filter(|x| !is_default(x)),
                    });
                }
            }
            // the `sounds` list (dynamicSounds) and the single path both count;
            // `fileSoundPath = default` with a filled list is the usual case
            let embedded = str_field(doc, n, &["pakSoundPath"]).filter(|x| !x.is_empty());
            let loose = str_field(doc, n, &["fileSoundPath"]).filter(|x| !is_default(x));
            let single = PakSound { embedded, loose };
            if (single.embedded.is_some() || single.loose.is_some())
                && !sounds.iter().any(|s| same_sound(s, &single))
            {
                sounds.push(single);
            }
            let mut refs = Vec::new();
            subtree_texts(doc, n, 0, &mut refs);
            c.entries.push(PakEntry {
                key,
                sounds,
                sound: bool_field(doc, n, "soundEnabled").unwrap_or(true),
                overlay: bool_field(doc, n, "imageEnabledInGame").unwrap_or(true),
                stream: bool_field(doc, n, "imageEnabledStreaming").unwrap_or(true),
                refs,
            });
        }
        // embedded data: name → bytes pairs
        let mut add = |name: &str, data: &[u8]| {
            let name = text(name);
            if is_png(data) {
                c.images.push((blob_key(&name), data.to_vec()));
            }
            c.blobs.insert(blob_key(&name), data.to_vec());
        };
        for (k, v) in doc.pairs(n) {
            if let (Some(k), Node::Bytes(b)) = (doc.str(k), v) {
                add(k, b);
            }
        }
        // entry object {fileName, dataType, data} (e.g. Camikaze78 pack)
        if let Some(name) = doc.field(n, "fileName").and_then(|v| doc.str(v))
            && let Some(Node::Bytes(b)) = doc.field(n, "data").map(|v| doc.resolve(v))
        {
            add(name, b);
            return;
        }
        if let Node::Obj { fields, .. } = n {
            let strs: Vec<&str> = fields.iter().filter_map(|(_, v)| doc.str(v)).collect();
            let bytes: Vec<&Vec<u8>> = fields
                .iter()
                .filter_map(|(_, v)| match doc.resolve(v) {
                    Node::Bytes(b) => Some(b),
                    _ => None,
                })
                .collect();
            if strs.len() == 1 && bytes.len() == 1 {
                add(strs[0], bytes[0]);
            }
        }
    });
    c
}

/// All texts below `n` (depth first, in file order, bounded).
fn subtree_texts(doc: &Doc, n: &Node, depth: usize, out: &mut Vec<String>) {
    if depth > 8 || out.len() > 256 {
        return;
    }
    match doc.resolve(n) {
        Node::Str(s) => out.push(text(s)),
        Node::List(v) => v.iter().for_each(|x| subtree_texts(doc, x, depth + 1, out)),
        Node::Map(v) => {
            for (k, x) in v {
                subtree_texts(doc, k, depth + 1, out);
                subtree_texts(doc, x, depth + 1, out);
            }
        }
        Node::Obj { fields, .. } => fields
            .iter()
            .for_each(|(_, x)| subtree_texts(doc, x, depth + 1, out)),
        _ => {}
    }
}

/// Printable length-delimited fields below a protobuf message (bounded).
fn pb_texts(b: &[u8], depth: usize, out: &mut Vec<String>) {
    if depth > 4 || out.len() > 256 {
        return;
    }
    let Ok(m) = protowire::parse(b, 0) else {
        return;
    };
    for (_, w) in &m {
        if let Wire::Len(sub) = w {
            match printable(sub) {
                Some(s) if s.contains('.') => out.push(text(s)),
                _ => pb_texts(sub, depth + 1, out),
            }
        }
    }
}

/// `First Blood 001.ogg` → (`first blood`, `ogg`): the name without a
/// trailing number, for packs with numbered variants of one sound.
fn numbered_stem(key: &str) -> Option<(String, String)> {
    let (stem, ext) = key.rsplit_once('.')?;
    let base = stem.trim_end_matches(|c: char| c.is_ascii_digit());
    if base.len() == stem.len() || base.is_empty() {
        return None;
    }
    let base = base.trim_end_matches([' ', '_', '-', '(']).trim_end();
    (!base.is_empty()).then(|| (base.to_owned(), ext.to_owned()))
}

/// Adds embedded sounds the known fields missed: texts below the entry that
/// name an embedded sound, then numbered variants (`… 002.ogg`) of a sound
/// the entry already has, if no other entry names them. Returns the number
/// of sounds added as numbered variants.
fn complete_entries(c: &PakContent) -> (Vec<Vec<PakSound>>, usize) {
    let audio_blob = |k: &str| {
        c.blobs.get(k).is_some_and(|b| !is_png(b)) && !c.images.iter().any(|(n, _)| n == k)
    };
    let explicit_keys = |s: &PakSound| -> Vec<String> {
        [
            s.embedded.as_deref(),
            s.loose.as_deref().and_then(base_name),
        ]
        .into_iter()
        .flatten()
        .map(blob_key)
        .collect()
    };
    let mut lists: Vec<Vec<PakSound>> = Vec::with_capacity(c.entries.len());
    let mut claimed: std::collections::HashSet<String> = std::collections::HashSet::new();
    for e in &c.entries {
        let mut list = e.sounds.clone();
        let mut have: std::collections::HashSet<String> =
            list.iter().flat_map(explicit_keys).collect();
        for r in &e.refs {
            let k = blob_key(r);
            if audio_blob(&k) && have.insert(k.clone()) {
                list.push(PakSound {
                    embedded: Some(r.clone()),
                    loose: None,
                });
            }
        }
        claimed.extend(have);
        lists.push(list);
    }
    // numbered variants nobody names: to the entry that has a sibling
    let mut pool: Vec<&String> = c
        .blobs
        .keys()
        .filter(|k| audio_blob(k) && !claimed.contains(*k))
        .collect();
    pool.sort();
    let mut added = 0;
    for list in &mut lists {
        let stems: Vec<(String, String)> = list
            .iter()
            .flat_map(explicit_keys)
            .filter(|k| c.blobs.contains_key(k))
            .filter_map(|k| numbered_stem(&k))
            .collect();
        if stems.is_empty() {
            continue;
        }
        pool.retain(|k| {
            let hit = numbered_stem(k).is_some_and(|s| stems.contains(&s));
            if hit {
                list.push(PakSound {
                    embedded: Some((*k).clone()),
                    loose: None,
                });
                added += 1;
            }
            !hit
        });
    }
    (lists, added)
}

fn is_default(s: &str) -> bool {
    s.trim().is_empty() || s.trim().eq_ignore_ascii_case("default")
}

fn printable(b: &[u8]) -> Option<&str> {
    let s = std::str::from_utf8(b).ok()?;
    (!s.is_empty() && !s.chars().any(|c| c.is_control() && !"\r\n\t".contains(c))).then_some(s)
}

/// Mapping for the protobuf variant (field numbers as observed in the packs).
fn from_protobuf(data: &[u8]) -> Result<PakContent, PackError> {
    let top = protowire::parse(data, 0).map_err(|e| invalid(e.to_string()))?;
    let mut c = PakContent::default();
    let mut saw_recursion = false;
    let mut any_sound_flag = false;
    for comp in top.all(1) {
        let Wire::Len(cb) = comp else { continue };
        let Ok(m) = protowire::parse(cb, 0) else {
            continue;
        };
        let ty = m.text(8).unwrap_or("");
        saw_recursion |= ty.contains("RecursionTracker.");
        let payload = m.bytes(10).unwrap_or_default();
        if ty.contains("ComponentInformation") {
            // name/author/description are encrypted in this variant: not read
            continue;
        }
        if ty.contains("GroupManager") {
            collect_pb_entries(payload, 0, &mut c.entries, &mut any_sound_flag);
            continue;
        }
        // embedded file: name in a text field of the component or the payload
        let name = m
            .iter()
            .filter(|(f, _)| *f != 8 && *f != 10)
            .find_map(|(_, w)| match w {
                Wire::Len(b) => printable(b).filter(|s| s.contains('.')),
                _ => None,
            });
        if ty.starts_with("System.Byte") && !payload.is_empty() {
            if let Some(n) = name {
                c.blobs.insert(blob_key(n), payload.to_vec());
            } else if let Ok(p) = protowire::parse(payload, 0)
                && let (Some(n), Some(b)) = (p.text(1), p.bytes(2))
            {
                c.blobs.insert(blob_key(n), b.to_vec());
            }
        } else if ty.contains("Bitmap")
            && let Ok(p) = protowire::parse(payload, 0)
            && let Some(b) = p.bytes(2).filter(|b| is_png(b))
        {
            let n = p.text(1).or(name).unwrap_or("image.png");
            c.images.push((blob_key(n), b.to_vec()));
        }
    }
    if !saw_recursion {
        return Err(invalid("not a Recursion pack"));
    }
    if !any_sound_flag {
        // protobuf-net leaves out false values; a pack without a single
        // soundEnabled field most likely means "all enabled"
        for e in &mut c.entries {
            e.sound = true;
        }
    }
    c.name.clear();
    Ok(c)
}

fn collect_pb_entries(b: &[u8], depth: usize, out: &mut Vec<PakEntry>, any_flag: &mut bool) {
    if depth > 4 {
        return;
    }
    let Ok(m) = protowire::parse(b, 0) else {
        return;
    };
    // an entry: field 1 = key (plain text), field 2 = options message
    if let (Some(key), Some(opts)) = (m.text(1), m.bytes(2))
        && voicepack::is_recursion_style_key(key)
        && let Ok(o) = protowire::parse(opts, 0)
    {
        *any_flag |= o.get(2).is_some();
        let flag = |f: u32| o.varint(f).is_some_and(|v| v != 0);
        let mut sounds: Vec<PakSound> = o
            .all(6)
            .into_iter()
            .filter_map(|w| match w {
                Wire::Len(sb) => protowire::parse(sb, 0).ok(),
                _ => None,
            })
            .map(|s| PakSound {
                loose: s.text(1).map(text).filter(|x| !is_default(x)),
                embedded: s.text(2).map(text).filter(|x| !x.is_empty()),
            })
            .collect();
        if let Some(p) = o.text(1).map(text).filter(|x| !is_default(x)) {
            let single = PakSound {
                embedded: None,
                loose: Some(p),
            };
            if !sounds.iter().any(|s| same_sound(s, &single)) {
                sounds.push(single);
            }
        }
        let mut refs = Vec::new();
        pb_texts(opts, 0, &mut refs);
        out.push(PakEntry {
            key: key.to_owned(),
            sounds,
            sound: flag(2),
            overlay: flag(3),
            stream: flag(4),
            refs,
        });
        return;
    }
    for (_, w) in &m {
        if let Wire::Len(sub) = w {
            collect_pb_entries(sub, depth + 1, out, any_flag);
        }
    }
}

// ------------------------------------------------------------------ import

/// Same sound file (by its file name), so the single path is not added twice.
fn same_sound(a: &PakSound, b: &PakSound) -> bool {
    match (sound_file_name(a), sound_file_name(b)) {
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(&y),
        _ => false,
    }
}

/// Last part of a path written on Windows or Unix.
pub(crate) fn base_name(raw: &str) -> Option<&str> {
    let b = raw.trim().rsplit(['\\', '/']).next()?.trim();
    (!b.is_empty() && b != "." && b != ".." && !b.contains('\0')).then_some(b)
}

/// Absolute path from the pack author's machine (`C:\Users\…`, UNC, `/home/…`).
/// Never opened; only its file name is used.
pub(crate) fn is_foreign_absolute(raw: &str) -> bool {
    let r = raw.trim();
    let b = r.as_bytes();
    r.starts_with('\\')
        || r.starts_with('/')
        || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
}

/// A file with this name in the pack folder or one of its direct subfolders
/// (e.g. `ogg files/`); no symlinks, never outside the folder.
pub(crate) fn find_by_name(root: &Path, name: &str) -> Option<PathBuf> {
    let root_c = root.canonicalize().ok()?;
    let ok = |p: &Path| {
        std::fs::symlink_metadata(p).is_ok_and(|m| m.is_file())
            && p.canonicalize().is_ok_and(|c| c.starts_with(&root_c))
    };
    let matches = |p: &Path| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
    };
    let entries: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    if let Some(p) = entries.iter().find(|p| matches(p) && ok(p)) {
        return Some(p.clone());
    }
    for d in entries
        .iter()
        .filter(|p| std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir()))
    {
        if let Ok(rd) = std::fs::read_dir(d)
            && let Some(p) = rd
                .filter_map(Result::ok)
                .map(|e| e.path())
                .find(|p| matches(p) && ok(p))
        {
            return Some(p);
        }
    }
    None
}

/// File name for a sound: last path part, no `..`, audio extension.
fn sound_file_name(s: &PakSound) -> Option<String> {
    let raw = s.embedded.as_deref().or(s.loose.as_deref())?;
    let n = raw.trim();
    let n = match n.rfind("_System.") {
        Some(i) => &n[..i],
        None => n,
    };
    let base = n.rsplit(['\\', '/']).next()?.trim();
    if base.is_empty() || base == "." || base == ".." || base.contains('\0') {
        return None;
    }
    Some(base.to_owned())
}

/// Converts a binary pack into our format in `dest` (sounds in `dest/sounds/`,
/// `voicepack.toml`, background image). `loose_root` is the folder of the
/// `.rtst_vpk` (for sounds that are not embedded). `fallback_name` names the
/// pack when the file carries no (readable) name.
pub fn convert(
    c: &PakContent,
    loose_root: &Path,
    dest: &Path,
    fallback_name: &str,
) -> Result<VoicePack, PackError> {
    std::fs::create_dir_all(dest.join("sounds"))?;
    let mut warnings = c.warnings.clone();
    let mut written: HashMap<String, String> = HashMap::new(); // source id → rel path
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut total: u64 = 0;
    let mut stats = voicepack::ImportStats::default();
    let mut used_blobs: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut events = Vec::new();
    let (lists, variants_added) = complete_entries(c);
    if variants_added > 0 {
        warnings.push(format!(
            "{variants_added} embedded sounds assigned as numbered variants of a sound of the same event"
        ));
    }
    let mut silent_events = Vec::new();
    for (e, sounds) in c.entries.iter().zip(&lists) {
        let mut files = Vec::new();
        for s in sounds {
            let id = format!("{:?}|{:?}", s.embedded, s.loose);
            if let Some(rel) = written.get(&id) {
                files.push(rel.clone());
                continue;
            }
            // 1. embedded data by `pakSoundFile`, 2. embedded data by the file
            // name of `soundFile`, 3. the loose file (relative path, or by name
            // in the pack folder); absolute paths of the author are never opened
            let mut from_blob = false;
            let mut data = None;
            for key in [
                s.embedded.as_deref(),
                s.loose.as_deref().and_then(base_name),
            ]
            .into_iter()
            .flatten()
            {
                if let Some(d) = c.blobs.get(&blob_key(key)) {
                    used_blobs.insert(blob_key(key));
                    data = Some(d.clone());
                    from_blob = true;
                    break;
                }
            }
            if data.is_none() {
                match s.loose.as_deref() {
                    Some(raw) if is_foreign_absolute(raw) => {
                        match base_name(raw).and_then(|b| find_by_name(loose_root, b)) {
                            Some(p) => data = Some(std::fs::read(p)?),
                            None => warnings.push(format!("{}: absolute path ignored", e.key)),
                        }
                    }
                    Some(raw) => match crate::recursion::resolve_path(loose_root, raw) {
                        Ok(rel) => data = Some(std::fs::read(loose_root.join(rel))?),
                        Err(issue) => {
                            match base_name(raw).and_then(|b| find_by_name(loose_root, b)) {
                                Some(p) => data = Some(std::fs::read(p)?),
                                None => warnings
                                    .push(format!("{}: sound {raw} not usable ({issue:?})", e.key)),
                            }
                        }
                    },
                    None => warnings.push(format!(
                        "{}: embedded sound {} missing",
                        e.key,
                        s.embedded.as_deref().unwrap_or("?")
                    )),
                }
            }
            let Some(data) = data else { continue };
            let Some(mut name) = sound_file_name(s) else {
                warnings.push(format!("{}: unsafe sound name rejected", e.key));
                continue;
            };
            if s.embedded.as_deref().is_some_and(|x| x.contains("..")) {
                warnings.push(format!("{}: unsafe sound name rejected", e.key));
                continue;
            }
            if data.len() > voicepack::MAX_FILE_BYTES as usize {
                warnings.push(format!("{}: {name} too large", e.key));
                continue;
            }
            if from_blob {
                stats.embedded += 1;
            } else {
                stats.loose += 1;
            }
            total += data.len() as u64;
            if total > voicepack::MAX_PACK_BYTES {
                return Err(invalid("pack too large"));
            }
            let lower = name.to_ascii_lowercase();
            if !voicepack::AUDIO_EXTENSIONS
                .iter()
                .any(|x| lower.ends_with(&format!(".{x}")))
            {
                name.push_str(".ogg");
            }
            let mut rel = format!("sounds/{name}");
            let mut i = 2;
            while used.contains(&rel.to_lowercase()) {
                let (stem, ext) = name.rsplit_once('.').unwrap_or((&name, "ogg"));
                rel = format!("sounds/{stem}-{i}.{ext}");
                i += 1;
            }
            if voicepack::safe_relative(&rel).is_none() {
                warnings.push(format!("{}: unsafe sound name rejected", e.key));
                continue;
            }
            std::fs::write(dest.join(&rel), &data)?;
            used.insert(rel.to_lowercase());
            written.insert(id, rel.clone());
            files.push(rel);
        }
        if e.sound && files.is_empty() && !sounds.is_empty() {
            silent_events.push(e.key.clone());
        }
        let mut entry = PackEntry::new(&e.key, files);
        entry.silent = !e.sound;
        entry.overlay = e.overlay;
        entry.stream = e.stream;
        events.push(entry);
    }
    // A key listed more than once (e.g. in two groups) becomes one entry with
    // all files: the selector takes the first entry of a key per pack, so an
    // empty first entry would hide the sounds of a later one.
    let mut merged: Vec<PackEntry> = Vec::with_capacity(events.len());
    for e in events {
        match merged.iter_mut().find(|m| m.event == e.event) {
            Some(m) => {
                for f in e.files {
                    if !m.files.contains(&f) {
                        m.files.push(f);
                    }
                }
                m.silent &= e.silent;
            }
            None => merged.push(e),
        }
    }
    let events = merged;
    silent_events.retain(|k| events.iter().any(|e| &e.event == k && e.files.is_empty()));
    if !silent_events.is_empty() {
        warnings.push(format!(
            "events whose sounds could not be taken over: {}",
            silent_events.join(", ")
        ));
    }
    let mut unassigned: Vec<&String> = c
        .blobs
        .iter()
        .filter(|(k, b)| {
            !used_blobs.contains(*k) && !is_png(b) && !c.images.iter().any(|(n, _)| n == *k)
        })
        .map(|(k, _)| k)
        .collect();
    if !unassigned.is_empty() {
        unassigned.sort();
        tracing::debug!(files = ?unassigned, "embedded sounds without an event");
        warnings.push(format!(
            "{} embedded sounds are not assigned to any event (e.g. {})",
            unassigned.len(),
            unassigned
                .iter()
                .take(3)
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    // background: the image named by `backgroundImage`, else an embedded image
    // or a loose AchievementBG.png / single PNG; none with `backgroundImageDisable`
    let mut bg_saved = c.background_disabled;
    let named = c.background_name.as_deref().map(blob_key);
    let mut candidates: Vec<(String, Vec<u8>)> = c.images.clone();
    if let Some(k) = &named
        && !candidates.iter().any(|(n, _)| n == k)
    {
        // named but not embedded: a loose file next to the definition
        if let Some(b) = loose_root
            .join(k)
            .canonicalize()
            .ok()
            .filter(|p| p.starts_with(loose_root.canonicalize().unwrap_or_default()))
            .and_then(|p| std::fs::read(p).ok())
        {
            candidates.push((k.clone(), b));
        }
    }
    candidates.sort_by_key(|(n, _)| {
        (
            Some(n) != named.as_ref(),
            !(n.contains("bg") || n.contains("background")),
        )
    });
    for (n, b) in candidates
        .iter()
        .filter(|(n, _)| !c.background_disabled && !n.contains("sampleimage"))
    {
        match check_background(b) {
            Ok(_) => {
                std::fs::write(dest.join(BACKGROUND_FILE), b)?;
                bg_saved = true;
                break;
            }
            Err(e) => warnings.push(format!("image {n}: {e}")),
        }
    }
    if c.background_disabled {
        warnings.push("pack disables its background image (backgroundImageDisable)".into());
    }
    if !bg_saved && let Some(p) = find_loose_background(loose_root) {
        match std::fs::read(&p)
            .map_err(|e| e.to_string())
            .and_then(|b| check_background(&b).map(|_| b))
        {
            Ok(b) => std::fs::write(dest.join(BACKGROUND_FILE), b)?,
            Err(e) => warnings.push(format!("{}: {e}", p.display())),
        }
    }
    let name = if c.name.trim().is_empty() {
        fallback_name.to_owned()
    } else {
        c.name.trim().to_owned()
    };
    let pack = VoicePack {
        id: dest
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        dir: dest.to_owned(),
        manifest: Manifest {
            pack: PackMeta {
                source: c
                    .variant
                    .map(|v| format!("recursion: {}", v.label()))
                    .unwrap_or_default(),
                name,
                author: c.author.clone(),
                version: "1.0.0".into(),
                license: "unknown".into(),
                description: c.description.replace('\r', ""),
            },
            events,
        },
        warnings,
        stats: voicepack::ImportStats {
            background: dest.join(BACKGROUND_FILE).is_file(),
            referenced: used_blobs.len(),
            unreferenced: c
                .blobs
                .keys()
                .filter(|k| !used_blobs.contains(*k) && !c.images.iter().any(|(n, _)| n == *k))
                .count(),
            ..stats
        },
    };
    pack.save()?;
    Ok(pack)
}

/// Background image of a pack in the library: `AchievementBG.png` in the pack
/// folder or its audio folder, else the only PNG in the pack folder; only if
/// it passes [`check_background`]. Returns (path, width, height).
pub fn background_of(pack_dir: &Path) -> Option<(PathBuf, u32, u32)> {
    let p = find_loose_background(pack_dir)?;
    let b = std::fs::read(&p).ok()?;
    let (w, h) = check_background(&b).ok()?;
    Some((p, w, h))
}

/// `AchievementBG.png` (any letter case) in the folder or its direct
/// subfolders, else the only PNG in the folder.
pub fn find_loose_background(root: &Path) -> Option<PathBuf> {
    let is_bg = |p: &Path| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(BACKGROUND_FILE))
    };
    let entries: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    if let Some(p) = entries.iter().find(|p| p.is_file() && is_bg(p)) {
        return Some(p.clone());
    }
    for d in entries.iter().filter(|p| p.is_dir()) {
        if let Ok(rd) = std::fs::read_dir(d)
            && let Some(p) = rd
                .filter_map(Result::ok)
                .map(|e| e.path())
                .find(|p| p.is_file() && is_bg(p))
        {
            return Some(p);
        }
    }
    let pngs: Vec<&PathBuf> = entries
        .iter()
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")))
        .collect();
    (pngs.len() == 1).then(|| pngs[0].clone())
}

/// Reads and converts a binary `.rtst_vpk` into `dest`.
pub fn import_file(vpk: &Path, dest: &Path) -> Result<VoicePack, PackError> {
    let meta = std::fs::symlink_metadata(vpk)?;
    if meta.file_type().is_symlink() {
        return Err(invalid("definition file is a symbolic link"));
    }
    if meta.len() > MAX_PAK_BYTES {
        return Err(invalid(format!(
            "definition file too large ({:.1} MB, limit {} MB)",
            meta.len() as f64 / 1048576.0,
            MAX_PAK_BYTES / 1048576
        )));
    }
    let data = std::fs::read(vpk)?;
    let c = read(&data)?;
    tracing::info!(
        file = %vpk.display(),
        variant = c.variant.map_or("?", Variant::label),
        entries = c.entries.len(),
        embedded = c.blobs.len(),
        images = c.images.len(),
        "binary Recursion pack read"
    );
    let fallback = vpk
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Recursion pack".into());
    let root = vpk.parent().unwrap_or(Path::new("."));
    let p = convert(&c, root, dest, &fallback)?;
    tracing::info!(
        referenced = p.stats.referenced,
        extracted = p.stats.embedded,
        unreferenced = p.stats.unreferenced,
        loose = p.stats.loose,
        "embedded sounds: referenced / extracted / unreferenced"
    );
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> Node {
        Node::Str(x.into())
    }
    fn obj(fields: Vec<(&str, Node)>) -> Node {
        Node::Obj {
            ty: "X".into(),
            fields: fields.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        }
    }

    /// Regression (Rick and Morty pack, 269 embedded / 128 taken over): sounds
    /// listed in fields we do not know by name, and numbered variants nobody
    /// names, are taken over instead of being dropped.
    #[test]
    fn sounds_outside_known_fields_are_kept() {
        let ogg = |n: u8| Node::Bytes(vec![b'O', b'g', b'g', b'S', n]);
        let doc = Doc {
            root: obj(vec![
                (
                    "groups",
                    Node::List(vec![
                        // the same key once more, empty, before the real one
                        obj(vec![
                            ("Key", s("PER_KILL")),
                            ("soundEnabled", Node::Bool(true)),
                            ("pakSoundPath", s("")),
                            ("fileSoundPath", s("default")),
                        ]),
                        obj(vec![
                            ("Key", s("FIRST_BLOOD")),
                            ("soundEnabled", Node::Bool(true)),
                            ("pakSoundPath", s("First Blood 001.ogg_System.Byte")),
                            ("fileSoundPath", s("default")),
                        ]),
                        obj(vec![
                            ("Key", s("PER_KILL")),
                            ("soundEnabled", Node::Bool(true)),
                            ("pakSoundPath", s("")),
                            ("fileSoundPath", s("default")),
                            // unknown list of sound objects
                            (
                                "variantList",
                                Node::List(vec![
                                    obj(vec![("clip", s("Per Kill A.ogg_System.Byte"))]),
                                    obj(vec![("clip", s(r"R&M\Per Kill B.ogg"))]),
                                ]),
                            ),
                        ]),
                    ]),
                ),
                (
                    "data",
                    Node::Map(vec![
                        (s("First Blood 001.ogg"), ogg(1)),
                        (s("First Blood 002.ogg"), ogg(2)),
                        (s("First Blood 003.ogg"), ogg(3)),
                        (s("Per Kill A.ogg"), ogg(4)),
                        (s("Per Kill B.ogg"), ogg(5)),
                        (s("Leftover.ogg"), ogg(6)),
                    ]),
                ),
            ]),
            targets: HashMap::new(),
        };
        let c = from_doc(&doc);
        assert_eq!(c.entries.len(), 3);
        assert_eq!(c.blobs.len(), 6);
        let dir = std::env::temp_dir().join(format!("ps2t-pakrefs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = convert(&c, &dir, &dir.join("out"), "R&M").unwrap();
        let files = |k: &str| {
            p.manifest
                .events
                .iter()
                .find(|e| e.event == k)
                .map(|e| e.files.len())
                .unwrap_or(0)
        };
        assert_eq!(files("FIRST_BLOOD"), 3, "{:?}", p.warnings);
        assert_eq!(files("PER_KILL"), 2, "{:?}", p.warnings);
        // one entry per key in the converted pack
        assert_eq!(
            p.manifest
                .events
                .iter()
                .filter(|e| e.event == "PER_KILL")
                .count(),
            1
        );
        assert_eq!(p.stats.embedded, 5);
        assert_eq!(p.stats.unreferenced, 1);
        assert!(p.warnings.iter().any(|w| w.contains("numbered variants")));
        assert!(
            p.warnings
                .iter()
                .any(|w| w.contains("1 embedded sounds are not assigned")
                    && w.contains("leftover.ogg")),
            "{:?}",
            p.warnings
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn numbered_stems() {
        assert_eq!(
            numbered_stem("first blood 001.ogg"),
            Some(("first blood".into(), "ogg".into()))
        );
        assert_eq!(
            numbered_stem("kill_2.mp3"),
            Some(("kill".into(), "mp3".into()))
        );
        assert_eq!(numbered_stem("headshot.ogg"), None);
        assert_eq!(numbered_stem("007.ogg"), None);
    }

    /// `pakSoundFile`/`pakSoundPath` carry the type as suffix
    /// (`see ya.ogg_System.Byte`, images `_System.Drawing.Bitmap`); the embedded
    /// entry is named without it (`fileName`). Lookup: suffix off, then by name.
    #[test]
    fn blob_key_strips_type_suffix() {
        assert_eq!(blob_key("see ya.ogg_System.Byte"), blob_key("see ya.ogg"));
        assert_eq!(
            blob_key("Just The Tip.ogg_System.Byte"),
            blob_key("Just The Tip.ogg")
        );
        assert_eq!(
            blob_key("ArcherAchieve.png_System.Drawing.Bitmap"),
            blob_key("ArcherAchieve.png")
        );
        assert_eq!(blob_key(r"DeadpoolVP\see ya.ogg_System.Byte"), "see ya.ogg");
        assert_eq!(base_name(r"C:\Users\x\Desktop\a b.ogg"), Some("a b.ogg"));
        assert!(is_foreign_absolute(r"C:\Users\x\a.ogg"));
        assert!(is_foreign_absolute(r"\\server\share\a.ogg"));
        assert!(is_foreign_absolute("/home/x/a.ogg"));
        assert!(!is_foreign_absolute(r".\Pack\a.ogg"));
    }
}
