//! .NET BinaryFormatter stream (NRBF) inside Recursion packs, with the limits
//! the import uses.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ps2_tracker_core::nrbf;

fuzz_target!(|data: &[u8]| {
    let _ = nrbf::parse(data, nrbf::Limits::default());
});
