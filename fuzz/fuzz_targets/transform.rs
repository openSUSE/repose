//! Fuzzes the version-transform and repo-name-to-product derivation helpers.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = repose_core::transform::transform_version_partialy(text);
        let _ = repose_core::types::product_from_repo_name(text, "x86_64");
    }
});
