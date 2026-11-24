//! Recursion loader against a SYNTHETIC fixture: same structure as a real pack
//! (BOM, CRLF, "Default", SOUND=false, space/apostrophe/exclamation mark in a file
//! name, wrong letter case, a `..\` path), but self-generated tones.

use std::io::Write;
use std::path::{Path, PathBuf};

use ps2_tracker_core::packloader::{self, PackFormat, VoicePackLoader};
use ps2_tracker_core::recursion::{self, PathIssue, RecPack, RecursionLoader};
use ps2_tracker_core::voice::{Trigger, VoiceSelector};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recursion")
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ps2t-rt-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

#[test]
fn parses_bom_crlf_and_entities() {
    let raw = std::fs::read_to_string(fixture().join("Testpack.rtst_vpk")).unwrap();
    assert!(raw.starts_with('\u{feff}') && raw.contains("\r\n"));
    let p = RecPack::parse(&raw).unwrap();
    assert_eq!(p.author, "Synthetic test data");
    assert!(p.description.contains('&'));
    assert_eq!(p.entries.len(), 9);
    let hs = &p.entries[0];
    assert_eq!(hs.name, "HEAD_SHOT");
    assert_eq!(hs.path.as_deref(), Some(".\\Testpack\\Don't Stop!.ogg"));
    assert!(hs.image_ingame && !hs.image_streaming && hs.sound);
    assert_eq!(p.entries[3].path, None, "Default");
    assert!(!p.entries[4].sound, "SOUND=false");
    assert_eq!(p.entries[8].name, "SOME_FUTURE_KEY");
}

#[test]
fn rejects_dtd_and_unknown_entities() {
    let evil = "<?xml version=\"1.0\"?><!DOCTYPE x [<!ENTITY a SYSTEM \"file:///etc/passwd\">]><AchievementSettings/>";
    assert!(RecPack::parse(evil).is_err());
    let ent = "<AchievementSettings><ACHIEVEMENTDATA><AUTHOR>&a;</AUTHOR></ACHIEVEMENTDATA></AchievementSettings>";
    assert!(RecPack::parse(ent).is_err());
    assert!(RecPack::parse("<Other/>").is_err());
}

#[test]
fn path_resolution_rules() {
    let root = fixture();
    assert_eq!(
        recursion::resolve_path(&root, ".\\TESTPACK\\Triple.OGG").unwrap(),
        PathBuf::from("Testpack/triple.ogg")
    );
    assert_eq!(
        recursion::resolve_path(&root, ".\\Testpack\\Don't Stop!.ogg").unwrap(),
        PathBuf::from("Testpack/Don't Stop!.ogg")
    );
    // exists, but lies outside via `..` → rejected
    assert_eq!(
        recursion::resolve_path(&root, ".\\..\\recursion\\evil.ogg"),
        Err(PathIssue::Unsafe)
    );
    assert_eq!(
        recursion::resolve_path(&root, "C:\\x.ogg"),
        Err(PathIssue::Unsafe)
    );
    assert_eq!(
        recursion::resolve_path(&root, "/etc/passwd"),
        Err(PathIssue::Unsafe)
    );
    assert_eq!(
        recursion::resolve_path(&root, ".\\Testpack\\missing.ogg"),
        Err(PathIssue::Missing)
    );
}

