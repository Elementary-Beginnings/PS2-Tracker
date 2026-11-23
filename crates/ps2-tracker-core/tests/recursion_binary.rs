//! Binary `.rtst_vpk` variants (SharpSerializer, BinaryFormatter, protobuf) with
//! SYNTHETIC files: same class/member names and structure as described for the
//! real packs, Base64 strings, self-generated tones and a self-made PNG.
//! Cases in every variant: a key with two sounds, one `default`, one with
//! soundEnabled=false, one embedded name with `..\` (rejected), one sound only
//! as a loose file two folder levels deep in a folder with `&`.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use ps2_tracker_core::audiofmt::{self, AudioKind};
use ps2_tracker_core::nrbf::writer::W as NW;
use ps2_tracker_core::packloader;
use ps2_tracker_core::protowire::writer as pw;
use ps2_tracker_core::recursion_pak::{self, BACKGROUND_FILE, Variant};
use ps2_tracker_core::sharpser::writer::W as SW;
use ps2_tracker_core::voicepack::VoicePack;

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s)
}

fn tone() -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio/tone.ogg"))
        .unwrap()
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().unwrap();
        wr.write_image_data(&vec![128u8; (w * h * 4) as usize])
            .unwrap();
    }
    out
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ps2t-bin-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// (key, sounds as (soundFile, pakSoundFile), fileSoundPath, soundEnabled)
type Ach = (
    &'static str,
    Vec<(&'static str, &'static str)>,
    &'static str,
    bool,
);

fn achievements() -> Vec<Ach> {
    vec![
        (
            "HEAD_SHOT",
            vec![
                (".\\Pack\\in the face.ogg", "in the face.ogg_System.Byte"),
                (".\\Pack\\Boom!.ogg", "Boom!.ogg_System.Byte"),
            ],
            ".\\Pack\\in the face.ogg",
            true,
        ),
        ("KILLING_SPREE", vec![], "default", true),
        (
            "TEAM_KILLER",
            vec![(".\\Pack\\tk.ogg", "tk.ogg_System.Byte")],
            ".\\Pack\\tk.ogg",
            false,
        ),
        (
            "REVENGE",
            vec![("", "..\\evil.ogg_System.Byte")],
            "default",
            true,
        ),
        (
            "PER_KILL",
            vec![(".\\C&C Sounds\\Units\\Sniper Here.ogg", "")],
            ".\\C&C Sounds\\Units\\Sniper Here.ogg",
            true,
        ),
        // a death and a kill chain, also from the loose file
        (
            "DEATH_INSULT",
            vec![(".\\C&C Sounds\\Units\\Sniper Here.ogg", "")],
            ".\\C&C Sounds\\Units\\Sniper Here.ogg",
            true,
        ),
        (
            "DOUBLE_KILL",
            vec![(".\\C&C Sounds\\Units\\Sniper Here.ogg", "")],
            ".\\C&C Sounds\\Units\\Sniper Here.ogg",
            true,
        ),
    ]
}

const EMBEDDED: [&str; 4] = [
    "in the face.ogg_System.Byte",
    "Boom!.ogg_System.Byte",
    "tk.ogg_System.Byte",
    "..\\evil.ogg_System.Byte",
];

/// Loose file next to the .rtst_vpk (two levels, `&` in the folder name).
fn write_loose(dir: &Path) {
    let d = dir.join("C&C Sounds/Units");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("Sniper Here.ogg"), tone()).unwrap();
    std::fs::create_dir_all(dir.join("Unused Units")).unwrap();
    std::fs::write(dir.join("Unused Units/never.ogg"), tone()).unwrap();
}

// ------------------------------------------------------------ SharpSerializer

