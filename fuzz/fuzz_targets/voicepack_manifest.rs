//! `voicepack.toml` of an imported pack, and the path check of its file list.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ps2_tracker_core::voicepack;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(m) = voicepack::Manifest::parse(text) {
        for f in m.events.iter().flat_map(|e| &e.files) {
            // a parsed manifest only holds paths inside the pack
            let p = voicepack::safe_relative(f).expect("validated path");
            assert!(
                p.components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
            );
        }
    }
    for line in text.lines() {
        if let Some(p) = voicepack::safe_relative(line) {
            assert!(
                p.components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
            );
        }
    }
});