#[cfg(unix)]
#[test]
fn symlinks_are_rejected() {
    let d = tmp("sym");
    copy_dir(&fixture(), &d);
    std::os::unix::fs::symlink("/etc/hostname", d.join("Testpack/link.ogg")).unwrap();
    assert_eq!(
        recursion::resolve_path(&d, ".\\Testpack\\link.ogg"),
        Err(PathIssue::Symlink)
    );
    std::os::unix::fs::symlink(d.join("Testpack"), d.join("Linked")).unwrap();
    assert_eq!(
        recursion::resolve_path(&d, ".\\Linked\\double.ogg"),
        Err(PathIssue::Symlink)
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn loads_as_voicepack_with_warnings() {
    let root = fixture();
    assert_eq!(packloader::detect(&root), Some(PackFormat::Recursion));
    let p = RecursionLoader.load(&root).unwrap();
    assert_eq!(p.manifest.pack.name, "Testpack");
    let e = |k: &str| p.manifest.events.iter().find(|e| e.event == k).unwrap();
    assert_eq!(e("HEAD_SHOT").files, vec!["Testpack/Don't Stop!.ogg"]);
    assert!(!e("HEAD_SHOT").stream);
    assert_eq!(e("TRIPLE_KILL").files, vec!["Testpack/triple.ogg"]);
    assert!(e("KILLING_SPREE").files.is_empty() && !e("KILLING_SPREE").silent);
    assert!(e("TEAM_KILLER").silent);
    assert!(e("REVENGE").files.is_empty(), ".. path rejected → Default");
    assert!(e("FIRST_BLOOD").files.is_empty(), "missing file → Default");
    assert_eq!(
        e("PIZZA_DELIVERY").files.len(),
        1,
        "key without trigger is kept"
    );
    assert_eq!(e("SOME_FUTURE_KEY").files.len(), 1, "unknown key is kept");
    let w = p.warnings.join("\n");
    assert!(w.contains("REVENGE") && w.contains("outside"), "{w}");
    assert!(w.contains("FIRST_BLOOD") && w.contains("missing"), "{w}");
}

#[test]
fn selection_uses_imported_files() {
    let p = RecursionLoader.load(&fixture()).unwrap();
    let mut s = VoiceSelector::default();
    let none = Default::default();
    let mut t = Trigger::new("TRIPLE_KILL");
    t.count = 3;
    let sel = s.select(&t, &none, std::slice::from_ref(&p), 0).unwrap();
    assert!(sel.file.unwrap().ends_with("Testpack/triple.ogg"));
    let tk = s
        .select(
            &Trigger::new("TEAM_KILLER"),
            &none,
            std::slice::from_ref(&p),
            0,
        )
        .unwrap();
    assert_eq!(tk.file, None);
}

#[test]
fn import_via_vpk_file_copies_only_referenced() {
    let src = tmp("vpksrc");
    copy_dir(&fixture(), &src);
    std::fs::write(src.join("unrelated.txt"), b"do not copy").unwrap();
    let lib = tmp("vpklib");
    let (f, p) = packloader::import(&src.join("Testpack.rtst_vpk"), &lib).unwrap();
    assert_eq!(f, PackFormat::Recursion);
    assert!(p.dir.join("Testpack.rtst_vpk").is_file());
    assert!(p.dir.join("Testpack/Don't Stop!.ogg").is_file());
    assert!(!p.dir.join("unrelated.txt").exists());
    assert!(
        !p.dir.join("evil.ogg").exists(),
        "rejected path is not copied"
    );
    let _ = std::fs::remove_dir_all(src);
    let _ = std::fs::remove_dir_all(lib);
}

/// The definition names the files next to it; a referenced file that is not
/// audio (a document saved under that name in Downloads) is not copied.
#[test]
fn import_via_vpk_file_copies_only_audio() {
    let src = tmp("vpkaudio");
    copy_dir(&fixture(), &src);
    std::fs::write(src.join("Testpack/pizza.ogg"), b"%PDF-1.7 not a sound").unwrap();
    let lib = tmp("vpkaudiolib");
    let (_, p) = packloader::import(&src.join("Testpack.rtst_vpk"), &lib).unwrap();
    assert!(p.dir.join("Testpack/Don't Stop!.ogg").is_file());
    assert!(
        !p.dir.join("Testpack/pizza.ogg").exists(),
        "non-audio file is not copied"
    );
    let _ = std::fs::remove_dir_all(src);
    let _ = std::fs::remove_dir_all(lib);
}

#[test]
fn import_via_zip_and_rar_rejected() {
    use zip::write::SimpleFileOptions;
    let d = tmp("zip");
    let zp = d.join("pack.zip");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
    let root = fixture();
    w.start_file("Testpack.rtst_vpk", SimpleFileOptions::default())
        .unwrap();
    w.write_all(&std::fs::read(root.join("Testpack.rtst_vpk")).unwrap())
        .unwrap();
    for f in [
        "Don't Stop!.ogg",
        "double.ogg",
        "triple.ogg",
        "tk.ogg",
        "pizza.ogg",
    ] {
        w.start_file(format!("Testpack/{f}"), SimpleFileOptions::default())
            .unwrap();
        w.write_all(&std::fs::read(root.join("Testpack").join(f)).unwrap())
            .unwrap();
    }
    w.finish().unwrap();
    let lib = d.join("lib");
    let (f, p) = packloader::import(&zp, &lib).unwrap();
    assert_eq!(f, PackFormat::Recursion);
    assert_eq!(p.manifest.events.len(), 9);
    std::fs::write(d.join("pack.rar"), b"Rar!").unwrap();
    let err = packloader::import(&d.join("pack.rar"), &lib)
        .unwrap_err()
        .to_string();
    assert!(err.contains("RAR"), "{err}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn export_roundtrip_keeps_structure() {
    let p = RecursionLoader.load(&fixture()).unwrap();
    let d = tmp("exp");
    let zp = d.join("out.zip");
    RecursionLoader.export_zip(&p, &zp).unwrap();
    let lib = d.join("lib");
    let (f, back) = packloader::import(&zp, &lib).unwrap();
    assert_eq!(f, PackFormat::Recursion);
    let vpk = recursion::find_vpk(&back.dir).unwrap();
    let raw = std::fs::read_to_string(&vpk).unwrap();
    assert!(raw.starts_with('\u{feff}'), "BOM");
    assert!(
        raw.contains("\r\n") && !raw.replace("\r\n", "").contains('\n'),
        "CRLF only"
    );
    let rec = RecPack::parse(&raw).unwrap();
    let names: Vec<&str> = rec.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"SOME_FUTURE_KEY"));
    let ks = rec
        .entries
        .iter()
        .find(|e| e.name == "KILLING_SPREE")
        .unwrap();
    assert_eq!(ks.path, None, "Default stays Default");
    let tk = rec
        .entries
        .iter()
        .find(|e| e.name == "TEAM_KILLER")
        .unwrap();
    assert!(!tk.sound);
    let hs = back
        .manifest
        .events
        .iter()
        .find(|e| e.event == "HEAD_SHOT")
        .unwrap();
    assert_eq!(hs.files.len(), 1);
    assert!(!hs.stream);
    let _ = std::fs::remove_dir_all(d);
}
