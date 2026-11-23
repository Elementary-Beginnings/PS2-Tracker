//! Real pack layout, synthetic content: `ArnoldVoicePk.rtst_vpk` next to the
//! folder `ArnoldVoicePack/`, file names with spaces, apostrophe and `!`, PATH
//! values `.\ArnoldVoicePack\…`. Also the ZIP the real pack ships as (both under a
//! common top folder). The audio is the self-generated test tone.

use std::io::Write;
use std::path::{Path, PathBuf};

use ps2_tracker_core::audiofmt::{self, AudioKind};
use ps2_tracker_core::packloader::{self, PackFormat};
use ps2_tracker_core::voice::{Trigger, VoiceSelector};

const FILES: [(&str, &str); 3] = [
    ("HEAD_SHOT", "Rubber baby buggy bumpers!.ogg"),
    ("DOUBLE_KILL", "I'llBeBack.ogg"),
    ("REVENGE", "Stop Whining.ogg"),
];

fn tone() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio/tone.ogg")
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ps2t-arnold-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn vpk_xml() -> String {
    let mut x = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<AchievementSettings>\r\n  <ACHIEVEMENTDATA>\r\n    <AUTHOR>Synthetic</AUTHOR>\r\n    <DESCRIPTION>layout test</DESCRIPTION>\r\n    <DATA>\r\n",
    );
    for (key, file) in FILES {
        let file = file.replace('\'', "&apos;");
        x.push_str(&format!(
            "      <ACHIEVEMENT>\r\n        <NAME>{key}</NAME>\r\n        <PATH>.\\ArnoldVoicePack\\{file}</PATH>\r\n        <IMAGE_INGAME>true</IMAGE_INGAME>\r\n        <IMAGE_STREAMING>true</IMAGE_STREAMING>\r\n        <SOUND>true</SOUND>\r\n      </ACHIEVEMENT>\r\n"
        ));
    }
    x.push_str("    </DATA>\r\n  </ACHIEVEMENTDATA>\r\n</AchievementSettings>\r\n");
    x
}

/// Writes `ArnoldVoicePk.rtst_vpk` + `ArnoldVoicePack/*.ogg` into `dir`.
fn write_pack(dir: &Path) {
    std::fs::create_dir_all(dir.join("ArnoldVoicePack")).unwrap();
    std::fs::write(dir.join("ArnoldVoicePk.rtst_vpk"), vpk_xml()).unwrap();
    for (_, f) in FILES {
        std::fs::copy(tone(), dir.join("ArnoldVoicePack").join(f)).unwrap();
    }
}

fn check_imported(p: &ps2_tracker_core::voicepack::VoicePack, lib: &Path) {
    assert!(
        p.dir.starts_with(lib),
        "{} not in the library",
        p.dir.display()
    );
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    let packs = std::slice::from_ref(p);
    for (key, file) in FILES {
        let e = p.manifest.events.iter().find(|e| e.event == key).unwrap();
        assert_eq!(e.files, vec![format!("ArnoldVoicePack/{file}")]);
        // what voice_preview_file resolves
        let path = p.file_path(&e.files[0]).unwrap();
        assert!(path.is_file(), "{}", path.display());
        assert_eq!(audiofmt::sniff_file(&path).unwrap(), AudioKind::OggVorbis);
        // what voice_preview_event / the game events resolve
        let mut t = Trigger::new(key);
        t.count = 2;
        let sel = VoiceSelector::default()
            .select(&t, &Default::default(), packs, 0)
            .unwrap();
        assert_eq!(sel.file.as_deref(), Some(path.as_path()));
    }
}

#[test]
fn import_from_vpk_file_next_to_folder() {
    let src = tmp("src");
    write_pack(&src);
    let lib = tmp("lib");
    let (f, p) = packloader::import(&src.join("ArnoldVoicePk.rtst_vpk"), &lib).unwrap();
    assert_eq!(f, PackFormat::Recursion);
    check_imported(&p, &lib);
    // the copy stands alone
    std::fs::remove_dir_all(&src).unwrap();
    let (_, again) = packloader::load_any(&p.dir).unwrap();
    check_imported(&again, &lib);
    let _ = std::fs::remove_dir_all(lib);
}

#[test]
fn import_from_zip_with_common_top_folder() {
    use zip::write::SimpleFileOptions;
    let d = tmp("zip");
    let zp = d.join("ArnoldVoicePack.zip");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
    let o = SimpleFileOptions::default();
    w.add_directory("ArnoldVoicePack/", o).unwrap();
    w.start_file("ArnoldVoicePack/ArnoldVoicePk.rtst_vpk", o)
        .unwrap();
    w.write_all(vpk_xml().as_bytes()).unwrap();
    w.add_directory("ArnoldVoicePack/ArnoldVoicePack/", o)
        .unwrap();
    for (_, f) in FILES {
        w.start_file(format!("ArnoldVoicePack/ArnoldVoicePack/{f}"), o)
            .unwrap();
        w.write_all(&std::fs::read(tone()).unwrap()).unwrap();
    }
    w.finish().unwrap();
    let lib = d.join("lib");
    let (f, p) = packloader::import(&zp, &lib).unwrap();
    assert_eq!(f, PackFormat::Recursion);
    check_imported(&p, &lib);
    let left: Vec<_> = std::fs::read_dir(&lib)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    assert_eq!(left.len(), 1, "staging folder left behind: {left:?}");
    let _ = std::fs::remove_dir_all(d);
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().unwrap();
        wr.write_image_data(&vec![200u8; (w * h * 4) as usize])
            .unwrap();
    }
    out
}