fn sharp_pak() -> Vec<u8> {
    let mut w = SW::default();
    // one member of every simple type, as SharpSerializer writes them
    let simple: [(&str, &str, Vec<u8>); 18] = [
        ("vBool", "System.Boolean", vec![1]),
        ("vByte", "System.Byte", vec![7]),
        ("vSByte", "System.SByte", vec![0xfe]),
        ("vChar", "System.Char", vec![b'c']),
        ("vInt16", "System.Int16", 3i16.to_le_bytes().to_vec()),
        ("vUInt16", "System.UInt16", 4u16.to_le_bytes().to_vec()),
        ("vInt32", "System.Int32", 5i32.to_le_bytes().to_vec()),
        ("vUInt32", "System.UInt32", 6u32.to_le_bytes().to_vec()),
        ("vInt64", "System.Int64", 7i64.to_le_bytes().to_vec()),
        ("vUInt64", "System.UInt64", 8u64.to_le_bytes().to_vec()),
        ("vSingle", "System.Single", 1.5f32.to_le_bytes().to_vec()),
        ("vDouble", "System.Double", 2.5f64.to_le_bytes().to_vec()),
        ("vDecimal", "System.Decimal", vec![0; 16]),
        ("vDateTime", "System.DateTime", 9i64.to_le_bytes().to_vec()),
        ("vTimeSpan", "System.TimeSpan", 10i64.to_le_bytes().to_vec()),
        ("vGuid", "System.Guid", vec![0x11; 16]),
        ("vType", "System.Type", vec![1, b'T']),
        (
            "vEnum",
            "RecursionTracker.SomeEnum, RTLibrary",
            2i32.to_le_bytes().to_vec(),
        ),
    ];
    w.complex(
        Some("Root"),
        Some("RecursionTracker.PAKFormat, RTLibrary"),
        2 + simple.len() as i32,
    );
    for (n, ty, raw) in &simple {
        w.typed_raw(n, ty, raw);
    }
    w.collection(Some("objects"), None, Some("System.Object, mscorlib"), 2);
    // ComponentInformation
    w.complex(
        None,
        Some("RecursionTracker.ComponentInformation, RTLibrary"),
        5,
    );
    w.str_prop(Some("Name"), &b64("Synthetic PAK"));
    w.str_prop(Some("description"), &b64("Line one\r\nLine two"));
    w.str_prop(Some("author"), &b64("Test"));
    w.str_prop(
        Some("classType"),
        &b64("RecursionTracker.Plugins.PlanetSide2.AchievementOptionsComponents+GroupManager"),
    );
    w.str_prop(Some("assembly"), &b64("RTPluginPS2"));
    // GroupManager
    let achs = achievements();
    w.complex(
        None,
        Some("RecursionTracker.Plugins.PlanetSide2.AchievementOptionsComponents+GroupManager, RTPluginPS2"),
        1,
    );
    w.collection(
        Some("achievementList"),
        None,
        Some("RecursionTracker.Plugins.PlanetSide2.AchievementOptions, RTPluginPS2"),
        achs.len() as i32,
    );
    for (key, sounds, file, enabled) in &achs {
        w.complex(None, None, 8);
        w.str_prop(Some("Key"), &b64(key));
        w.str_prop(Some("fileSoundPath"), &b64(file));
        w.bool_prop("soundEnabled", *enabled);
        w.bool_prop("imageEnabledInGame", true);
        w.bool_prop("imageEnabledStreaming", *key != "HEAD_SHOT");
        w.str_prop(
            Some("pakSoundPath"),
            &b64(sounds.first().map_or("", |s| s.1)),
        );
        w.bool_prop("dynamicSounds", sounds.len() > 1);
        w.collection(
            Some("sounds"),
            None,
            Some("RecursionTracker.Plugins.PlanetSide2.BasicAchievementSound, RTPluginPS2"),
            sounds.len() as i32,
        );
        for (sf, pf) in sounds {
            w.complex(None, None, 3);
            w.str_prop(Some("soundFile"), &b64(sf));
            w.str_prop(Some("pakSoundFile"), &b64(pf));
            // untyped System.Guid as in the Deadpool pack (`06 01 04 guid 00 01` + 16 bytes)
            w.guid_prop("guid", [0x5a; 16]);
        }
    }
    // embedded data
    w.dictionary(
        Some("pakObjects"),
        Some("System.String, mscorlib"),
        Some("System.Object, mscorlib"),
        5,
    );
    for name in EMBEDDED {
        w.str_prop(None, &b64(name));
        w.bytes_prop(None, Some("System.Byte[], mscorlib"), &tone());
    }
    w.str_prop(None, &b64("AchievementBG.png_System.Byte"));
    w.bytes_prop(None, Some("System.Byte[], mscorlib"), &png(300, 60));
    w.buf
}

// ------------------------------------------------------------ BinaryFormatter

fn nrbf_string(w: &mut NW, s: &str) {
    let id = w.id();
    w.string(id, s);
}

