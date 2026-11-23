//! Diagnosis for a binary Recursion pack: reads it, converts it into a temporary
//! folder and prints per key how many sounds were found, plus all notes.
//! Nothing is written next to the pack.
//!
//!     cargo run -p ps2-tracker-core --example pak_probe -- "/path/Deadpool v2.rtst_vpk"

use std::path::Path;

use ps2_tracker_core::recursion_pak;

fn main() {
    let Some(arg) = std::env::args().nth(1) else {
        eprintln!("usage: pak_probe <file.rtst_vpk>");
        std::process::exit(2);
    };
    let vpk = Path::new(&arg);
    let data = std::fs::read(vpk).expect("read file");
    let c = match recursion_pak::read(&data) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("read failed: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "variant {:?}, {} entries, {} embedded blobs, {} images, background name {:?}, disabled {}",
        c.variant,
        c.entries.len(),
        c.blobs.len(),
        c.images.len(),
        c.background_name,
        c.background_disabled
    );
    let mut refs = 0;
    let mut hits = 0;
    for e in &c.entries {
        for s in &e.sounds {
            if let Some(n) = &s.embedded {
                refs += 1;
                if c.blobs.contains_key(&recursion_pak::blob_key(n)) {
                    hits += 1;
                }
            }
        }
    }
    println!("pakSoundFile/pakSoundPath references: {refs}, found in the embedded data: {hits}");
    // keys that occur more than once, and per key of interest every entry
    // with its sound references (names only, no audio data)
    let mut count: std::collections::BTreeMap<&str, usize> = Default::default();
    for e in &c.entries {
        *count.entry(e.key.as_str()).or_default() += 1;
    }
    let dups: Vec<_> = count.iter().filter(|(_, n)| **n > 1).collect();
    println!("keys with more than one entry: {dups:?}");
    for k in ["PER_KILL", "DEATH_INSULT", "HEAD_SHOT"] {
        for (i, e) in c.entries.iter().enumerate().filter(|(_, e)| e.key == k) {
            println!(
                "  entry #{i} {k}: sound={} sounds={:?}",
                e.sound,
                e.sounds
                    .iter()
                    .map(|s| (s.embedded.clone(), s.loose.clone()))
                    .collect::<Vec<_>>()
            );
            let matching: Vec<&String> = e
                .refs
                .iter()
                .filter(|r| c.blobs.contains_key(&recursion_pak::blob_key(r)))
                .collect();
            println!("    texts in the entry naming embedded data: {matching:?}");
            println!(
                "    other texts in the entry: {:?}",
                e.refs
                    .iter()
                    .filter(|r| !c.blobs.contains_key(&recursion_pak::blob_key(r)))
                    .take(12)
                    .collect::<Vec<_>>()
            );
        }
    }
    let mut names: Vec<&String> = c.blobs.keys().collect();
    names.sort();
    println!(
        "embedded names (first 40 of {}): {:?}",
        names.len(),
        &names[..names.len().min(40)]
    );
    let dest = std::env::temp_dir().join(format!("pak_probe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dest);
    let root = vpk.parent().unwrap_or(Path::new("."));
    match recursion_pak::convert(&c, root, &dest, "probe") {
        Ok(p) => {
            println!(
                "converted: extracted {}, loose {}, referenced {}, unreferenced {}, background {}",
                p.stats.embedded,
                p.stats.loose,
                p.stats.referenced,
                p.stats.unreferenced,
                p.stats.background
            );
            for k in [
                "PER_KILL",
                "DEATH_INSULT",
                "DEATH_STREAK",
                "MUTUAL",
                "HEAD_SHOT",
                "FIRST_BLOOD",
            ] {
                let n = p
                    .manifest
                    .events
                    .iter()
                    .find(|e| e.event == k)
                    .map(|e| e.files.len());
                let src = c
                    .entries
                    .iter()
                    .find(|e| e.key == k)
                    .map(|e| e.sounds.len());
                println!("  {k}: sounds in the pack {src:?}, files after import {n:?}");
            }
            for w in &p.warnings {
                println!("  note: {w}");
            }
        }
        Err(e) => println!("convert failed: {e}"),
    }
    let _ = std::fs::remove_dir_all(&dest);
}
