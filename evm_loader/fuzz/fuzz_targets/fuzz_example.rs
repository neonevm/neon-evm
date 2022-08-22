#![no_main]
use libfuzzer_sys::fuzz_target;
use std::convert::TryInto;

fuzz_target!(|data: &[u8]| {
    // Uncomment for testing.
    // if u32::from_ne_bytes(data.try_into().unwrap_or_default()) == u32::MAX {
    //     panic!("Found error");
    // }
});