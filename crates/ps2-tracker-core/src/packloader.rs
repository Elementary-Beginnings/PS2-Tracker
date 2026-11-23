//! Interchangeable pack loaders: our open format and the Recursion format.
//! The format is detected automatically on import; internally our own event
//! list ([`crate::voicepack::VOICE_EVENTS`]) is always used.

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use crate::recursion::RecursionLoader;
use crate::voicepack::{self, MANIFEST, PackError, VoicePack};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackFormat {
    /// Our format: `voicepack.toml` + audio files.
    Open,
    /// Recursion Tracker format: `<Name>.rtst_vpk` (XML) + audio folder.
    Recursion,
}

pub trait VoicePackLoader: Send + Sync {
    fn format(&self) -> PackFormat;
    /// Detects the format of an unpacked folder.
    fn detect(&self, root: &Path) -> bool;
    /// Loads the pack as it is on disk and maps its events onto our event list.
    fn load(&self, root: &Path) -> Result<VoicePack, PackError>;
    /// Exports a pack in this format as ZIP.
    fn export_zip(&self, pack: &VoicePack, out: &Path) -> Result<(), PackError>;
}

pub struct OpenLoader;

impl VoicePackLoader for OpenLoader {
    fn format(&self) -> PackFormat {
        PackFormat::Open
    }
    fn detect(&self, root: &Path) -> bool {
        root.join(MANIFEST).is_file()
    }
    fn load(&self, root: &Path) -> Result<VoicePack, PackError> {
        VoicePack::load(root)
    }
    fn export_zip(&self, pack: &VoicePack, out: &Path) -> Result<(), PackError> {
        voicepack::export_zip(pack, out)
    }
}

pub fn loaders() -> [&'static dyn VoicePackLoader; 2] {
    [&OpenLoader, &RecursionLoader]
}

pub fn loader(format: PackFormat) -> &'static dyn VoicePackLoader {
    match format {
        PackFormat::Open => &OpenLoader,
        PackFormat::Recursion => &RecursionLoader,
    }
}

/// Detects the format of a folder.
pub fn detect(root: &Path) -> Option<PackFormat> {
    loaders()
        .into_iter()
        .find(|l| l.detect(root))
        .map(|l| l.format())
}

/// Loads a pack from the library, whatever its format.
pub fn load_any(root: &Path) -> Result<(PackFormat, VoicePack), PackError> {
    let f = detect(root).ok_or_else(|| {
        PackError::Invalid(format!(
            "unknown pack format in {} (neither voicepack.toml nor .rtst_vpk)",
            root.display()
        ))
    })?;
    loader(f).load(root).map(|p| (f, p))
}

/// Import from a folder, ZIP or .rtst_vpk with automatic format detection. Packs
/// are copied into the library unchanged (as delivered).
pub fn import(path: &Path, library: &Path) -> Result<(PackFormat, VoicePack), PackError> {
    std::fs::create_dir_all(library)?;
    // binary .rtst_vpk (directly, or inside a folder/ZIP): converted into a
    // fresh staging folder; the conversion notes are kept for the result
    let mut notes: Vec<String> = Vec::new();
    let mut stats: Option<voicepack::ImportStats> = None;
    let mut converted = false;
    let direct_binary = path.is_file()
        && path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(crate::recursion_mapping::VPK_EXTENSION))
        && !matches!(
            crate::recursion_pak::detect_file(path)?,
            Some(crate::recursion_pak::Variant::Xml)
        );
    let staged = if direct_binary {
        let out = voicepack::new_import_dir(library)?;
        match crate::recursion_pak::import_file(path, &out) {
            Ok(p) => {
                notes = p.warnings;
                stats = Some(p.stats);
                converted = true;
                out
            }
            Err(e) => {
                voicepack::discard_import(&out);
                return Err(e);
            }
        }
    } else {
        let staged = voicepack::stage_import(path, library)?;
        match binary_vpk(&staged) {
            Some(vpk) => {
                let out = voicepack::new_import_dir(library);
                let res = out.and_then(|o| {
                    crate::recursion_pak::import_file(&vpk, &o).map(|p| (o, p.warnings, p.stats))
                });
                voicepack::discard_import(&staged);
                let (o, w, st) = res?;
                notes = w;
                stats = Some(st);
                converted = true;
                o
            }
            None => staged,
        }
    };
    let result = (|| {
        let format = detect(&staged).ok_or_else(|| {
            PackError::Invalid(
                "Format not recognised: neither voicepack.toml nor .rtst_vpk. \
                 Plain sound folders can be added with the mapping assistant."
                    .into(),
            )
        })?;
        let pack = loader(format).load(&staged)?;
        let dst = voicepack::finalize_import(&staged, library, &pack.manifest.pack.name)?;
        loader(format).load(&dst).map(|mut p| {
            p.warnings.extend(notes);
            p.stats = stats.unwrap_or_else(|| voicepack::ImportStats {
                embedded: 0,
                loose: p.manifest.events.iter().map(|e| e.files.len()).sum(),
                background: crate::recursion_pak::background_of(&p.dir).is_some(),
                ..Default::default()
            });
            // a converted binary pack is reported as a Recursion import
            (
                if converted {
                    PackFormat::Recursion
                } else {
                    format
                },
                p,
            )
        })
    })();
    if result.is_err() {
        voicepack::discard_import(&staged);
    }
    result
}

fn binary_vpk(dir: &Path) -> Option<std::path::PathBuf> {
    if dir.join(MANIFEST).is_file() {
        return None;
    }
    let vpk = crate::recursion::find_vpk(dir)?;
    match crate::recursion_pak::detect_file(&vpk) {
        Ok(Some(crate::recursion_pak::Variant::Xml)) | Err(_) => None,
        Ok(_) => Some(vpk),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_format_detected_and_imported() {
        let lib = std::env::temp_dir().join(format!("ps2t-pl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&lib);
        let src = voicepack::install_standard_pack(&lib.join("src")).unwrap();
        assert_eq!(detect(&src), Some(PackFormat::Open));
        let (f, p) = import(&src, &lib.join("library")).unwrap();
        assert_eq!(f, PackFormat::Open);
        assert_eq!(p.id, voicepack::STANDARD_PACK_ID);
        let zip = lib.join("x.zip");
        loader(PackFormat::Open).export_zip(&p, &zip).unwrap();
        let (f2, p2) = import(&zip, &lib.join("library2")).unwrap();
        assert_eq!(f2, PackFormat::Open);
        assert_eq!(p2.manifest, p.manifest);
        let _ = std::fs::remove_dir_all(lib);
    }

    #[test]
    fn unknown_folder_is_rejected() {
        let dir = std::env::temp_dir().join(format!("ps2t-unk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("headshot.ogg"), b"OggS").unwrap();
        assert_eq!(detect(&dir), None);
        let lib = dir.with_extension("lib");
        let err = import(&dir, &lib).unwrap_err().to_string();
        assert!(err.contains("mapping assistant"), "{err}");
        assert_eq!(std::fs::read_dir(&lib).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(lib);
    }
}
