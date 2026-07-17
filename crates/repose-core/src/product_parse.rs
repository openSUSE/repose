//! Pure parsers for `.prod` XML and os-release text (no SFTP).

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::types::Product;

/// Parse one `.prod` XML document into a product, or `None` if malformed.
pub fn parse_prod_xml(xml: &str, _filename: &str) -> Option<Product> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut name = None;
    let mut arch = None;
    let mut baseversion = None;
    let mut patchlevel = None;
    let mut version = None;
    let mut cur = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                cur = String::from_utf8_lossy(e.name().as_ref()).into_owned();
            }
            Ok(Event::Text(t)) => {
                let text = t.decode().map(|c| c.into_owned()).unwrap_or_default();
                match cur.as_str() {
                    "name" => name = Some(text),
                    "arch" => arch = Some(text),
                    "baseversion" => baseversion = Some(text),
                    "patchlevel" => patchlevel = Some(text),
                    "version" => version = Some(text),
                    _ => {}
                }
            }
            Ok(Event::End(_)) => cur.clear(),
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }

    let name = name.filter(|s| !s.is_empty())?;
    let arch = arch.filter(|s| !s.is_empty())?;
    let mut ver = if let Some(bv) = baseversion.filter(|s| !s.is_empty()) {
        let mut v = bv;
        if let Some(sp) = patchlevel {
            if sp != "0" && !sp.is_empty() {
                v = format!("{v}-SP{sp}");
            }
        }
        v
    } else {
        version.filter(|s| !s.is_empty())?
    };
    if name == "CAASP" {
        ver = "ALL".into();
    }
    Some(Product {
        name,
        version: ver,
        arch,
    })
}

/// Parse `/etc/os-release` body into (name, version, arch).
pub fn parse_os_release(text: &str) -> (String, String, String) {
    let mut values = std::collections::BTreeMap::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let (key, _, value) = {
            let (k, rest) = line.split_once('=').unwrap();
            (
                k.trim(),
                "=",
                rest.trim().trim_matches(|c| c == '"' || c == '\''),
            )
        };
        values.insert(key.to_string(), value.to_string());
    }
    let name = values
        .get("ID")
        .cloned()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "linux".into());
    let version = values.get("VERSION_ID").cloned().unwrap_or_default();
    let arch = values
        .get("ARCHITECTURE")
        .cloned()
        .unwrap_or_else(|| "unknown".into());
    (name, version, arch)
}

pub const TRANSACTIONAL_CONF_PATHS: &[&str] = &[
    "/usr/etc/transactional-update.conf",
    "/etc/transactional-update.conf",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_prod_with_sp() {
        let xml = r#"<?xml version="1.0"?>
<product>
  <name>SLES</name>
  <baseversion>15</baseversion>
  <patchlevel>6</patchlevel>
  <arch>x86_64</arch>
</product>"#;
        let p = parse_prod_xml(xml, "SLES.prod").unwrap();
        assert_eq!(p.name, "SLES");
        assert_eq!(p.version, "15-SP6");
        assert_eq!(p.arch, "x86_64");
    }

    #[test]
    fn caasp_all() {
        let xml = r#"<?xml version="1.0"?>
<product><name>CAASP</name><version>4.0</version><arch>x86_64</arch></product>"#;
        let p = parse_prod_xml(xml, "CAASP.prod").unwrap();
        assert_eq!(p.version, "ALL");
    }

    #[test]
    fn os_release_basic() {
        let text = r#"
ID="sles"
VERSION_ID="15-SP6"
ARCHITECTURE="x86_64"
"#;
        let (n, v, a) = parse_os_release(text);
        assert_eq!(n, "sles");
        assert_eq!(v, "15-SP6");
        assert_eq!(a, "x86_64");
    }
}