fn nrbf_pak() -> Vec<u8> {
    let mut w = NW::new();
    let root = w.id(); // 1
    w.library(1000, "RTLibrary, Version=1.0.0.0");
    w.class(
        root,
        "RecursionTracker.PAKFormat",
        &[("objects", 2, 0)],
        1000,
    );
    // List<object>
    let list = w.id();
    w.class(
        list,
        "System.Collections.Generic.List`1[[System.Object]]",
        &[("_items", 5, 0), ("_size", 0, 8), ("_version", 0, 8)],
        0,
    );
    let arr = w.id();
    // slots: info, group manager, files, one unused (null)
    let slots = 2 + EMBEDDED.len() + 1 + 1;
    w.object_array(arr, slots);
    // ComponentInformation
    let ci = w.id();
    w.class(
        ci,
        "RecursionTracker.ComponentInformation",
        &[("Name", 1, 0), ("description", 1, 0), ("author", 1, 0)],
        1000,
    );
    nrbf_string(&mut w, &b64("Synthetic NRBF"));
    nrbf_string(&mut w, &b64("desc"));
    nrbf_string(&mut w, &b64("Test"));
    // GroupManager
    let gm = w.id();
    w.class(
        gm,
        "RecursionTracker.Plugins.PlanetSide2.AchievementOptionsComponents+GroupManager",
        &[("achievementList", 2, 0)],
        1000,
    );
    let al = w.id();
    let list_meta = list;
    w.class_with_id(al, list_meta);
    let achs = achievements();
    let al_items = w.id();
    w.object_array(al_items, achs.len());
    let mut ach_meta = None;
    let mut sound_meta = None;
    for (key, sounds, file, enabled) in &achs {
        let id = w.id();
        let members: &[(&str, u8, u8)] = &[
            ("Key", 1, 0),
            ("fileSoundPath", 1, 0),
            ("soundEnabled", 0, 1),
            ("imageEnabledInGame", 0, 1),
            ("imageEnabledStreaming", 0, 1),
            ("pakSoundPath", 1, 0),
            ("dynamicSounds", 0, 1),
            ("sounds", 2, 0),
        ];
        match ach_meta {
            None => {
                w.class(
                    id,
                    "RecursionTracker.Plugins.PlanetSide2.AchievementOptions",
                    members,
                    1000,
                );
                ach_meta = Some(id);
            }
            Some(m) => w.class_with_id(id, m),
        }
        nrbf_string(&mut w, &b64(key));
        nrbf_string(&mut w, &b64(file));
        w.u8(u8::from(*enabled));
        w.u8(1);
        w.u8(u8::from(*key != "HEAD_SHOT"));
        nrbf_string(&mut w, &b64(sounds.first().map_or("", |s| s.1)));
        w.u8(u8::from(sounds.len() > 1));
        // sounds: List<BasicAchievementSound>
        let sl = w.id();
        w.class_with_id(sl, list_meta);
        let sa = w.id();
        w.object_array(sa, sounds.len());
        for (sf, pf) in sounds {
            let sid = w.id();
            match sound_meta {
                None => {
                    w.class(
                        sid,
                        "RecursionTracker.Plugins.PlanetSide2.BasicAchievementSound",
                        &[("soundFile", 1, 0), ("pakSoundFile", 1, 0)],
                        1000,
                    );
                    sound_meta = Some(sid);
                }
                Some(m) => w.class_with_id(sid, m),
            }
            nrbf_string(&mut w, &b64(sf));
            nrbf_string(&mut w, &b64(pf));
        }
        w.i32(sounds.len() as i32);
        w.i32(1);
    }
    w.i32(achs.len() as i32);
    w.i32(1);
    // embedded files: {name, data} objects
    let mut file_meta = None;
    let files: Vec<(String, Vec<u8>)> = EMBEDDED
        .iter()
        .map(|n| (b64(n), tone()))
        .chain([(b64("ArnoldBG.png_System.Byte"), png(454, 90))])
        .collect();
    for (name, data) in &files {
        let id = w.id();
        match file_meta {
            None => {
                w.class(
                    id,
                    "RecursionTracker.PAKObject",
                    &[("name", 1, 0), ("data", 7, 2)],
                    1000,
                );
                file_meta = Some(id);
            }
            Some(m) => w.class_with_id(id, m),
        }
        nrbf_string(&mut w, name);
        let bid = w.id();
        w.bytes(bid, data);
    }
    w.null(); // unused list slot
    // remaining members of the outer List: _size, _version
    w.i32(slots as i32 - 1);
    w.i32(1);
    w.end();
    w.buf
}

// ------------------------------------------------------------ protobuf

fn pb_component(ty: &str, extra: &[(u32, &[u8])], payload: &[u8]) -> Vec<u8> {
    let mut c = Vec::new();
    pw::uint(&mut c, 2, 1);
    pw::uint(&mut c, 4, 2);
    pw::bytes(&mut c, 8, ty.as_bytes());
    for (f, b) in extra {
        pw::bytes(&mut c, *f, b);
    }
    pw::bytes(&mut c, 10, payload);
    c
}

