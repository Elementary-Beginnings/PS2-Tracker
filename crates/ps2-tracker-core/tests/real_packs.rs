//! Real Recursion packs from the maintainer's machine, never in the repo.
//! Runs only with `PS2_TESTPACKS=<folder>` (every `.rtst_vpk` in it), through
//! the same import as the app (`packloader::import`, used by `voice_import`):
//!
//!     PS2_TESTPACKS=~/Projekte/ps2-testpacks cargo test -p ps2-tracker-core --test real_packs -- --nocapture
//!
//! Checked per pack: every embedded sound an event names is taken over, no
//! event loses its sounds, and the base keys PER_KILL / DEATH_INSULT have
//! files when the pack names sounds for them.

use std::path::PathBuf;

use ps2_tracker_core::{packloader, recursion_pak};

#[test]
fn real_packs_keep_all_named_sounds() {
    let Ok(dir) = std::env::var("PS2_TESTPACKS") else {
        eprintln!("PS2_TESTPACKS not set: skipped");
        return;
    };
    let mut packs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("PS2_TESTPACKS folder")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("rtst_vpk"))
        })
        .collect();
    packs.sort();
    assert!(!packs.is_empty(), "no .rtst_vpk in {dir}");
    for vpk in packs {
        let data = std::fs::read(&vpk).unwrap();
        let Ok(c) = recursion_pak::read(&data) else {
            eprintln!("{}: XML or unreadable, skipped", vpk.display());
            continue;
        };
        let lib = std::env::temp_dir().join(format!("ps2t-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&lib);
        let (_, p) = packloader::import(&vpk, &lib).unwrap();
        let files = |k: &str| {
            p.manifest
                .events
                .iter()
                .find(|e| e.event == k)
                .map_or(0, |e| e.files.len())
        };
        let named = |k: &str| {
            c.entries.iter().any(|e| {
                e.key == k
                    && e.refs
                        .iter()
                        .any(|r| c.blobs.contains_key(&recursion_pak::blob_key(r)))
            })
        };
        eprintln!(
            "{}: blobs {}, extracted {}, unreferenced {}, PER_KILL {}, DEATH_INSULT {}, HEAD_SHOT {}",
            vpk.display(),
            c.blobs.len(),
            p.stats.embedded,
            p.stats.unreferenced,
            files("PER_KILL"),
            files("DEATH_INSULT"),
            files("HEAD_SHOT")
        );
        for k in ["PER_KILL", "DEATH_INSULT", "HEAD_SHOT"] {
            assert!(
                !named(k) || files(k) > 0,
                "{}: {k} names sounds but has no files",
                vpk.display()
            );
        }
        assert!(
            !p.warnings
                .iter()
                .any(|w| w.contains("could not be taken over")),
            "{}: {:?}",
            vpk.display(),
            p.warnings
        );
        let _ = std::fs::remove_dir_all(&lib);
    }
}
