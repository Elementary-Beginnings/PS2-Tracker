//! Loader for Recursion voice packs (`<Name>.rtst_vpk` + audio folder) as they
//! are on disk. Format derived from a real pack and its read-me only
//! (see `recursion_mapping.rs`, `docs/wiki/Voice-Pack-Format.md`).
//!
//! Security: XML without DTD/entity resolution (only the five predefined entities
//! and character references), size limit, paths only inside the pack folder,
//! no symlinks.

use std::io::Write;
use std::path::{Component, Path, PathBuf};

use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};

use crate::packloader::{PackFormat, VoicePackLoader};
use crate::recursion_mapping::{self, VPK_EXTENSION};
use crate::voicepack::{self, Manifest, PackEntry, PackError, PackMeta, VoicePack, slug};

pub const MAX_VPK_BYTES: u64 = 1024 * 1024;
const BOM: &str = "\u{feff}";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecEntry {
    pub name: String,
    /// `None` = "Default" (no pack sound, built-in default).
    pub path: Option<String>,
    pub image_ingame: bool,
    pub image_streaming: bool,
    pub sound: bool,
    /// Unknown fields are kept for export.
    pub extra: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RecPack {
    pub author: String,
    pub description: String,
    pub entries: Vec<RecEntry>,
    pub extra: Vec<(String, String)>,
}

fn parse_bool(s: &str) -> bool {
    !s.trim().eq_ignore_ascii_case("false")
}

fn invalid(msg: impl Into<String>) -> PackError {
    PackError::Invalid(msg.into())
}

impl RecPack {
    /// Parses the content of a `.rtst_vpk` (BOM and CRLF tolerated).
    pub fn parse(text: &str) -> Result<Self, PackError> {
        let text = text.strip_prefix(BOM).unwrap_or(text);
        if text.len() as u64 > MAX_VPK_BYTES {
            return Err(invalid("definition file too large"));
        }
        let mut r = Reader::from_str(text);
        // no per-event trimming: it would eat the spaces around &amp; ("Two &amp; Two.ogg");
        // the collected text of an element is trimmed once at its end
        r.config_mut().trim_text(false);
        let mut stack: Vec<String> = Vec::new();
        let mut buf = String::new();
        let mut pack = RecPack::default();
        let mut cur: Option<RecEntry> = None;
        let mut seen_root = false;
        loop {
            let ev = r.read_event().map_err(|e| invalid(format!("XML: {e}")))?;
            match ev {
                Event::DocType(_) => return Err(invalid("DTD not allowed in a voice pack")),
                Event::Start(e) => {
                    let name = e.name().as_ref().to_owned();
                    if stack.is_empty() {
                        if name != "AchievementSettings" {
                            return Err(invalid(format!("unexpected root element <{name}>")));
                        }
                        seen_root = true;
                    }
                    if name == "ACHIEVEMENT" {
                        cur = Some(RecEntry {
                            name: String::new(),
                            path: None,
                            image_ingame: true,
                            image_streaming: true,
                            sound: true,
                            extra: vec![],
                        });
                    }
                    stack.push(name);
                    buf.clear();
                    if stack.len() > 16 {
                        return Err(invalid("XML nested too deeply"));
                    }
                }
                Event::Empty(e) => {
                    let name = e.name().as_ref().to_owned();
                    if let Some(c) = cur.as_mut() {
                        match name.as_str() {
                            "PATH" => c.path = None,
                            "NAME" => {}
                            _ => c.extra.push((name, String::new())),
                        }
                    }
                }
                Event::Text(t) => buf.push_str(&t.xml10_content()),
                Event::CData(t) => buf.push_str(&t.into_inner()),
                Event::GeneralRef(g) => {
                    if let Some(ch) = g
                        .resolve_char_ref()
                        .map_err(|e| invalid(format!("XML: {e}")))?
                    {
                        buf.push(ch);
                    } else {
                        let ent = g.xml10_content();
                        buf.push_str(match ent.as_ref() {
                            "amp" => "&",
                            "lt" => "<",
                            "gt" => ">",
                            "quot" => "\"",
                            "apos" => "'",
                            other => {
                                return Err(invalid(format!("entity &{other}; not allowed")));
                            }
                        });
                    }
                }
                Event::End(_) => {
                    let name = stack.pop().unwrap_or_default();
                    let parent = stack.last().map(String::as_str).unwrap_or("");
                    let text = std::mem::take(&mut buf).trim().to_owned();
                    match (parent, name.as_str()) {
                        ("ACHIEVEMENT", field) => {
                            if let Some(c) = cur.as_mut() {
                                match field {
                                    "NAME" => c.name = text,
                                    "PATH" => {
                                        c.path = (!text.is_empty()
                                            && !text.eq_ignore_ascii_case("Default"))
                                        .then_some(text)
                                    }
                                    "IMAGE_INGAME" => c.image_ingame = parse_bool(&text),
                                    "IMAGE_STREAMING" => c.image_streaming = parse_bool(&text),
                                    "SOUND" => c.sound = parse_bool(&text),
                                    other => c.extra.push((other.to_owned(), text)),
                                }
                            }
                        }
                        (_, "ACHIEVEMENT") => {
                            if let Some(c) = cur.take()
                                && !c.name.is_empty()
                            {
                                if pack.entries.len() >= 2000 {
                                    return Err(invalid("too many entries"));
                                }
                                pack.entries.push(c);
                            }
                        }
                        ("ACHIEVEMENTDATA", "AUTHOR") => pack.author = text,
                        ("ACHIEVEMENTDATA", "DESCRIPTION") => pack.description = text,
                        ("ACHIEVEMENTDATA", "DATA") => {}
                        ("ACHIEVEMENTDATA", other) => pack.extra.push((other.to_owned(), text)),
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        if !seen_root {
            return Err(invalid("no <AchievementSettings> found"));
        }
        Ok(pack)
    }

    /// Writes the same structure: UTF-8 with BOM, CRLF.
    pub fn to_xml(&self) -> String {
        fn esc(s: &str) -> String {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        }
        let b = |v: bool| if v { "true" } else { "false" };
        let mut l = vec![
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>".to_owned(),
            "<AchievementSettings>".to_owned(),
            "  <ACHIEVEMENTDATA>".to_owned(),
            format!("    <AUTHOR>{}</AUTHOR>", esc(&self.author)),
            format!("    <DESCRIPTION>{}</DESCRIPTION>", esc(&self.description)),
        ];
        for (k, v) in &self.extra {
            l.push(format!("    <{k}>{}</{k}>", esc(v)));
        }
        l.push("    <DATA>".into());
        for e in &self.entries {
            l.push("      <ACHIEVEMENT>".into());
            l.push(format!("        <NAME>{}</NAME>", esc(&e.name)));
            l.push(format!(
                "        <PATH>{}</PATH>",
                esc(e.path.as_deref().unwrap_or("Default"))
            ));
            l.push(format!(
                "        <IMAGE_INGAME>{}</IMAGE_INGAME>",
                b(e.image_ingame)
            ));
            l.push(format!(
                "        <IMAGE_STREAMING>{}</IMAGE_STREAMING>",
                b(e.image_streaming)
            ));
            l.push(format!("        <SOUND>{}</SOUND>", b(e.sound)));
            for (k, v) in &e.extra {
                l.push(format!("        <{k}>{}</{k}>", esc(v)));
            }
            l.push("      </ACHIEVEMENT>".into());
        }
        l.push("    </DATA>".into());
        l.push("  </ACHIEVEMENTDATA>".into());
        l.push("</AchievementSettings>".into());
        format!("{BOM}{}\r\n", l.join("\r\n"))
    }
}

/// Why a path is not used.
#[derive(Debug, Clone, PartialEq)]
pub enum PathIssue {
    Unsafe,
    Symlink,
    Missing,
}

/// Resolves a Recursion path (`.\Folder\File.ogg`) inside `root`:
/// backslash → slash, case-insensitive, no `..`, no absolute paths, no symlinks.
/// Returns the relative path as it is spelled on disk.
pub fn resolve_path(root: &Path, raw: &str) -> Result<PathBuf, PathIssue> {
    let norm = raw.trim().replace('\\', "/");
    if norm.starts_with('/') || norm.contains(':') {
        return Err(PathIssue::Unsafe);
    }
    let mut rel = PathBuf::new();
    for c in Path::new(&norm).components() {
        match c {
            Component::Normal(x) => rel.push(x),
            Component::CurDir => {}
            _ => return Err(PathIssue::Unsafe),
        }
    }
    if rel.as_os_str().is_empty() {
        return Err(PathIssue::Unsafe);
    }
    let mut cur = root.to_path_buf();
    let mut out = PathBuf::new();
    for part in rel.components() {
        let Component::Normal(name) = part else {
            return Err(PathIssue::Unsafe);
        };
        let exact = cur.join(name);
        let found = if std::fs::symlink_metadata(&exact).is_ok() {
            Some(name.to_os_string())
        } else {
            let want = name.to_string_lossy().to_lowercase();
            std::fs::read_dir(&cur).ok().and_then(|rd| {
                rd.filter_map(Result::ok)
                    .map(|e| e.file_name())
                    .find(|n| n.to_string_lossy().to_lowercase() == want)
            })
        };
        let Some(found) = found else {
            return Err(PathIssue::Missing);
        };
        cur.push(&found);
        out.push(&found);
        let meta = std::fs::symlink_metadata(&cur).map_err(|_| PathIssue::Missing)?;
        if meta.file_type().is_symlink() {
            return Err(PathIssue::Symlink);
        }
    }
    if !cur.is_file() {
        return Err(PathIssue::Missing);
    }
    Ok(out)
}

/// The `.rtst_vpk` in the root folder (exactly one expected; the first if several).
pub fn find_vpk(root: &Path) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case(VPK_EXTENSION))
        })
        .collect();
    v.sort();
    v.into_iter().next()
}

pub fn read_vpk(path: &Path) -> Result<RecPack, PackError> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(invalid("definition file is a symbolic link"));
    }
    match crate::recursion_pak::detect_file(path)? {
        Some(crate::recursion_pak::Variant::Xml) => {}
        Some(v) => {
            return Err(invalid(format!(
                "{} pack: it is imported as a binary pack, not read as XML",
                v.label()
            )));
        }
        None => return Err(invalid("not a Recursion pack")),
    }
    if meta.len() > MAX_VPK_BYTES {
        return Err(invalid(format!(
            "definition file too large ({:.1} MB, limit {} MB for the XML variant)",
            meta.len() as f64 / 1048576.0,
            MAX_VPK_BYTES / 1048576
        )));
    }
    let bytes = std::fs::read(path)?;
    let text =
        String::from_utf8(bytes).map_err(|_| invalid("not a Recursion pack (not UTF-8 XML)"))?;
    RecPack::parse(&text)
}

