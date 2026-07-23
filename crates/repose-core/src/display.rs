//! list-* / known-products display (Python `CommandDisplay` / `JsonCommandDisplay`).

use std::io::{self, Write};

use serde_json::Value;

use crate::types::{Product, Repositories, Repository, System};

/// Addons in a deterministic order (by name, then version, then arch).
///
/// Deterministic addon ordering is part of repose's output contract.
fn sorted_addons(system: &System) -> Vec<&Product> {
    let mut addons: Vec<&Product> = system.get_addons().iter().collect();
    addons.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.version.cmp(&b.version))
            .then_with(|| a.arch.cmp(&b.arch))
    });
    addons
}

/// ANSI color helpers ported from Python `repose.utils` (`green`/`yellow`/
/// `blue`), used by [`TextDisplay`] to color `list-*` / `known-products`
/// labels and values. The sequences (`\x1b[1;3Nm{s}\x1b[1;m\x1b[0m`, double
/// reset) are byte-identical to the Python 2.1.0 helpers. Returns `s`
/// unchanged when `enabled` is false.
fn green(enabled: bool, s: &str) -> String {
    wrap(enabled, "\x1b[1;32m", s)
}

fn yellow(enabled: bool, s: &str) -> String {
    wrap(enabled, "\x1b[1;33m", s)
}

fn blue(enabled: bool, s: &str) -> String {
    wrap(enabled, "\x1b[1;34m", s)
}

fn wrap(enabled: bool, seq: &str, s: &str) -> String {
    if enabled {
        format!("{seq}{s}\x1b[1;m\x1b[0m")
    } else {
        s.to_string()
    }
}

/// One JSON scalar, matching Python `json.dumps` with its default
/// `ensure_ascii=True`: serde_json already escapes ASCII controls, `"` and
/// `\` identically (verified: `json.dumps('tab\tq"b\\s\x01')` ==
/// `serde_json::to_string`), but emits non-ASCII as raw UTF-8 where Python
/// emits `\uXXXX` per UTF-16 code unit (lowercase hex, surrogate pairs for
/// astral chars): U+00E4 becomes `\u00e4`, U+1F600 becomes the
/// surrogate pair `\ud83d\ude00`.
fn js(s: &str) -> String {
    let quoted = serde_json::to_string(s).expect("string always serializes");
    if quoted.is_ascii() {
        return quoted;
    }
    // Escape sequences serde emits are pure ASCII, so every non-ASCII char
    // left in `quoted` is a literal char that json.dumps would \u-escape.
    let mut out = String::with_capacity(quoted.len());
    let mut units = [0u16; 2];
    for c in quoted.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            for u in c.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{u:04x}"));
            }
        }
    }
    out
}

/// A single `list-products` JSON event line, matching Python `json.dumps` with
/// default separators (`", "` / `": "`) and insertion key order
/// (event, host, port, kind, name, version, arch).
fn product_json_line(host: &str, port: u16, kind: &str, p: &Product) -> String {
    format!(
        "{{\"event\": \"product\", \"host\": {}, \"port\": {}, \"kind\": {}, \"name\": {}, \"version\": {}, \"arch\": {}}}",
        js(host),
        port,
        js(kind),
        js(&p.name),
        js(&p.version),
        js(&p.arch),
    )
}

/// A single `list-repos` JSON event line, matching Python `json.dumps` with
/// default separators (`", "` / `": "`) and insertion key order
/// (event, host, port, alias, name, url, state).
fn repo_json_line(host: &str, port: u16, r: &Repository) -> String {
    format!(
        "{{\"event\": \"repo\", \"host\": {}, \"port\": {}, \"alias\": {}, \"name\": {}, \"url\": {}, \"state\": {}}}",
        js(host),
        port,
        js(&r.alias),
        js(&r.name),
        js(&r.url),
        r.state,
    )
}

/// A single `known-products` JSON event line, matching Python `json.dumps`
/// (insertion key order: event, name).
fn known_product_json_line(name: &str) -> String {
    format!("{{\"event\": \"known_product\", \"name\": {}}}", js(name))
}

