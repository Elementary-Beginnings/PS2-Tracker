//! Session file (JSON) imported through "Import session".
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(f) = ps2_tracker_core::sessionfile::from_json(text) {
        let _ = f.validate(1_800_000_000);
    }
});
