//! Pure parsers for `.prod` XML and os-release text (no SFTP).

use quick_xml::Reader;
use quick_xml::events::{BytesRef, Event};

use crate::types::{Product, System};

/// Resolve a `&name;` / `&#N;` general-entity reference to its text, the way
/// Python's ElementTree inlines it. Returns `None` for an undefined entity
/// (where Python raises `ParseError`); callers treat that as malformed input.
pub(crate) fn resolve_general_ref(e: &BytesRef<'_>) -> Option<String> {
    if let Ok(Some(ch)) = e.resolve_char_ref() {
        return Some(ch.to_string());
    }
    let name = e.decode().ok()?;
    quick_xml::escape::resolve_predefined_entity(&name).map(str::to_string)
}

/// Index of `tag` in the tracked `.prod` fields, if it is one.
fn field_index(tag: &[u8]) -> Option<usize> {
    [
        b"name".as_slice(),
        b"arch",
        b"baseversion",
        b"patchlevel",
        b"version",
    ]
    .iter()
    .position(|f| *f == tag)
}

/// Parse one `.prod` XML document into a product, or `None` if malformed.
///
/// Mirrors Python `__parse_product`, which selects fields with
/// `root.find("./name")` (and `./arch`, `./version`, `./baseversion`,
/// `./patchlevel`) — i.e. the **first direct child** of the root `<product>`
/// element. Real `.prod` files carry a second, nested `<codestream><name>`
/// (the friendly/summary name); it must NOT clobber the canonical one, so we
/// only consider elements whose parent is the root (depth 1) and commit the
/// **first element** per field (an empty first `<name></name>` shadows a later
/// text-bearing one, exactly like `find`'s first-match + `.text is None`).
///
/// Like ElementTree's `.text`, a field's value is the concatenation of the
/// character data — text, resolved entity/char references, CDATA — between
/// its start tag and its first child element (comments do not interrupt it).
fn parse_prod_xml(xml: &str, _filename: &str) -> Option<Product> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    // Per-field committed `.text` (index shape from `field_index`). `Some("")`
    // models Python's "element seen, `.text` is None/empty" — falsy downstream.
    let mut texts: [Option<String>; 5] = Default::default();
    // Whether each field's element has been seen at depth 1: the FIRST element
    // wins the field even if it carries no text.
    let mut seen = [false; 5];
    // The root `<product>` is at depth 0, so its direct children sit at depth 1.
    let mut depth: i32 = 0;
    // Field whose depth-1 element is currently open and collecting `.text`.
    let mut active: Option<usize> = None;
    let mut acc = String::new();
    // `.text` ends at the first child element (its content and everything
    // after it belongs to child text/tails in ElementTree).
    let mut text_done = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if active.is_some() {
                    text_done = true;
                } else if depth == 1
                    && let Some(idx) = field_index(e.name().as_ref())
                    && !seen[idx]
                {
                    seen[idx] = true;
                    active = Some(idx);
                    acc.clear();
                    text_done = false;
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                // Self-closing element: immediately closed, carries no text —
                // Python's `.text` is None. Crucially it does NOT stay
                // "current": tail text after it belongs to no field.
                if active.is_some() {
                    text_done = true;
                } else if depth == 1
                    && let Some(idx) = field_index(e.name().as_ref())
                    && !seen[idx]
                {
                    seen[idx] = true;
                    texts[idx] = Some(String::new());
                }
            }
            // Character data is fragmented across Text / GeneralRef / CData
            // events: accumulate every piece of the active field's `.text`.
            Ok(Event::Text(t)) => {
                if active.is_some() && !text_done {
                    acc.push_str(&t.decode().ok()?);
                }
            }
            Ok(Event::CData(t)) => {
                if active.is_some() && !text_done {
                    acc.push_str(&t.decode().ok()?);
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if active.is_some() && !text_done {
                    // Undefined entity → Python ParseError → malformed here.
                    acc.push_str(&resolve_general_ref(&e)?);
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 1
                    && let Some(idx) = active.take()
                {
                    // Whitespace trim is a documented delta (Python keeps
                    // `.text` verbatim); real `.prod` files are unpadded.
                    texts[idx] = Some(acc.trim().to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }

    let [name, arch, baseversion, patchlevel, version] = texts;
    let name = name.filter(|s| !s.is_empty())?;
    let arch = arch.filter(|s| !s.is_empty())?;
    // Intentional delta: with `<baseversion>` present but `<patchlevel>` entirely
    // absent, Python's fragile `find("./patchlevel").text` raises and discards the
    // baseversion (falling back to `<version>`, else None). Rust keeps the
    // baseversion as the version. Real `.prod` files always pair the two, so this
    // only differs on malformed input (excluded from the golden vectors).
    let mut ver = if let Some(bv) = baseversion.filter(|s| !s.is_empty()) {
        let mut v = bv;
        if let Some(sp) = patchlevel
            && sp != "0"
            && !sp.is_empty()
        {
            v = format!("{v}-SP{sp}");
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
fn parse_os_release(text: &str) -> (String, String, String) {
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

/// One `/etc/products.d/*.prod` addon candidate the caller already fetched.
#[derive(Debug, Clone)]
pub struct ProdFile {
    /// Basename, e.g. `sle-module-basesystem.prod`.
    pub filename: String,
    /// File contents, or `None` if the read failed (candidate is skipped).
    pub xml: Option<String>,
}

/// Failure modes of [`parse_system`], mirroring Python `UnknownSystemError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseSystemError {
    /// `/etc/products.d/baseproduct` symlink did not resolve.
    NoBaseproductTarget,
    /// The base `.prod` file could not be read.
    BaseproductUnreadable { filename: String },
    /// The base `.prod` file parsed to no product.
    BaseproductMalformed { filename: String },
}

impl std::fmt::Display for ParseSystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBaseproductTarget => write!(f, "baseproduct symlink did not resolve"),
            Self::BaseproductUnreadable { filename } => {
                write!(f, "base product {filename} could not be read")
            }
            Self::BaseproductMalformed { filename } => {
                write!(f, "base product {filename} is malformed")
            }
        }
    }
}

impl std::error::Error for ParseSystemError {}

/// Python `name.rpartition("-")[-1] == "migration"`.
fn is_migration_name(name: &str) -> bool {
    name.rsplit_once('-').map_or(name, |(_, tail)| tail) == "migration"
}

/// Pure port of Python `parse_system` (target/parsers/product.py). No SSH.
///
/// `products_d` is `Some` when `listdir("/etc/products.d")` succeeded (the
/// SUSE path — even an empty listing) and `None` when it failed (os-release /
/// rhel6 fallback). On the SUSE path the base is chosen by the `baseproduct`
/// symlink (`baseproduct_link`, path-stripped), read via `base_xml`; addons
/// are the other `.prod` files, deduped, with `*-migration` products skipped
/// by parsed **name**. `transactional` is honored only on the SUSE path —
/// Python never computes it in a fallback, so it is forced `false` there.
pub fn parse_system(
    products_d: Option<&[ProdFile]>,
    baseproduct_link: Option<&str>,
    base_xml: Option<&str>,
    os_release: Option<&str>,
    transactional: bool,
) -> Result<System, ParseSystemError> {
    let files = match products_d {
        None => {
            let base = match os_release {
                Some(text) => {
                    let (name, version, arch) = parse_os_release(text);
                    Product {
                        name,
                        version,
                        arch,
                    }
                }
                None => Product {
                    name: "rhel".into(),
                    version: "6".into(),
                    arch: "x86_64".into(),
                },
            };
            return Ok(System {
                base,
                addons: vec![],
                transactional: false,
            });
        }
        Some(files) => files,
    };

    let raw = baseproduct_link.ok_or(ParseSystemError::NoBaseproductTarget)?;
    let base_file = raw.rsplit_once('/').map_or(raw, |(_, tail)| tail);

    let bx = base_xml.ok_or_else(|| ParseSystemError::BaseproductUnreadable {
        filename: base_file.to_string(),
    })?;
    let base =
        parse_prod_xml(bx, base_file).ok_or_else(|| ParseSystemError::BaseproductMalformed {
            filename: base_file.to_string(),
        })?;

    let mut addons: Vec<Product> = Vec::new();
    for pf in files {
        if pf.filename == base_file {
            continue;
        }
        let Some(xml) = pf.xml.as_deref() else {
            continue;
        };
        let Some(p) = parse_prod_xml(xml, &pf.filename) else {
            continue;
        };
        if is_migration_name(&p.name) {
            continue;
        }
        if !addons.contains(&p) {
            addons.push(p);
        }
    }

    Ok(System {
        base,
        addons,
        transactional,
    })
}

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
    fn codestream_name_does_not_clobber_canonical_simple_version() {
        // Real `.prod` shape (SL-Micro): canonical <name> at depth 1 plus a
        // nested <codestream><name> friendly name. The direct child must win.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<product schemeversion="0">
  <vendor>SUSE</vendor>
  <name>SL-Micro</name>
  <version>6.1</version>
  <arch>x86_64</arch>
  <productline>SL-Micro</productline>
  <codestream>
    <name>SUSE Linux Micro 6.1</name>
    <endoflife>2026-10-31</endoflife>
  </codestream>
  <summary>SUSE Linux Micro 6.1</summary>
</product>"#;
        let p = parse_prod_xml(xml, "SL-Micro.prod").unwrap();
        assert_eq!(p.name, "SL-Micro");
        assert_eq!(p.version, "6.1");
        assert_eq!(p.arch, "x86_64");
    }

    #[test]
    fn codestream_name_does_not_clobber_canonical_baseversion() {
        // Real `.prod` shape (SLES_SAP): depth-1 <baseversion>/<patchlevel>
        // win over the depth-1 <version>, and the nested <codestream><name>
        // must not overwrite the canonical <name>.
        let xml = r#"<product schemeversion="0">
  <name>SLES_SAP</name>
  <version>12.5</version>
  <baseversion>12</baseversion>
  <patchlevel>5</patchlevel>
  <arch>x86_64</arch>
  <productline>sles</productline>
  <codestream>
    <name>SUSE Linux Enterprise Server 12</name>
  </codestream>
  <summary>SUSE Linux Enterprise Server for SAP Applications 12 SP5</summary>
</product>"#;
        let p = parse_prod_xml(xml, "SLES_SAP.prod").unwrap();
        assert_eq!(p.name, "SLES_SAP");
        assert_eq!(p.version, "12-SP5");
        assert_eq!(p.arch, "x86_64");
    }

    #[test]
    fn caasp_all() {
        let xml = r#"<?xml version="1.0"?>
<product><name>CAASP</name><version>4.0</version><arch>x86_64</arch></product>"#;
        let p = parse_prod_xml(xml, "CAASP.prod").unwrap();
        assert_eq!(p.version, "ALL");
    }

    /// Tail text after a self-closing depth-1 element must not be attributed
    /// to it (Python: `<arch/>` → `.text is None` → malformed).
    #[test]
    fn self_closing_field_with_tail_text_is_malformed() {
        let xml = "<product><arch/>x86_64<name>S</name><version>1</version></product>";
        assert_eq!(parse_prod_xml(xml, "t.prod"), None);
        let xml = "<product><name/>SLES<version>1</version><arch>x86_64</arch></product>";
        assert_eq!(parse_prod_xml(xml, "t.prod"), None);
    }

    /// The FIRST element wins per field, even when empty: a later text-bearing
    /// duplicate must not fill the field (Python `find` + `.text is None`).
    #[test]
    fn first_element_wins_even_when_empty() {
        let xml =
            "<product><name></name><name>SLES</name><version>1</version><arch>a</arch></product>";
        assert_eq!(parse_prod_xml(xml, "t.prod"), None);
        let xml = "<product><name>FIRST</name><name>SECOND</name><version>1</version><arch>a</arch></product>";
        let p = parse_prod_xml(xml, "t.prod").unwrap();
        assert_eq!(p.name, "FIRST");
    }

    /// Entity references, comments, and CDATA fragment the character data;
    /// the pieces must be accumulated and resolved like ElementTree's `.text`.
    #[test]
    fn fragmented_text_entities_comments_cdata() {
        let entity = "<product><name>A&amp;B</name><version>1</version><arch>a</arch></product>";
        assert_eq!(parse_prod_xml(entity, "t.prod").unwrap().name, "A&B");
        let comment =
            "<product><name>A<!-- c -->B</name><version>1</version><arch>a</arch></product>";
        assert_eq!(parse_prod_xml(comment, "t.prod").unwrap().name, "AB");
        let cdata =
            "<product><name><![CDATA[SLES]]></name><version>1</version><arch>a</arch></product>";
        assert_eq!(parse_prod_xml(cdata, "t.prod").unwrap().name, "SLES");
        let mixed =
            "<product><name>SL<![CDATA[ES]]></name><version>1</version><arch>a</arch></product>";
        assert_eq!(parse_prod_xml(mixed, "t.prod").unwrap().name, "SLES");
        let charref = "<product><name>S&#76;ES</name><version>1</version><arch>a</arch></product>";
        assert_eq!(parse_prod_xml(charref, "t.prod").unwrap().name, "SLES");
    }

    /// `.text` stops at the first child element; the child's content and its
    /// tail belong to the child, not the field.
    #[test]
    fn text_stops_at_first_child_element() {
        let xml = "<product><name>REAL<sub>X</sub>tail</name><version>1</version><arch>a</arch></product>";
        let p = parse_prod_xml(xml, "t.prod").unwrap();
        assert_eq!(p.name, "REAL");
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

    #[test]
    fn matches_vector_parse_prod() {
        let raw = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/vectors/product/parse_prod.json"),
        )
        .expect("vector product/parse_prod.json");
        for case in serde_json::from_str::<Vec<serde_json::Value>>(&raw).unwrap() {
            let name = case["name"].as_str().unwrap();
            let xml = case["input"]["xml"].as_str().unwrap();
            let fname = case["input"]["filename"].as_str().unwrap();
            match (parse_prod_xml(xml, fname), &case["expected"]) {
                (None, serde_json::Value::Null) => {}
                (Some(p), exp) => {
                    assert_eq!(p.name, exp["name"].as_str().unwrap(), "case {name} name");
                    assert_eq!(
                        p.version,
                        exp["version"].as_str().unwrap(),
                        "case {name} version"
                    );
                    assert_eq!(p.arch, exp["arch"].as_str().unwrap(), "case {name} arch");
                }
                (got, exp) => panic!("case {name}: got {got:?} expected {exp}"),
            }
        }
    }

    fn sles_xml() -> String {
        "<product><name>SLES</name><baseversion>15</baseversion><patchlevel>6</patchlevel><arch>x86_64</arch></product>".into()
    }

    fn simple_xml(name: &str) -> String {
        format!("<product><name>{name}</name><version>1</version><arch>x86_64</arch></product>")
    }

    fn pf(filename: &str, xml: String) -> ProdFile {
        ProdFile {
            filename: filename.into(),
            xml: Some(xml),
        }
    }

    fn sles(v: &str) -> Product {
        Product {
            name: "SLES".into(),
            version: v.into(),
            arch: "x86_64".into(),
        }
    }

    #[test]
    fn parse_system_normal_base_and_addons() {
        let files = vec![
            pf("SLES.prod", sles_xml()),
            pf(
                "sle-module-basesystem.prod",
                simple_xml("sle-module-basesystem"),
            ),
            pf(
                "sle-module-server-applications.prod",
                simple_xml("sle-module-server-applications"),
            ),
        ];
        let sys = parse_system(
            Some(&files),
            Some("SLES.prod"),
            Some(&sles_xml()),
            None,
            false,
        )
        .unwrap();
        assert_eq!(sys.base, sles("15-SP6"));
        let mut names: Vec<&str> = sys.addons.iter().map(|p| p.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            ["sle-module-basesystem", "sle-module-server-applications"]
        );
        assert!(!sys.transactional);
    }

    #[test]
    fn parse_system_uses_canonical_names_with_codestream() {
        // End-to-end: a base and an addon each carrying a nested
        // <codestream><name>. The canonical direct-child names must survive.
        let base_xml = "<product><name>SL-Micro</name><version>6.1</version><arch>x86_64</arch><codestream><name>SUSE Linux Micro 6.1</name></codestream></product>";
        let extras_xml = "<product><name>SL-Micro-Extras</name><version>6.1</version><arch>x86_64</arch><codestream><name>SUSE Linux Micro 6.1</name></codestream></product>";
        let files = vec![pf("SL-Micro-Extras.prod", extras_xml.into())];
        let sys = parse_system(
            Some(&files),
            Some("SL-Micro.prod"),
            Some(base_xml),
            None,
            false,
        )
        .unwrap();
        assert_eq!(sys.base.name, "SL-Micro");
        assert_eq!(sys.base.version, "6.1");
        assert_eq!(sys.addons.len(), 1);
        assert_eq!(sys.addons[0].name, "SL-Micro-Extras");
        assert_eq!(sys.addons[0].version, "6.1");
    }

    #[test]
    fn parse_system_path_prefixed_symlink_and_migration_by_name() {
        let files = vec![
            pf(
                "sle-module-foo-migration.prod",
                simple_xml("sle-module-foo-migration"),
            ),
            pf(
                "sle-module-basesystem.prod",
                simple_xml("sle-module-basesystem"),
            ),
        ];
        let sys = parse_system(
            Some(&files),
            Some("/etc/products.d/SLES.prod"),
            Some(&sles_xml()),
            None,
            false,
        )
        .unwrap();
        assert_eq!(sys.base, sles("15-SP6"));
        assert_eq!(sys.addons.len(), 1);
        assert_eq!(sys.addons[0].name, "sle-module-basesystem");
    }

    #[test]
    fn parse_system_migration_skip_is_by_name_not_filename() {
        // Filename contains `-migration.` but the product name is a real addon.
        let files = vec![pf("foo-migration.prod", simple_xml("sle-module-foo"))];
        let sys = parse_system(
            Some(&files),
            Some("SLES.prod"),
            Some(&sles_xml()),
            None,
            false,
        )
        .unwrap();
        assert_eq!(sys.addons.len(), 1);
        assert_eq!(sys.addons[0].name, "sle-module-foo");
    }

    #[test]
    fn parse_system_product_named_exactly_migration_is_skipped() {
        let files = vec![pf("migration.prod", simple_xml("migration"))];
        let sys = parse_system(
            Some(&files),
            Some("SLES.prod"),
            Some(&sles_xml()),
            None,
            false,
        )
        .unwrap();
        assert!(sys.addons.is_empty());
    }

    #[test]
    fn parse_system_transactional_only_on_suse_path() {
        let sys = parse_system(
            Some(&[pf("SLES.prod", sles_xml())]),
            Some("SLES.prod"),
            Some(&sles_xml()),
            None,
            true,
        )
        .unwrap();
        assert!(sys.transactional);
        assert!(sys.addons.is_empty());
    }

    #[test]
    fn parse_system_non_suse_fallback_forces_transactional_false() {
        // transactional=true is deliberately passed; the fallback must ignore it.
        let sys = parse_system(None, None, None, Some("ID=rhel\nVERSION_ID=9.3\n"), true).unwrap();
        assert_eq!(sys.base.name, "rhel");
        assert_eq!(sys.base.version, "9.3");
        assert!(!sys.transactional);
    }

    #[test]
    fn parse_system_rhel6_synthetic_fallback() {
        let sys = parse_system(None, None, None, None, true).unwrap();
        assert_eq!(
            sys.base,
            Product {
                name: "rhel".into(),
                version: "6".into(),
                arch: "x86_64".into()
            }
        );
        assert!(!sys.transactional);
    }

    #[test]
    fn parse_system_unresolved_baseproduct_symlink_errors() {
        let err = parse_system(
            Some(&[pf("sle-module-basesystem.prod", simple_xml("x"))]),
            None,
            None,
            None,
            false,
        )
        .unwrap_err();
        assert_eq!(err, ParseSystemError::NoBaseproductTarget);
    }

    #[test]
    fn parse_system_base_read_via_symlink_even_if_absent_from_listing() {
        let files = vec![pf(
            "sle-module-basesystem.prod",
            simple_xml("sle-module-basesystem"),
        )];
        let sys = parse_system(
            Some(&files),
            Some("SLES.prod"),
            Some(&sles_xml()),
            None,
            false,
        )
        .unwrap();
        assert_eq!(sys.base, sles("15-SP6"));
        assert_eq!(sys.addons.len(), 1);
    }

    #[test]
    fn parse_system_malformed_and_unreadable_base_error() {
        let malformed =
            parse_system(Some(&[]), Some("SLES.prod"), Some("<nope/>"), None, false).unwrap_err();
        assert!(matches!(
            malformed,
            ParseSystemError::BaseproductMalformed { .. }
        ));
        let unreadable = parse_system(Some(&[]), Some("SLES.prod"), None, None, false).unwrap_err();
        assert!(matches!(
            unreadable,
            ParseSystemError::BaseproductUnreadable { .. }
        ));
    }

    #[test]
    fn parse_system_dedups_identical_addons() {
        let files = vec![
            pf("a.prod", simple_xml("dup")),
            pf("b.prod", simple_xml("dup")),
        ];
        let sys = parse_system(
            Some(&files),
            Some("SLES.prod"),
            Some(&sles_xml()),
            None,
            false,
        )
        .unwrap();
        assert_eq!(sys.addons.len(), 1);
    }

    #[test]
    fn matches_vector_os_release() {
        let raw = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/vectors/product/os_release.json"),
        )
        .expect("vector product/os_release.json");
        for case in serde_json::from_str::<Vec<serde_json::Value>>(&raw).unwrap() {
            let name = case["name"].as_str().unwrap();
            let (n, v, a) = parse_os_release(case["input"]["text"].as_str().unwrap());
            let exp = &case["expected"];
            assert_eq!(n, exp["name"].as_str().unwrap(), "case {name} name");
            assert_eq!(v, exp["version"].as_str().unwrap(), "case {name} version");
            assert_eq!(a, exp["arch"].as_str().unwrap(), "case {name} arch");
        }
    }
}