/// True when the string parses as a YAML 1.2 core int or float, so ruamel
/// would emit it quoted to keep it a string: `0`, `22`, `08`, `+1`, `-1`,
/// `6.1`, `.5`, `1.`, `1e3`. Plain SUSE shapes (`15-SP3`, `SP3`, `ALL`,
/// `tumbleweed`, `3.19.1`) do not match.
fn is_numeric_like(s: &str) -> bool {
    let t = s.strip_prefix(['+', '-']).unwrap_or(s);
    if t.is_empty() {
        return false;
    }
    let (mantissa, exp) = match t.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e)),
        None => (t, None),
    };
    if let Some(e) = exp {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        if e.is_empty() || !e.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    match mantissa.split_once('.') {
        // "6.1", "1." (frac may be empty), ".5" (int part may be empty) —
        // but a lone "." is not numeric.
        Some((int, frac)) => {
            (!int.is_empty() || !frac.is_empty())
                && int.bytes().all(|b| b.is_ascii_digit())
                && frac.bytes().all(|b| b.is_ascii_digit())
        }
        None => !mantissa.is_empty() && mantissa.bytes().all(|b| b.is_ascii_digit()),
    }
}

/// One YAML string scalar, single-quoted exactly where ruamel's safe dumper
/// quotes (verified against ruamel.yaml `YAML(typ='safe')`, see the review
/// evidence): the empty string, int/float-like strings (`'0'`, `'22'`,
/// `'08'`, `'6.1'`), YAML 1.2 core booleans/null (`true`/`True`/`TRUE`,
/// `false`/..., `null`/`Null`/`NULL`, `~` — but NOT the YAML 1.1-only
/// `yes`/`no`/`on`/`off`, which ruamel leaves plain), and strings containing
/// `": "` or `" #"`. Everything else (`15-SP3`, `SP3`, `ALL`, `SLES`,
/// `tumbleweed`, hostnames) stays plain.
fn yaml_string(s: &str) -> String {
    let quote = s.is_empty()
        || is_numeric_like(s)
        || matches!(
            s,
            "true" | "True" | "TRUE" | "false" | "False" | "FALSE" | "null" | "Null" | "NULL" | "~"
        )
        || s.contains(": ")
        || s.contains(" #");
    if quote {
        format!("'{}'", s.replace('\'', "''"))
    } else {
        s.to_string()
    }
}