fn pb_pak() -> Vec<u8> {
    let mut top = Vec::new();
    // encrypted-looking metadata (not decoded)
    let mut ci = Vec::new();
    pw::bytes(&mut ci, 1, b"q8Jk2mVx0L1sYt9uW3eRpA==");
    pw::bytes(
        &mut top,
        1,
        &pb_component(
            "RecursionTracker.ComponentInformation, RTLibrary, Version=1.0.0.0",
            &[],
            &ci,
        ),
    );
    let mut gm = Vec::new();
    for (key, sounds, file, enabled) in achievements() {
        let mut o = Vec::new();
        pw::bytes(&mut o, 1, file.as_bytes());
        if enabled {
            pw::uint(&mut o, 2, 1);
        }
        pw::uint(&mut o, 3, 1);
        if key != "HEAD_SHOT" {
            pw::uint(&mut o, 4, 1);
        }
        for (sf, pf) in sounds {
            let mut s = Vec::new();
            pw::bytes(
                &mut s,
                1,
                sf.replace(".\\Pack\\", ".\\ArnoldV3\\Pack\\").as_bytes(),
            );
            if !pf.is_empty() {
                pw::bytes(&mut s, 2, pf.as_bytes());
            }
            pw::bytes(&mut o, 6, &s);
        }
        let mut e = Vec::new();
        pw::bytes(&mut e, 1, key.as_bytes());
        pw::bytes(&mut e, 2, &o);
        pw::bytes(&mut gm, 1, &e);
    }
    pw::bytes(
        &mut top,
        1,
        &pb_component(
            "RecursionTracker.AchievementOptionsComponents+GroupManager, RTPluginPS2",
            &[],
            &gm,
        ),
    );
    for name in EMBEDDED {
        pw::bytes(
            &mut top,
            1,
            &pb_component(
                "System.Byte, mscorlib, Version=4.0.0.0",
                &[(6, name.as_bytes())],
                &tone(),
            ),
        );
    }
    let mut bmp = Vec::new();
    pw::bytes(&mut bmp, 1, b"ArnoldBG.png");
    pw::bytes(&mut bmp, 2, &png(454, 90));
    pw::bytes(
        &mut top,
        1,
        &pb_component(
            "System.Drawing.Bitmap, System.Drawing",
            &[(6, b"ArnoldBG.png_System.Drawing.Bitmap")],
            &bmp,
        ),
    );
    top
}

// ------------------------------------------------------------ checks

fn check(p: &VoicePack, expect_name: &str) {
    assert_eq!(p.manifest.pack.name, expect_name);
    let e = |k: &str| {
        p.manifest
            .events
            .iter()
            .find(|e| e.event == k)
            .unwrap_or_else(|| panic!("{k} missing"))
    };
    let hs = e("HEAD_SHOT");
    assert_eq!(hs.files.len(), 2, "{:?}", hs.files);
    assert!(!hs.stream && hs.overlay);
    for f in &hs.files {
        let path = p.file_path(f).unwrap();
        assert_eq!(
            audiofmt::sniff_file(&path).unwrap(),
            AudioKind::OggVorbis,
            "{f}"
        );
    }
    let ks = e("KILLING_SPREE");
    assert!(ks.files.is_empty() && !ks.silent, "default → next pack");
    assert!(e("TEAM_KILLER").silent);
    assert!(e("REVENGE").files.is_empty(), "..\\ rejected");
    assert!(
        p.warnings.iter().any(|w| w.contains("REVENGE")),
        "{:?}",
        p.warnings
    );
    let pk = e("PER_KILL");
    assert_eq!(pk.files.len(), 1, "loose file used: {:?}", p.warnings);
    assert!(p.file_path(&pk.files[0]).unwrap().is_file());
    assert!(
        !p.dir.join("Unused Units").exists(),
        "unreferenced folder must not be copied"
    );
}

fn import_vpk(bytes: &[u8], name: &str, variant: Variant) -> (PathBuf, VoicePack) {
    let d = tmp(name);
    assert_eq!(recursion_pak::detect(bytes), Some(variant));
    let vpk = d.join(format!("{name}.rtst_vpk"));
    std::fs::write(&vpk, bytes).unwrap();
    write_loose(&d);
    let lib = d.join("lib");
    let (_, p) = packloader::import(&vpk, &lib).unwrap();
    assert!(p.dir.starts_with(&lib));
    (d, p)
}

