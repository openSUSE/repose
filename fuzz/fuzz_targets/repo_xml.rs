//! Fuzzes the `zypper -x lr` XML parser with host-controlled output.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = repose_core::repo_parse::parse_repositories(text);
    }
});
