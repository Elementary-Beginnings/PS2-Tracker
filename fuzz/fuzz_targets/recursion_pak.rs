//! Binary Recursion voice pack definition (`.rtst_vpk`): NRBF, SharpSerializer
//! or protobuf, chosen by the content. Imported from files of other people.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = ps2_tracker_core::recursion_pak::read(data);
});