/// BL2Zer0 layout: audio folder name differs from the pack name, DESCRIPTION
/// with line breaks, unreferenced AchievementBG.png in the audio folder.
fn bl2_xml() -> String {
    let mut x = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<AchievementSettings>\r\n  <ACHIEVEMENTDATA>\r\n    <AUTHOR>Synthetic</AUTHOR>\r\n    <DESCRIPTION>First line\nSecond line</DESCRIPTION>\r\n    <DATA>\r\n",
    );
    for (key, file) in [("HEAD_SHOT", "Hit.ogg"), ("DOUBLE_KILL", "Two & Two.ogg")] {
        x.push_str(&format!("      <ACHIEVEMENT>\r\n        <NAME>{key}</NAME>\r\n        <PATH>.\\BL2_Zer0\\{}</PATH>\r\n        <IMAGE_INGAME>true</IMAGE_INGAME>\r\n        <IMAGE_STREAMING>true</IMAGE_STREAMING>\r\n        <SOUND>true</SOUND>\r\n      </ACHIEVEMENT>\r\n", file.replace('&', "&amp;")));
    }
    x.push_str("    </DATA>\r\n  </ACHIEVEMENTDATA>\r\n</AchievementSettings>\r\n");
    x
}

#[test]
fn bl2zer0_zip_with_background_image() {
    use ps2_tracker_core::recursion_pak;
    use zip::write::SimpleFileOptions;
    let d = tmp("bl2");
    let zp = d.join("BL2Zer0.zip");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
    let o = SimpleFileOptions::default();
    w.start_file("BL2Zer0/BL2Zer0.rtst_vpk", o).unwrap();
    w.write_all(bl2_xml().as_bytes()).unwrap();
    for f in ["Hit.ogg", "Two & Two.ogg"] {
        w.start_file(format!("BL2Zer0/BL2_Zer0/{f}"), o).unwrap();
        w.write_all(&std::fs::read(tone()).unwrap()).unwrap();
    }
    w.start_file("BL2Zer0/BL2_Zer0/AchievementBG.png", o)
        .unwrap();
    w.write_all(&png(525, 140)).unwrap();
    w.finish().unwrap();
    let (_, p) = packloader::import(&zp, &d.join("lib")).unwrap();
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);
    assert_eq!(p.manifest.pack.description, "First line\nSecond line");
    let hs = p
        .manifest
        .events
        .iter()
        .find(|e| e.event == "DOUBLE_KILL")
        .unwrap();
    assert!(p.file_path(&hs.files[0]).unwrap().is_file());
    let (_, bw, bh) = recursion_pak::background_of(&p.dir).expect("background");
    assert_eq!((bw, bh), (525, 140));
    // export in Recursion format carries the image in the audio folder
    let out = d.join("export.zip");
    packloader::loader(PackFormat::Recursion)
        .export_zip(&p, &out)
        .unwrap();
    let ar = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
    let names: Vec<String> = (0..ar.len())
        .filter_map(|i| {
            ar.name_for_index(i)
                .and_then(|r| r.ok())
                .map(|c| c.to_string())
        })
        .collect();
    assert!(
        names.iter().any(|n| n.ends_with("/AchievementBG.png")),
        "{names:?}"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn background_import_from_vpk_and_validation() {
    use ps2_tracker_core::recursion_pak;
    let src = tmp("bgsrc");
    std::fs::create_dir_all(src.join("BL2_Zer0")).unwrap();
    std::fs::write(src.join("BL2Zer0.rtst_vpk"), bl2_xml()).unwrap();
    for f in ["Hit.ogg", "Two & Two.ogg"] {
        std::fs::copy(tone(), src.join("BL2_Zer0").join(f)).unwrap();
    }
    std::fs::write(src.join("BL2_Zer0/achievementbg.PNG"), png(300, 80)).unwrap();
    let lib = tmp("bglib");
    let (_, p) = packloader::import(&src.join("BL2Zer0.rtst_vpk"), &lib).unwrap();
    assert!(
        p.dir.join(recursion_pak::BACKGROUND_FILE).is_file(),
        "copied next to the definition"
    );
    // too large / not a PNG
    assert!(recursion_pak::check_background(&png(4000, 100)).is_err());
    assert!(recursion_pak::check_background(b"GIF89a").is_err());
    let _ = std::fs::remove_dir_all(src);
    let _ = std::fs::remove_dir_all(lib);
}
