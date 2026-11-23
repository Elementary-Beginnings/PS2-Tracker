//! Official class icons, loaded at runtime from Census and cached locally.
//!
//! No Daybreak graphic is part of the repository or the packages: only the
//! image IDs are built in (`data/loadouts.json`, `class_images`). On first use
//! the app downloads `https://census.daybreakgames.com/files/ps2/images/static/<id>.png`
//! (no service ID in the path), checks it (real PNG, at most 256×256 pixels and
//! 64 KB) and keeps it in the cache folder (`$XDG_CACHE_HOME/ps2-tracker/class-icons/`,
//! `%LOCALAPPDATA%\ps2-tracker\cache\class-icons\` on Windows). Without network
//! the UI keeps its own CC0 symbols.

use std::path::{Path, PathBuf};

use crate::census::CensusClient;
use crate::classes::{self, Class};

/// Upper limit for one icon file (Census: 64×64 RGBA, a few hundred bytes to ~10 KB).
pub const MAX_BYTES: usize = 64 * 1024;
/// Upper limit for width and height.
pub const MAX_SIDE: u32 = 256;

/// Cache folder for the icons (created on demand).
pub fn cache_dir() -> Option<PathBuf> {
    let base = directories::BaseDirs::new()?;
    #[cfg(windows)]
    let dir = base.data_local_dir().join("ps2-tracker").join("cache");
    #[cfg(not(windows))]
    let dir = base.cache_dir().join("ps2-tracker");
    Some(dir.join("class-icons"))
}

/// Real PNG within the limits; returns (width, height).
pub fn check_png(bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("{} bytes, max. {MAX_BYTES}", bytes.len()));
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("not a PNG".into());
    }
    let r = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| format!("invalid PNG: {e}"))?;
    let i = r.info();
    if i.width == 0 || i.height == 0 || i.width > MAX_SIDE || i.height > MAX_SIDE {
        return Err(format!("{}x{} not allowed", i.width, i.height));
    }
    Ok((i.width, i.height))
}

fn file(dir: &Path, id: u32) -> PathBuf {
    // the name is built from the number only
    dir.join(format!("{id}.png"))
}

/// Cached icon of a class, if present and valid.
pub fn cached(dir: &Path, class: Class) -> Option<Vec<u8>> {
    let id = classes::image_id(class)?;
    let p = file(dir, id);
    let meta = std::fs::symlink_metadata(&p).ok()?;
    if !meta.is_file() || meta.len() > MAX_BYTES as u64 {
        return None;
    }
    let b = std::fs::read(&p).ok()?;
    check_png(&b).ok()?;
    Some(b)
}

/// Classes whose icon is not cached yet.
pub fn missing(dir: &Path) -> Vec<Class> {
    Class::ALL
        .into_iter()
        .filter(|c| classes::image_id(*c).is_some() && cached(dir, *c).is_none())
        .collect()
}

/// Downloads and stores the missing icons; returns how many were stored.
/// Errors are logged per icon (no URL with secrets: the path has none).
pub async fn fetch_missing(census: &CensusClient, dir: &Path) -> usize {
    let mut stored = 0;
    for class in missing(dir) {
        let Some(id) = classes::image_id(class) else {
            continue;
        };
        match census.static_image(id, MAX_BYTES).await {
            Ok(b) => match check_png(&b) {
                Ok(_) => match store(dir, id, &b) {
                    Ok(()) => stored += 1,
                    Err(e) => tracing::warn!(id, error = %e, "class icon not cached"),
                },
                Err(e) => tracing::warn!(id, error = %e, "class icon rejected"),
            },
            Err(e) => tracing::info!(id, error = %e, "class icon not loaded (offline?)"),
        }
    }
    stored
}

fn store(dir: &Path, id: u32, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{id}.png.tmp"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, file(dir, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().unwrap();
        wr.write_image_data(&vec![0u8; (w * h * 4) as usize])
            .unwrap();
        drop(wr);
        out
    }

    #[test]
    fn checks_png() {
        assert_eq!(check_png(&png(64, 64)), Ok((64, 64)));
        assert!(check_png(b"<html>").is_err());
        assert!(check_png(&png(512, 64)).is_err());
        let mut big = png(64, 64);
        big.resize(MAX_BYTES + 1, 0);
        assert!(check_png(&big).is_err());
    }

    #[test]
    fn cache_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ps2t-icons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(missing(&dir).len(), 6);
        store(&dir, 204, &png(64, 64)).unwrap();
        assert!(cached(&dir, Class::Infiltrator).is_some());
        assert_eq!(missing(&dir).len(), 5);
        // a broken file in the cache counts as missing
        std::fs::write(dir.join("62.png"), b"not a png").unwrap();
        assert!(cached(&dir, Class::LightAssault).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