/// Render one YAML scalar (plain style) from a `transform_version_partialy`
/// leaf: numbers unquoted, strings via [`yaml_string`] — matching ruamel's
/// safe dumper for the SUSE version shapes seen in real `.prod` files.
fn yaml_scalar(v: &Value) -> String {
    match v {
        Value::String(s) => yaml_string(s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// Emit the `version:` block (or inline scalar) for a normalized version at the
/// given `indent`, mirroring ruamel's `mapping=4, sequence=4, offset=2` layout
/// (keys +2, block-sequence dashes at the parent indent).
fn push_version(s: &mut String, v: &Value, indent: &str) {
    if v.is_object() {
        s.push_str(indent);
        s.push_str("version:\n");
        if let Some(major) = v.get("major") {
            s.push_str(&format!("{indent}  major: {}\n", yaml_scalar(major)));
        }
        if let Some(minor) = v.get("minor") {
            s.push_str(&format!("{indent}  minor: {}\n", yaml_scalar(minor)));
        }
    } else {
        // Version shapes that don't normalize (e.g. os-release "tumbleweed")
        // pass through unchanged as an inline scalar.
        s.push_str(&format!("{indent}version: {}\n", yaml_scalar(v)));
    }
}

/// YAML refhost-spec output for `list-products --yaml` (Python
/// `list_products_yaml` → `System.to_refhost_dict_partially_normalized`).
///
/// Hand-rolled to byte-match ruamel's safe dumper: `---`/`...` document
/// markers, alphabetically sorted top-level keys (addons, arch, location,
/// name, product), version leaves run through `transform_version_partialy`.
/// The addon list order is sorted (see `sorted_addons`).
pub fn list_products_yaml<W: Write>(
    out: &mut W,
    hostname: &str,
    system: &System,
) -> io::Result<()> {
    use crate::transform::transform_version_partialy;

    let base = system.get_base();
    let addons = sorted_addons(system);

    let mut s = String::new();
    s.push_str("---\n");
    if addons.is_empty() {
        s.push_str("addons: []\n");
    } else {
        s.push_str("addons:\n");
        for a in &addons {
            s.push_str(&format!("- name: {}\n", yaml_string(&a.name)));
            push_version(&mut s, &transform_version_partialy(&a.version), "  ");
        }
    }
    s.push_str(&format!("arch: {}\n", system.arch()));
    s.push_str("location:\n- some location\n");
    s.push_str(&format!("name: {}\n", yaml_string(hostname)));
    s.push_str("product:\n");
    s.push_str(&format!("  name: {}\n", yaml_string(&base.name)));
    push_version(&mut s, &transform_version_partialy(&base.version), "  ");
    s.push_str("...\n");
    out.write_all(s.as_bytes())
}

/// A `transform_version_partialy` leaf as Python `json.dumps` renders it:
/// normalized versions become `{"major": ..., "minor": ...}` (insertion order
/// major-then-minor, `", "`/`": "` separators), unnormalized ones stay a bare
/// scalar (string via the `ensure_ascii` [`js`], numbers verbatim).
fn version_json(v: &Value) -> String {
    fn scalar(v: &Value) -> String {
        match v {
            Value::String(s) => js(s),
            other => other.to_string(),
        }
    }
    match v.as_object() {
        Some(m) => {
            let mut parts = Vec::new();
            if let Some(major) = m.get("major") {
                parts.push(format!("\"major\": {}", scalar(major)));
            }
            if let Some(minor) = m.get("minor") {
                parts.push(format!("\"minor\": {}", scalar(minor)));
            }
            format!("{{{}}}", parts.join(", "))
        }
        None => scalar(v),
    }
}

/// NDJSON refhost-spec output for `list-products --yaml --format json`
/// (Python `JsonCommandDisplay.list_products_yaml`, display.py:121-127): one
/// `host_spec` document per host, carrying the same payload as the YAML
/// dumper — `{"event": "host_spec", "host": <hostname>,
/// **to_refhost_dict_partially_normalized(), "name": <hostname>}` — i.e. key
/// order event, host, location, arch, product, addons, name, byte-matching
/// `json.dumps` (default separators and `ensure_ascii`). The addon list order
/// is sorted (see `sorted_addons`).
pub(crate) fn list_products_yaml_json<W: Write>(
    out: &mut W,
    hostname: &str,
    system: &System,
) -> io::Result<()> {
    use crate::transform::transform_version_partialy;

    let base = system.get_base();
    let addons: Vec<String> = sorted_addons(system)
        .iter()
        .map(|a| {
            format!(
                "{{\"name\": {}, \"version\": {}}}",
                js(&a.name),
                version_json(&transform_version_partialy(&a.version)),
            )
        })
        .collect();
    writeln!(
        out,
        "{{\"event\": \"host_spec\", \"host\": {host}, \"location\": [\"some location\"], \
         \"arch\": {arch}, \"product\": {{\"name\": {base_name}, \"version\": {base_version}}}, \
         \"addons\": [{addons}], \"name\": {host}}}",
        host = js(hostname),
        arch = js(system.arch()),
        base_name = js(&base.name),
        base_version = version_json(&transform_version_partialy(&base.version)),
        addons = addons.join(", "),
    )
}

pub trait CommandDisplay {
    fn list_products(&mut self, hostname: &str, port: u16, system: &System) -> io::Result<()>;
    fn list_repos(&mut self, hostname: &str, port: u16, repos: &[Repository]) -> io::Result<()>;
    fn list_known_products(&mut self, products: &[String]) -> io::Result<()>;
}

pub struct TextDisplay<W: Write> {
    pub output: W,
    /// Emit ANSI color (Python `CommandDisplay` via `utils` color helpers).
    pub color: bool,
}

impl<W: Write> CommandDisplay for TextDisplay<W> {
    fn list_products(&mut self, hostname: &str, port: u16, system: &System) -> io::Result<()> {
        // Mirrors Python `CommandDisplay.list_products` + `System.pretty`:
        // `Host` green, hostname/port yellow; the `pretty()` lines stay plain.
        let base = &system.base;
        writeln!(
            self.output,
            "{}: {}:{}",
            green(self.color, "Host"),
            yellow(self.color, hostname),
            yellow(self.color, &port.to_string()),
        )?;
        writeln!(
            self.output,
            "  Base product: {}-{}-{}",
            base.name, base.version, base.arch
        )?;
        let addons = sorted_addons(system);
        if !addons.is_empty() {
            writeln!(self.output, "  Installed Extensions and Modules:")?;
            for a in &addons {
                // Python: f"      Addon: {x.name:<53} - version: {x.version}"
                writeln!(
                    self.output,
                    "      Addon: {:<53} - version: {}",
                    a.name, a.version
                )?;
            }
        }
        writeln!(self.output)?;
        Ok(())
    }

    fn list_repos(&mut self, hostname: &str, port: u16, repos: &[Repository]) -> io::Result<()> {
        // Python `list_update_repos`: `Repositories` green, host/port blue;
        // per repo the `REPO name`/`REPO URL` labels green, values plain.
        writeln!(
            self.output,
            "{} on {}:{}",
            green(self.color, "Repositories"),
            blue(self.color, hostname),
            blue(self.color, &port.to_string()),
        )?;
        for r in repos {
            writeln!(
                self.output,
                "{}: {}",
                green(self.color, "REPO name"),
                r.name
            )?;
            writeln!(self.output, "{}: {}", green(self.color, "REPO URL"), r.url)?;
        }
        writeln!(self.output)?;
        Ok(())
    }

    fn list_known_products(&mut self, products: &[String]) -> io::Result<()> {
        // Python `list_known_products`: label green, names line plain.
        writeln!(
            self.output,
            "{}",
            green(self.color, "Products known by 'repose':")
        )?;
        writeln!(self.output, "{}", products.join(" "))?;
        writeln!(self.output)?;
        Ok(())
    }
}

pub struct JsonDisplay<W: Write> {
    pub output: W,
}

impl<W: Write> CommandDisplay for JsonDisplay<W> {
    fn list_products(&mut self, hostname: &str, port: u16, system: &System) -> io::Result<()> {
        // Newline-delimited JSON; key order and `", "`/`": "` separators match
        // Python `json.dumps` (see `product_json_line`).
        writeln!(
            self.output,
            "{}",
            product_json_line(hostname, port, "base", &system.base)
        )?;
        for a in sorted_addons(system) {
            writeln!(
                self.output,
                "{}",
                product_json_line(hostname, port, "addon", a)
            )?;
        }
        Ok(())
    }

    fn list_repos(&mut self, hostname: &str, port: u16, repos: &[Repository]) -> io::Result<()> {
        for r in repos {
            writeln!(self.output, "{}", repo_json_line(hostname, port, r))?;
        }
        Ok(())
    }

    fn list_known_products(&mut self, products: &[String]) -> io::Result<()> {
        for name in products {
            writeln!(self.output, "{}", known_product_json_line(name))?;
        }
        Ok(())
    }
}

/// Helper when only aliases from [`Repositories`] are needed later.
#[allow(dead_code)]
pub fn repo_slice(repos: &Repositories) -> Vec<String> {
    repos.keys().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::Buffer;
    use crate::types::Product;

    #[test]
    fn known_products_json() {
        let mut buf = Buffer::default();
        let mut d = JsonDisplay { output: &mut buf };
        d.list_known_products(&["SLES".into(), "QA".into()])
            .unwrap();
        let lines: Vec<_> = buf.0.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("known_product"));
        assert!(lines[0].contains("SLES"));
    }

    fn sample_system() -> System {
        System {
            base: Product {
                name: "SL-Micro".into(),
                version: "6.1".into(),
                arch: "x86_64".into(),
            },
            addons: vec![Product {
                name: "SL-Micro-Extras".into(),
                version: "6.1".into(),
                arch: "x86_64".into(),
            }],
            transactional: false,
        }
    }

    #[test]
    fn list_products_json_matches_python_json_dumps() {
        let mut buf = Buffer::default();
        let mut d = JsonDisplay { output: &mut buf };
        d.list_products("ulysse.qam.suse.cz", 22, &sample_system())
            .unwrap();
        assert_eq!(
            buf.0,
            "{\"event\": \"product\", \"host\": \"ulysse.qam.suse.cz\", \"port\": 22, \"kind\": \"base\", \"name\": \"SL-Micro\", \"version\": \"6.1\", \"arch\": \"x86_64\"}\n\
             {\"event\": \"product\", \"host\": \"ulysse.qam.suse.cz\", \"port\": 22, \"kind\": \"addon\", \"name\": \"SL-Micro-Extras\", \"version\": \"6.1\", \"arch\": \"x86_64\"}\n"
        );
    }

    #[test]
    fn repo_json_line_matches_python_json_dumps() {
        let r = crate::types::Repository {
            alias: "SLES:15-SP6::pool".into(),
            name: "SLES:15-SP6::pool".into(),
            url: "http://download.example.invalid/p/".into(),
            state: true,
        };
        assert_eq!(
            repo_json_line("dubai.qam.suse.cz", 22, &r),
            "{\"event\": \"repo\", \"host\": \"dubai.qam.suse.cz\", \"port\": 22, \
             \"alias\": \"SLES:15-SP6::pool\", \"name\": \"SLES:15-SP6::pool\", \
             \"url\": \"http://download.example.invalid/p/\", \"state\": true}"
        );
    }

    #[test]
    fn known_product_json_line_matches_python_json_dumps() {
        assert_eq!(
            known_product_json_line("SLES"),
            "{\"event\": \"known_product\", \"name\": \"SLES\"}"
        );
    }

    #[test]
    fn list_products_text_matches_python_pretty() {
        let mut buf = Buffer::default();
        let mut d = TextDisplay {
            output: &mut buf,
            color: false,
        };
        d.list_products("ulysse.qam.suse.cz", 22, &sample_system())
            .unwrap();
        // Python `f"      Addon: {x.name:<53} - version: {x.version}"`: name is
        // left-padded to column width 53 (15-char name -> 38 trailing spaces).
        let pad = " ".repeat(53 - "SL-Micro-Extras".len());
        let expected = format!(
            "Host: ulysse.qam.suse.cz:22\n  \
             Base product: SL-Micro-6.1-x86_64\n  \
             Installed Extensions and Modules:\n      \
             Addon: SL-Micro-Extras{pad} - version: 6.1\n\n"
        );
        assert_eq!(buf.0, expected);
    }

    #[test]
    fn color_helpers_match_python_utils_sequences() {
        // Byte-parity with Python 2.1.0 `utils.green/yellow/blue`
        // (`\033[1;3Nm{x}\033[1;m\033[0m`); plain passthrough when disabled.
        assert_eq!(green(true, "Host"), "\x1b[1;32mHost\x1b[1;m\x1b[0m");
        assert_eq!(yellow(true, "h1"), "\x1b[1;33mh1\x1b[1;m\x1b[0m");
        assert_eq!(blue(true, "h1"), "\x1b[1;34mh1\x1b[1;m\x1b[0m");
        assert_eq!(green(false, "Host"), "Host");
        assert_eq!(yellow(false, "h1"), "h1");
        assert_eq!(blue(false, "h1"), "h1");
    }

    #[test]
    fn list_products_text_colorized_header_matches_python() {
        let mut buf = Buffer::default();
        let mut d = TextDisplay {
            output: &mut buf,
            color: true,
        };
        d.list_products("ulysse.qam.suse.cz", 22, &sample_system())
            .unwrap();
        // Header: green `Host`, yellow hostname/port; pretty() lines plain.
        assert!(buf.0.starts_with(&format!(
            "{}: {}:{}\n",
            green(true, "Host"),
            yellow(true, "ulysse.qam.suse.cz"),
            yellow(true, "22"),
        )));
        assert!(buf.0.contains("  Base product: SL-Micro-6.1-x86_64\n"));
    }

    #[test]
    fn list_repos_text_colorized_matches_python() {
        let mut buf = Buffer::default();
        let mut d = TextDisplay {
            output: &mut buf,
            color: true,
        };
        let r = crate::types::Repository {
            alias: "a".into(),
            name: "SLES:pool".into(),
            url: "http://x/".into(),
            state: true,
        };
        d.list_repos("dubai", 22, std::slice::from_ref(&r)).unwrap();
        let expected = format!(
            "{} on {}:{}\n{}: SLES:pool\n{}: http://x/\n\n",
            green(true, "Repositories"),
            blue(true, "dubai"),
            blue(true, "22"),
            green(true, "REPO name"),
            green(true, "REPO URL"),
        );
        assert_eq!(buf.0, expected);
    }

    #[test]
    fn known_products_text_colorized_label_matches_python() {
        let mut buf = Buffer::default();
        let mut d = TextDisplay {
            output: &mut buf,
            color: true,
        };
        d.list_known_products(&["SLES".into(), "QA".into()])
            .unwrap();
        let expected = format!(
            "{}\nSLES QA\n\n",
            green(true, "Products known by 'repose':")
        );
        assert_eq!(buf.0, expected);
    }

    #[test]
    fn js_matches_python_json_dumps_ensure_ascii() {
        // Ground truth from python3 json.dumps (ensure_ascii=True default):
        //   json.dumps("Qualität")  -> "Qualit\u00e4t"   (lowercase hex)
        //   json.dumps("café 😀")   -> "caf\u00e9 \ud83d\ude00"  (surrogates)
        assert_eq!(js("Qualität"), r#""Qualit\u00e4t""#);
        assert_eq!(js("café 😀"), r#""caf\u00e9 \ud83d\ude00""#);
        // ASCII controls / quote / backslash: serde_json already matches
        // json.dumps('tab\tq"b\\s\x01') byte-for-byte.
        assert_eq!(js("tab\tq\"b\\s\u{1}"), r#""tab\tq\"b\\s\u0001""#);
        assert_eq!(js("plain"), "\"plain\"");
    }

    #[test]
    fn yaml_string_quotes_exactly_ruamel_classes() {
        // Both lists verified against ruamel.yaml YAML(typ='safe') dumping
        // {'k': s} (see review evidence): quoted = empty string, int-like,
        // float-like, YAML 1.2 core bool/null spellings, ': ' / ' #'.
        for s in [
            "", "0", "2", "22", "08", "+1", "-1", "6.1", ".5", "1.", "1e3", "true", "True", "TRUE",
            "false", "False", "FALSE", "null", "Null", "NULL", "~", "a: b", "a #c",
        ] {
            assert_eq!(yaml_string(s), format!("'{s}'"), "{s:?} must be quoted");
        }
        // Plain: the YAML 1.1-only bool spellings and ordinary SUSE shapes.
        for s in [
            "yes",
            "no",
            "on",
            "off",
            "Yes",
            "No",
            "On",
            "Off",
            "ALL",
            "SP3",
            "15-SP3",
            "3.19.1",
            "SLES",
            "tumbleweed",
            "a b",
            "x:y",
            "ulysse.qam.suse.cz",
        ] {
            assert_eq!(yaml_string(s), s, "{s:?} must stay plain");
        }
    }

    #[test]
    fn yaml_empty_version_is_quoted_empty_scalar() {
        // transform_version_partialy("") passes the empty string through
        // unchanged; ruamel renders it `version: ''` — NOT `version: ` with a
        // trailing space.
        let mut s = String::new();
        push_version(&mut s, &Value::String(String::new()), "  ");
        assert_eq!(s, "  version: ''\n");
    }

    #[test]
    fn list_products_yaml_json_matches_python_json_dumps() {
        // Ground truth derived by running the display.py:121-127 payload
        // ({"event": "host_spec", "host": h, **to_refhost_dict_partially_
        // normalized(), "name": h}) through python3 json.dumps for the
        // sample_system fixture.
        let mut buf = Buffer::default();
        list_products_yaml_json(&mut buf, "ulysse.qam.suse.cz", &sample_system()).unwrap();
        assert_eq!(
            buf.0,
            "{\"event\": \"host_spec\", \"host\": \"ulysse.qam.suse.cz\", \
             \"location\": [\"some location\"], \"arch\": \"x86_64\", \
             \"product\": {\"name\": \"SL-Micro\", \"version\": {\"major\": 6, \"minor\": 1}}, \
             \"addons\": [{\"name\": \"SL-Micro-Extras\", \"version\": {\"major\": 6, \"minor\": 1}}], \
             \"name\": \"ulysse.qam.suse.cz\"}\n"
        );
    }

    #[test]
    fn list_products_yaml_json_unnormalized_version_and_no_addons() {
        // python3: json.dumps of the same payload for a tumbleweed host with
        // no addons — version stays a bare string, addons is [].
        let sys = System {
            base: Product {
                name: "openSUSE Tumbleweed".into(),
                version: "tumbleweed".into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional: false,
        };
        let mut buf = Buffer::default();
        list_products_yaml_json(&mut buf, "h2", &sys).unwrap();
        assert_eq!(
            buf.0,
            "{\"event\": \"host_spec\", \"host\": \"h2\", \"location\": [\"some location\"], \
             \"arch\": \"x86_64\", \"product\": {\"name\": \"openSUSE Tumbleweed\", \
             \"version\": \"tumbleweed\"}, \"addons\": [], \"name\": \"h2\"}\n"
        );
    }

    #[test]
    fn list_products_yaml_matches_ruamel() {
        let mut buf = Buffer::default();
        list_products_yaml(&mut buf, "ulysse.qam.suse.cz", &sample_system()).unwrap();
        let expected = concat!(
            "---\n",
            "addons:\n",
            "- name: SL-Micro-Extras\n",
            "  version:\n",
            "    major: 6\n",
            "    minor: 1\n",
            "arch: x86_64\n",
            "location:\n",
            "- some location\n",
            "name: ulysse.qam.suse.cz\n",
            "product:\n",
            "  name: SL-Micro\n",
            "  version:\n",
            "    major: 6\n",
            "    minor: 1\n",
            "...\n",
        );
        assert_eq!(buf.0, expected);
    }
}
