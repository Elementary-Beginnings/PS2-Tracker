//! Messages of the Census event stream (WebSocket, JSON).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = ps2_tracker_core::events::parse_message(text);
});
