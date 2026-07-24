//! Fuzzes the `/etc/products.d/*.prod` and `os-release` parsers with
//! host-controlled file contents.
#![no_main]

use libfuzzer_sys::fuzz_target;
use repose_core::product_parse::{ProdFile, parse_os_release, parse_prod_xml, parse_system};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = parse_prod_xml(text, "fuzz.prod");
    let _ = parse_os_release(text);
    // SUSE path: one synthesized addon candidate + the same bytes as the
    // baseproduct target, base XML, and os-release content.
    let files = [ProdFile {
        filename: "fuzz.prod".into(),
        xml: Some(text.to_owned()),
    }];
    let _ = parse_system(
        Some(&files),
        Some("fuzz.prod"),
        Some(text),
        Some(text),
        false,
    );
    // Fallback path: no products.d at all, os-release only.
    let _ = parse_system(None, None, None, Some(text), false);
});