/// Maps a Recursion pack onto the internal model.
pub fn to_voicepack(root: &Path, vpk: &Path, rec: &RecPack) -> VoicePack {
    let mut warnings = Vec::new();
    let mut events = Vec::new();
    for e in &rec.entries {
        let mut entry = PackEntry::new(&e.name, vec![]);
        entry.overlay = e.image_ingame;
        entry.stream = e.image_streaming;
        if !e.sound {
            entry.silent = true;
        } else if let Some(raw) = &e.path
            && crate::recursion_pak::is_foreign_absolute(raw)
        {
            // absolute path of the pack author: never opened, only its file name
            // is looked up in the pack folder; the path itself is not shown
            let found = crate::recursion_pak::base_name(raw)
                .and_then(|b| crate::recursion_pak::find_by_name(root, b))
                .and_then(|p| p.strip_prefix(root).ok().map(Path::to_path_buf));
            match found {
                Some(rel) => entry.files.push(rel.to_string_lossy().replace('\\', "/")),
                None => warnings.push(format!(
                    "{}: absolute path ignored, using default sound",
                    e.name
                )),
            }
        } else if let Some(raw) = &e.path {
            match resolve_path(root, raw) {
                Ok(rel) => entry.files.push(rel.to_string_lossy().replace('\\', "/")),
                Err(PathIssue::Missing) => warnings.push(format!(
                    "{}: file missing ({raw}), using default sound",
                    e.name
                )),
                Err(PathIssue::Unsafe) => warnings.push(format!(
                    "{}: path outside the pack rejected ({raw}), using default sound",
                    e.name
                )),
                Err(PathIssue::Symlink) => warnings.push(format!(
                    "{}: symbolic link rejected ({raw}), using default sound",
                    e.name
                )),
            }
        }
        if !voicepack::is_known_event(&e.name) {
            warnings.push(format!(
                "{}: unknown key, kept but without a trigger",
                e.name
            ));
        }
        events.push(entry);
    }
    let name = vpk
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Recursion pack".into());
    VoicePack {
        id: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        dir: root.to_owned(),
        manifest: Manifest {
            pack: PackMeta {
                source: String::new(),
                name,
                author: rec.author.clone(),
                version: String::new(),
                license: String::new(),
                description: rec.description.clone(),
            },
            events,
        },
        warnings,
        stats: Default::default(),
    }
}

