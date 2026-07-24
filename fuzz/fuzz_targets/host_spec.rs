//! Fuzzes the `-t/--target` host spec parser with CLI input.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = repose_core::host_parse::parse_host(text);
    }
});