#[test]
fn sharpserializer_variant() {
    let data = sharp_pak();
    assert_eq!(
        &data[..8],
        b"\x02\x01\x04Root\x01",
        "same first bytes as the real packs"
    );
    let (d, p) = import_vpk(&data, "sharp", Variant::SharpSerializer);
    check(&p, "Synthetic PAK");
    assert_eq!(p.manifest.pack.description, "Line one\nLine two");
    assert!(p.dir.join(BACKGROUND_FILE).is_file(), "embedded background");
    let _ = std::fs::remove_dir_all(d);
}

/// Layout like the "Camikaze78 Voice Pack": embedded entries as objects
/// {fileName, dataType (System.RuntimeType with its type name inline, 95
/// characters as in the real file), data}, `backgroundImage` names one of them,
/// `backgroundImageDisable` switches the background off.
fn entry_list_pak(bg_disabled: bool) -> Vec<u8> {
    const RT: &str = "System.RuntimeType, mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089";
    assert_eq!(RT.len(), 0x5f);
    let mut w = SW::default();
    w.complex(
        Some("Root"),
        Some("RecursionTracker.PAKFormat, RTLibrary"),
        5,
    );
    w.str_prop(Some("backgroundImage"), &b64("deko.png"));
    w.bool_prop("backgroundImageDisable", bg_disabled);
    w.complex(Some("componentInfo"), None, 3);
    w.str_prop(Some("name"), &b64("Entry List Pack"));
    w.str_prop(Some("author"), &b64("Test"));
    w.str_prop(Some("description"), &b64("synthetic"));
    w.collection(Some("achievementList"), None, None, 1);
    w.complex(None, None, 4);
    w.str_prop(Some("Key"), &b64("HEAD_SHOT"));
    w.str_prop(Some("fileSoundPath"), &b64(".\\ogg files\\hs.ogg"));
    w.str_prop(Some("pakSoundPath"), &b64("hs.ogg_System.Byte"));
    w.bool_prop("soundEnabled", true);
    let entries: [(&str, Vec<u8>); 3] = [
        ("hs.ogg_System.Byte", tone()),
        ("zz_bg.png_System.Byte", png(300, 60)),
        ("deko.png_System.Byte", png(400, 80)),
    ];
    w.collection(Some("pakObjects"), None, None, entries.len() as i32);
    for (name, data) in &entries {
        w.complex(None, None, 3);
        w.str_prop(Some("fileName"), &b64(name));
        w.header(6, Some("dataType"), Some(RT));
        w.u8(1);
        w.string("System.Byte[]");
        w.bytes_prop(Some("data"), None, data);
    }
    w.buf
}