/// Recursion structure from an internal pack (for export). Existing original
/// entries (including unknown ones and "Default") are kept.
pub fn to_recpack(
    pack: &VoicePack,
    base: Option<&RecPack>,
    folder: &str,
) -> (RecPack, Vec<(String, PathBuf)>) {
    let mut rec = base.cloned().unwrap_or_else(|| RecPack {
        author: pack.manifest.pack.author.clone(),
        description: pack.manifest.pack.description.clone(),
        entries: recursion_mapping::KEYS
            .iter()
            .map(|k| RecEntry {
                name: (*k).into(),
                path: None,
                image_ingame: true,
                image_streaming: true,
                sound: true,
                extra: vec![],
            })
            .collect(),
        extra: vec![],
    });
    let mut copies: Vec<(String, PathBuf)> = Vec::new();
    for e in &pack.manifest.events {
        if !voicepack::is_recursion_style_key(&e.event) {
            continue; // the Recursion format does not know our own events
        }
        let idx = match rec.entries.iter().position(|r| r.name == e.event) {
            Some(i) => i,
            None => {
                rec.entries.push(RecEntry {
                    name: e.event.clone(),
                    path: None,
                    image_ingame: true,
                    image_streaming: true,
                    sound: true,
                    extra: vec![],
                });
                rec.entries.len() - 1
            }
        };
        let r = &mut rec.entries[idx];
        r.image_ingame = e.overlay;
        r.image_streaming = e.stream;
        r.sound = !e.silent;
        // Recursion has exactly one file per event: take the first
        if let Some(f) = e.files.first()
            && let Some(src) = pack.file_path(f)
        {
            let file = Path::new(f)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            r.path = Some(format!(".\\{folder}\\{file}"));
            if !copies.iter().any(|(n, _)| *n == file) {
                copies.push((file, src));
            }
        } else if !e.silent {
            r.path = None;
        }
    }
    (rec, copies)
}

pub struct RecursionLoader;

impl VoicePackLoader for RecursionLoader {
    fn format(&self) -> PackFormat {
        PackFormat::Recursion
    }

    fn detect(&self, root: &Path) -> bool {
        !root.join(voicepack::MANIFEST).exists() && find_vpk(root).is_some()
    }

    fn load(&self, root: &Path) -> Result<VoicePack, PackError> {
        let vpk = find_vpk(root).ok_or_else(|| invalid("no .rtst_vpk found"))?;
        let rec = read_vpk(&vpk)?;
        Ok(to_voicepack(root, &vpk, &rec))
    }

    fn export_zip(&self, pack: &VoicePack, out: &Path) -> Result<(), PackError> {
        use zip::write::SimpleFileOptions;
        let base = find_vpk(&pack.dir).and_then(|p| read_vpk(&p).ok());
        let name = {
            let s = slug(&pack.manifest.pack.name);
            if s.is_empty() {
                "VoicePack".to_owned()
            } else {
                s
            }
        };
        let (rec, copies) = to_recpack(pack, base.as_ref(), &name);
        let mut w = zip::ZipWriter::new(std::fs::File::create(out)?);
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        w.start_file(format!("{name}.{VPK_EXTENSION}"), opts)?;
        w.write_all(rec.to_xml().as_bytes())?;
        for (file, src) in copies {
            w.start_file(format!("{name}/{file}"), opts)?;
            w.write_all(&std::fs::read(src)?)?;
        }
        // background image by the packs' convention: in the audio folder
        if let Some((bg, _, _)) = crate::recursion_pak::background_of(&pack.dir) {
            w.start_file(
                format!("{name}/{}", crate::recursion_pak::BACKGROUND_FILE),
                opts,
            )?;
            w.write_all(&std::fs::read(bg)?)?;
        }
        w.finish()?;
        Ok(())
    }
}

/// Staging when importing a single `.rtst_vpk`: only the definition file and the
/// files it references are copied (not the whole folder).
pub fn stage_vpk(vpk: &Path, dest: &Path) -> Result<(), PackError> {
    let root = vpk.parent().ok_or_else(|| invalid("invalid path"))?;
    let rec = read_vpk(vpk)?;
    std::fs::create_dir_all(dest)?;
    std::fs::copy(vpk, dest.join(vpk.file_name().unwrap_or_default()))?;
    let mut total = 0u64;
    for e in &rec.entries {
        let Some(raw) = &e.path else { continue };
        let Ok(rel) = resolve_path(root, raw) else {
            continue;
        };
        let src = root.join(&rel);
        let meta = std::fs::symlink_metadata(&src)?;
        let len = meta.len();
        total += len;
        if len > voicepack::MAX_FILE_BYTES || total > voicepack::MAX_PACK_BYTES {
            return Err(invalid("pack too large"));
        }
        // the definition names the files; next to a downloaded .rtst_vpk only
        // audio files are copied (no documents or keys from the same folder)
        if !meta.is_file() || !crate::audiofmt::is_audio_file(&src)? {
            tracing::warn!(file = %rel.display(), "not an audio file, not imported");
            continue;
        }
        let to = dest.join(&rel);
        if let Some(d) = to.parent() {
            std::fs::create_dir_all(d)?;
        }
        if !to.exists() {
            std::fs::copy(&src, &to)?;
        }
    }
    // background image by convention (not referenced in the XML)
    if let Some(bg) = crate::recursion_pak::find_loose_background(root) {
        match std::fs::read(&bg)
            .map_err(|e| e.to_string())
            .and_then(|b| crate::recursion_pak::check_background(&b).map(|_| b))
        {
            Ok(b) => std::fs::write(dest.join(crate::recursion_pak::BACKGROUND_FILE), b)?,
            Err(e) => tracing::warn!(file = %bg.display(), "background not imported: {e}"),
        }
    }
    Ok(())
}