#[test]
fn entry_list_with_named_background() {
    let (d, p) = import_vpk(
        &entry_list_pak(false),
        "entrylist",
        Variant::SharpSerializer,
    );
    assert_eq!(p.manifest.pack.name, "Entry List Pack");
    let hs = p
        .manifest
        .events
        .iter()
        .find(|e| e.event == "HEAD_SHOT")
        .unwrap();
    assert_eq!(hs.files.len(), 1);
    let bg = std::fs::read(p.dir.join(BACKGROUND_FILE)).unwrap();
    // backgroundImage wins over the name heuristic ("bg")
    assert_eq!(recursion_pak::check_background(&bg), Ok((400, 80)));
    let _ = std::fs::remove_dir_all(d);

    let (d, p) = import_vpk(
        &entry_list_pak(true),
        "entrylist-off",
        Variant::SharpSerializer,
    );
    assert!(
        !p.dir.join(BACKGROUND_FILE).exists(),
        "backgroundImageDisable respected"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn binaryformatter_variant() {
    let (d, p) = import_vpk(&nrbf_pak(), "nrbf", Variant::BinaryFormatter);
    check(&p, "Synthetic NRBF");
    assert!(p.dir.join(BACKGROUND_FILE).is_file());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn protobuf_variant() {
    let (d, p) = import_vpk(&pb_pak(), "ArnoldVoicePkBETA", Variant::Protobuf);
    // metadata is encrypted in this variant: name from the file name
    check(&p, "ArnoldVoicePkBETA");
    assert!(p.manifest.pack.author.is_empty());
    let bg = std::fs::read(p.dir.join(BACKGROUND_FILE)).unwrap();
    assert_eq!(recursion_pak::check_background(&bg), Ok((454, 90)));
    let _ = std::fs::remove_dir_all(d);
}

/// ZIP like "C&C Recursion Voice Pack V6": common top folder with `&`, binary
/// .rtst_vpk over the normal 20 MB per-file limit, loose and unused folders.
#[test]
fn zip_with_large_binary_vpk() {
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    let mut data = sharp_pak();
    // pad with an extra embedded entry past 20 MB (still under 32 MB per entry)
    let big = {
        let mut w = SW::default();
        w.bytes_prop(
            None,
            Some("System.Byte[], mscorlib"),
            &vec![0u8; 21 * 1024 * 1024],
        );
        w.buf
    };
    // splice: raise the root's property count by one and append a property
    assert_eq!(
        data[8..8 + 1 + 37],
        *[&[37u8][..], b"RecursionTracker.PAKFormat, RTLibrary"].concat()
    );
    let count_at = 8 + 1 + 37;
    assert_eq!(data[count_at], 1);
    data[count_at + 1] += 1;
    let mut pad = vec![6u8, 1];
    pad.push(7);
    pad.extend_from_slice(b"padding");
    pad.extend_from_slice(&big[2..]);
    data.extend_from_slice(&pad);
    assert!(data.len() > 21 * 1024 * 1024);
    let d = tmp("zip");
    let zp = d.join("pack.zip");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
    let o = SimpleFileOptions::default();
    let top = "C&C Recursion Voice Pack V6";
    w.start_file(format!("{top}/CnC Recursion Voice Pack V6.rtst_vpk"), o)
        .unwrap();
    w.write_all(&data).unwrap();
    w.start_file(format!("{top}/C&C Sounds/Units/Sniper Here.ogg"), o)
        .unwrap();
    w.write_all(&tone()).unwrap();
    w.start_file(format!("{top}/Unused Units/never.ogg"), o)
        .unwrap();
    w.write_all(&tone()).unwrap();
    w.finish().unwrap();
    let lib = d.join("lib");
    let (_, p) = packloader::import(&zp, &lib).unwrap();
    check(&p, "Synthetic PAK");
    assert!(
        !p.dir.join("CnC Recursion Voice Pack V6.rtst_vpk").exists(),
        "converted, not copied"
    );
    assert_eq!(
        std::fs::read_dir(&lib).unwrap().count(),
        1,
        "staging left behind"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn error_messages_distinguish_cases() {
    let err = recursion_pak::read(b"GIF89a....").unwrap_err().to_string();
    assert!(err.contains("not a Recursion pack"), "{err}");
    let mut broken = sharp_pak();
    broken.truncate(200);
    let err = recursion_pak::read(&broken).unwrap_err().to_string();
    assert!(err.contains("corrupt PAK record at offset"), "{err}");
    let d = tmp("big");
    let f = d.join("big.rtst_vpk");
    let file = std::fs::File::create(&f).unwrap();
    file.set_len(recursion_pak::MAX_PAK_BYTES + 1).unwrap();
    std::fs::write(d.join("x"), b"").unwrap();
    let err = recursion_pak::import_file(&f, &d.join("out"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("too large") && err.contains("limit"), "{err}");
    let _ = std::fs::remove_dir_all(d);
}

/// Imported packs of all three binary variants serve the base events through
/// their keys: headshot kill → HEAD_SHOT, plain kill → PER_KILL, death →
/// DEATH_INSULT, second kill in the chain window → DOUBLE_KILL.
#[test]
fn imported_packs_serve_kills_in_all_variants() {
    use ps2_tracker_core::triggers::{TriggerEngine, TriggerSet};
    use ps2_tracker_core::voice::{Trigger, VoiceSelector, choose};
    let set = TriggerSet::default_set();
    for (data, name, variant) in [
        (sharp_pak(), "map-sharp", Variant::SharpSerializer),
        (nrbf_pak(), "map-nrbf", Variant::BinaryFormatter),
        (pb_pak(), "map-pb", Variant::Protobuf),
    ] {
        let (d, p) = import_vpk(&data, name, variant);
        let packs = [p];
        let hit_all = |facts: &[Trigger]| {
            let mut eng = TriggerEngine::default();
            let events: Vec<_> = facts.iter().flat_map(|f| eng.evaluate(&set, f)).collect();
            let c = choose(
                &mut VoiceSelector::new(3),
                &events,
                &Default::default(),
                &packs,
                &[],
                0,
            );
            let b = c.best.unwrap_or_else(|| {
                panic!("{name}: nothing for {} ({:?})", facts[0].event, c.tried)
            });
            assert_eq!(b.pack_id, packs[0].id);
            b.event
        };
        let hit = |fact: Trigger| hit_all(&[fact]);
        let mut hs = Trigger::new("kill");
        hs.headshot = true;
        assert_eq!(hit(hs), "HEAD_SHOT", "{name}");
        assert_eq!(hit(Trigger::new("kill")), "PER_KILL", "{name}");
        assert_eq!(hit(Trigger::new("death")), "DEATH_INSULT", "{name}");
        let mut chain = Trigger::new("multikill");
        chain.count = 2;
        assert_eq!(
            hit_all(&[Trigger::new("kill"), chain]),
            "DOUBLE_KILL",
            "{name}"
        );
        let _ = std::fs::remove_dir_all(d);
    }
}

// ------------------------------------------------------------ Deadpool layout
// fileSoundPath = default everywhere, the sounds are in the `sounds` list;
// `soundFile` holds absolute Windows/UNC paths of the pack author.

type DpSound = (&'static str, &'static str);
fn dp_achievements() -> Vec<(&'static str, Vec<DpSound>)> {
    vec![
        (
            "PER_KILL",
            vec![
                (
                    r"C:\Users\Someone\Desktop\DeadpoolVP\see ya.ogg",
                    "see ya.ogg_System.Byte",
                ),
                // no pakSoundFile: embedded under its file name
                (r"C:\Users\Someone\Desktop\DeadpoolVP\yea.ogg", ""),
                (
                    r"C:\Users\Someone\Desktop\DeadpoolVP\direct hit.ogg",
                    "direct hit.ogg_System.Byte",
                ),
            ],
        ),
        (
            "DEATH_INSULT",
            vec![
                (
                    r"C:\Users\Someone\Desktop\DeadpoolVP\ah come on.ogg",
                    "ah come on.ogg_System.Byte",
                ),
                // only as a loose file in "ogg files/" next to the definition
                (r"C:\Users\Someone\Desktop\DeadpoolVP\pony.ogg", ""),
            ],
        ),
        (
            "MUTUAL",
            vec![
                (
                    r"C:\Users\Someone\Desktop\DeadpoolVP\your mom says hi.ogg",
                    "your mom says hi.ogg_System.Byte",
                ),
                // UNC path, nowhere to be found: warning, no abort
                (r"\\server\share\blowme.ogg", ""),
            ],
        ),
    ]
}
const DP_EMBEDDED: [&str; 6] = [
    "see ya.ogg_System.Byte",
    "yea.ogg_System.Byte",
    "direct hit.ogg_System.Byte",
    "ah come on.ogg_System.Byte",
    "your mom says hi.ogg_System.Byte",
    "never used.ogg_System.Byte",
];

fn dp_sharp() -> Vec<u8> {
    let mut w = SW::default();
    w.complex(
        Some("Root"),
        Some("RecursionTracker.PAKFormat, RTLibrary"),
        2,
    );
    w.collection(Some("objects"), None, Some("System.Object, mscorlib"), 2);
    w.complex(
        None,
        Some("RecursionTracker.ComponentInformation, RTLibrary"),
        3,
    );
    w.str_prop(Some("Name"), &b64("Deadpool Layout"));
    w.str_prop(Some("description"), &b64("synthetic"));
    w.str_prop(Some("author"), &b64("Test"));
    let achs = dp_achievements();
    w.complex(None, Some("RecursionTracker.Plugins.PlanetSide2.AchievementOptionsComponents+GroupManager, RTPluginPS2"), 1);
    w.collection(Some("achievementList"), None, None, achs.len() as i32);
    for (key, sounds) in &achs {
        w.complex(None, None, 6);
        w.str_prop(Some("Key"), &b64(key));
        w.str_prop(Some("fileSoundPath"), &b64("default"));
        w.str_prop(Some("pakSoundPath"), "");
        w.bool_prop("soundEnabled", true);
        w.bool_prop("dynamicSounds", true);
        w.collection(Some("sounds"), None, None, sounds.len() as i32);
        for (sf, pf) in sounds {
            w.complex(None, None, 2);
            w.str_prop(Some("soundFile"), &b64(sf));
            w.str_prop(Some("pakSoundFile"), &b64(pf));
        }
    }
    w.dictionary(
        Some("pakObjects"),
        Some("System.String, mscorlib"),
        Some("System.Object, mscorlib"),
        DP_EMBEDDED.len() as i32,
    );
    for name in DP_EMBEDDED {
        w.str_prop(None, &b64(name));
        w.bytes_prop(None, Some("System.Byte[], mscorlib"), &tone());
    }
    w.buf
}

fn dp_pb() -> Vec<u8> {
    let mut top = Vec::new();
    pw::bytes(
        &mut top,
        1,
        &pb_component(
            "RecursionTracker.ComponentInformation, RTLibrary",
            &[],
            b"x",
        ),
    );
    let mut gm = Vec::new();
    for (key, sounds) in dp_achievements() {
        let mut o = Vec::new();
        pw::bytes(&mut o, 1, b"default");
        pw::uint(&mut o, 2, 1);
        for (sf, pf) in sounds {
            let mut sm = Vec::new();
            pw::bytes(&mut sm, 1, sf.as_bytes());
            if !pf.is_empty() {
                pw::bytes(&mut sm, 2, pf.as_bytes());
            }
            pw::bytes(&mut o, 6, &sm);
        }
        let mut e = Vec::new();
        pw::bytes(&mut e, 1, key.as_bytes());
        pw::bytes(&mut e, 2, &o);
        pw::bytes(&mut gm, 1, &e);
    }
    pw::bytes(
        &mut top,
        1,
        &pb_component(
            "RecursionTracker.AchievementOptionsComponents+GroupManager, RTPluginPS2",
            &[],
            &gm,
        ),
    );
    for name in DP_EMBEDDED {
        pw::bytes(
            &mut top,
            1,
            &pb_component("System.Byte, mscorlib", &[(6, name.as_bytes())], &tone()),
        );
    }
    top
}

#[test]
fn deadpool_layout_sound_lists_and_foreign_paths() {
    use ps2_tracker_core::triggers::{TriggerEngine, TriggerSet};
    use ps2_tracker_core::voice::{Trigger, VoiceSelector, choose};
    use ps2_tracker_core::voicepack;
    for (data, name, variant) in [
        (dp_sharp(), "dp-sharp", Variant::SharpSerializer),
        (dp_pb(), "dp-pb", Variant::Protobuf),
    ] {
        let d = tmp(name);
        assert_eq!(recursion_pak::detect(&data), Some(variant));
        let vpk = d.join(format!("{name}.rtst_vpk"));
        std::fs::write(&vpk, &data).unwrap();
        std::fs::create_dir_all(d.join("ogg files")).unwrap();
        std::fs::write(d.join("ogg files/pony.ogg"), tone()).unwrap();
        let lib = d.join("lib");
        let (_, p) = packloader::import(&vpk, &lib).unwrap();
        let files = |k: &str| {
            p.manifest
                .events
                .iter()
                .find(|e| e.event == k)
                .map_or(0, |e| e.files.len())
        };
        assert_eq!(files("PER_KILL"), 3, "{name}: {:?}", p.warnings);
        assert_eq!(files("DEATH_INSULT"), 2, "{name}: {:?}", p.warnings);
        assert_eq!(files("MUTUAL"), 1, "{name}: {:?}", p.warnings);
        assert!(
            p.warnings
                .iter()
                .any(|w| w.contains("absolute path ignored")),
            "{:?}",
            p.warnings
        );
        assert!(
            !p.warnings
                .iter()
                .any(|w| w.contains("Someone") || w.contains("server")),
            "{:?}",
            p.warnings
        );
        assert_eq!(
            (p.stats.referenced, p.stats.unreferenced, p.stats.loose),
            (5, 1, 1),
            "{name}"
        );
        // standard tones at rank 1: the pack still serves kill, death and MUTUAL
        voicepack::install_standard_pack(&lib).unwrap();
        let std_pack = voicepack::list_packs(&lib)
            .0
            .into_iter()
            .find(|x| x.id == voicepack::STANDARD_PACK_ID)
            .unwrap();
        let packs = [std_pack, p.clone()];
        let set = TriggerSet::default_set();
        let hit = |fact: Trigger| {
            let events = TriggerEngine::default().evaluate(&set, &fact);
            let c = choose(
                &mut VoiceSelector::new(9),
                &events,
                &Default::default(),
                &packs,
                &[],
                0,
            );
            let b = c.best.unwrap();
            (b.event, b.pack_id, c.tried)
        };
        let (ev, pack, tried) = hit(Trigger::new("kill"));
        assert_eq!(
            (ev.as_str(), pack.as_str()),
            ("PER_KILL", p.id.as_str()),
            "{tried:?}"
        );
        let (ev, pack, _) = hit(Trigger::new("death"));
        assert_eq!(
            (ev.as_str(), pack.as_str()),
            ("DEATH_INSULT", p.id.as_str())
        );
        let (ev, pack, tried) = hit(Trigger::new("mutual"));
        assert_eq!(
            (ev.as_str(), pack.as_str()),
            ("MUTUAL", p.id.as_str()),
            "{tried:?}"
        );
        let _ = std::fs::remove_dir_all(d);
    }
}
