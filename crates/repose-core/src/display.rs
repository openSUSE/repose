//! list-* / known-products display: the text and NDJSON renderers.

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

/// ANSI color helpers used by [`TextDisplay`] to color `list-*` /
/// `known-products` labels and values. The emitted sequence is
/// `\x1b[1;3Nm{s}\x1b[1;m\x1b[0m`; the trailing double reset is part of
/// repose's established output, not an accident. Returns `s` unchanged when
/// `enabled` is false.
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

/// One JSON scalar, escaped so that an NDJSON line is always pure ASCII.
/// `serde_json` already emits the standard JSON escapes for ASCII controls,
/// `"` and `\`, but renders non-ASCII as raw UTF-8; this re-escapes it as
/// `\uXXXX` per UTF-16 code unit (lowercase hex, surrogate pairs for astral
/// chars): U+00E4 becomes `\u00e4`, U+1F600 becomes the surrogate pair
/// `\ud83d\ude00`.
fn js(s: &str) -> String {
    let quoted = serde_json::to_string(s).expect("string always serializes");
    if quoted.is_ascii() {
        return quoted;
    }
    // Escape sequences serde emits are pure ASCII, so every non-ASCII char
    // left in `quoted` is a literal one still needing a `\u` escape.
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

/// A single `list-products` JSON event line: `", "` / `": "` separators and
/// fixed key order (event, host, port, kind, name, version, arch).
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

/// A single `list-repos` JSON event line: `", "` / `": "` separators and
/// fixed key order (event, host, port, alias, name, url, state).
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

/// A single `known-products` JSON event line (key order: event, name).
fn known_product_json_line(name: &str) -> String {
    format!("{{\"event\": \"known_product\", \"name\": {}}}", js(name))
}

/// True when the string parses as a YAML 1.2 core int or float, so it has to
/// be emitted quoted to stay a string: `0`, `22`, `08`, `+1`, `-1`, `6.1`,
/// `.5`, `1.`, `1e3`. Plain SUSE shapes (`15-SP3`, `SP3`, `ALL`,
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

/// YAML c-indicators, which cannot open a plain scalar: flow collections,
/// anchors and aliases, tags, comments, block scalars, the directive and
/// reserved markers, and both quote characters. Deliberately excludes `-`,
/// `?` and `:`, which YAML 1.2 permits to open a plain scalar when the next
/// character is not a space — `sle-ha` and `15-SP3` must stay unquoted.
const YAML_INDICATORS: &[char] = &[
    '[', ']', '{', '}', ',', '&', '*', '#', '|', '>', '!', '%', '@', '`', '"', '\'',
];

/// One YAML string scalar, single-quoted exactly where leaving it plain would
/// stop it being a string: the empty string, int/float-like strings (`'0'`,
/// `'22'`, `'08'`, `'6.1'`), YAML 1.2 core booleans/null (`true`/`True`/`TRUE`,
/// `false`/..., `null`/`Null`/`NULL`, `~` — but NOT the YAML 1.1-only
/// `yes`/`no`/`on`/`off`, which stay plain), strings containing `": "` or
/// `" #"`, and anything opening with a YAML indicator character — a
/// bracketed IPv6 host name would otherwise be read back as a flow
/// sequence rather than a string. Everything else (`15-SP3`, `SP3`, `ALL`,
/// `SLES`, `tumbleweed`, hostnames) stays plain.
fn yaml_string(s: &str) -> String {
    let quote = s.is_empty()
        || is_numeric_like(s)
        || matches!(
            s,
            "true" | "True" | "TRUE" | "false" | "False" | "FALSE" | "null" | "Null" | "NULL" | "~"
        )
        || s.starts_with(YAML_INDICATORS)
        // A plain scalar loses its surrounding whitespace on the way back in,
        // so padding has to be quoted or it is silently dropped.
        || s.trim() != s
        || s.contains(": ")
        || s.contains(" #");
    if quote {
        format!("'{}'", s.replace('\'', "''"))
    } else {
        s.to_string()
    }
}

/// Render one YAML scalar (plain style) from a `transform_version_partialy`
/// leaf: numbers unquoted, strings via [`yaml_string`], which covers the SUSE
/// version shapes seen in real `.prod` files.
fn yaml_scalar(v: &Value) -> String {
    match v {
        Value::String(s) => yaml_string(s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// Emit the `version:` block (or inline scalar) for a normalized version at the
/// given `indent`, in the layout the refhost spec uses: keys +2, with
/// block-sequence dashes at the parent indent.
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

/// YAML refhost-spec output for `list-products --yaml` — the host spec the
/// `refhosts.yml` generator consumes.
///
/// Hand-rolled to emit exactly that shape: `---`/`...` document
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

/// A `transform_version_partialy` leaf as NDJSON: normalized versions become
/// `{"major": ..., "minor": ...}` (that key order, `", "`/`": "` separators),
/// unnormalized ones stay a bare scalar (string via [`js`], numbers verbatim).
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

/// NDJSON refhost-spec output for `list-products --yaml --format json`: one
/// `host_spec` document per host, carrying the same payload as the YAML
/// dumper. Key order is event, host, location, arch, product, addons, name —
/// the hostname deliberately appears twice, as both `host` and `name` — with
/// the usual `", "` / `": "` separators and [`js`] escaping. The addon list
/// order is sorted (see `sorted_addons`).
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
    /// Emit ANSI color.
    pub color: bool,
}

impl<W: Write> CommandDisplay for TextDisplay<W> {
    fn list_products(&mut self, hostname: &str, port: u16, system: &System) -> io::Result<()> {
        // `Host` green, hostname/port yellow; the product lines stay plain.
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
                // Addon names are left-padded to 53 columns so the versions
                // line up; longer names push their version right.
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
        // `Repositories` green, host/port blue; per repo the `REPO name` /
        // `REPO URL` labels green, values plain.
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
        // Label green, names line plain.
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
        // Newline-delimited JSON; key order and `", "`/`": "` separators are
        // fixed by `product_json_line`.
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
    fn list_products_json_line_shape_is_pinned() {
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
    fn repo_json_line_shape_is_pinned() {
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
    fn known_product_json_line_shape_is_pinned() {
        assert_eq!(
            known_product_json_line("SLES"),
            "{\"event\": \"known_product\", \"name\": \"SLES\"}"
        );
    }

    #[test]
    fn list_products_text_pads_addon_names_to_column_53() {
        let mut buf = Buffer::default();
        let mut d = TextDisplay {
            output: &mut buf,
            color: false,
        };
        d.list_products("ulysse.qam.suse.cz", 22, &sample_system())
            .unwrap();
        // Addon name left-padded to column width 53 (15-char name -> 38
        // trailing spaces).
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
    fn color_helpers_emit_the_pinned_sequences() {
        // `\033[1;3Nm{x}\033[1;m\033[0m`, double reset included; plain
        // passthrough when disabled.
        assert_eq!(green(true, "Host"), "\x1b[1;32mHost\x1b[1;m\x1b[0m");
        assert_eq!(yellow(true, "h1"), "\x1b[1;33mh1\x1b[1;m\x1b[0m");
        assert_eq!(blue(true, "h1"), "\x1b[1;34mh1\x1b[1;m\x1b[0m");
        assert_eq!(green(false, "Host"), "Host");
        assert_eq!(yellow(false, "h1"), "h1");
        assert_eq!(blue(false, "h1"), "h1");
    }

    #[test]
    fn list_products_text_colorizes_only_the_header() {
        let mut buf = Buffer::default();
        let mut d = TextDisplay {
            output: &mut buf,
            color: true,
        };
        d.list_products("ulysse.qam.suse.cz", 22, &sample_system())
            .unwrap();
        // Header: green `Host`, yellow hostname/port; product lines plain.
        assert!(buf.0.starts_with(&format!(
            "{}: {}:{}\n",
            green(true, "Host"),
            yellow(true, "ulysse.qam.suse.cz"),
            yellow(true, "22"),
        )));
        assert!(buf.0.contains("  Base product: SL-Micro-6.1-x86_64\n"));
    }

    #[test]
    fn list_repos_text_colorizes_labels_not_values() {
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
    fn known_products_text_colorizes_the_label() {
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
    fn js_escapes_non_ascii_as_utf16_escapes() {
        // Non-ASCII is escaped per UTF-16 code unit in lowercase hex,
        // astral chars as a surrogate pair, so a line stays pure ASCII.
        assert_eq!(js("Qualität"), r#""Qualit\u00e4t""#);
        assert_eq!(js("café 😀"), r#""caf\u00e9 \ud83d\ude00""#);
        // ASCII controls, the quote and the backslash keep serde_json's
        // standard JSON escaping; this function leaves them untouched.
        assert_eq!(js("tab\tq\"b\\s\u{1}"), r#""tab\tq\"b\\s\u0001""#);
        assert_eq!(js("plain"), "\"plain\"");
    }

    #[test]
    fn yaml_string_quotes_exactly_the_ambiguous_classes() {
        // Quoted: the empty string, int-like, float-like, the YAML 1.2 core
        // bool/null spellings, and anything containing ': ' or ' #'.
        for s in [
            "", "0", "2", "22", "08", "+1", "-1", "6.1", ".5", "1.", "1e3", "true", "True", "TRUE",
            "false", "False", "FALSE", "null", "Null", "NULL", "~", "a: b", "a #c",
        ] {
            assert_eq!(yaml_string(s), format!("'{s}'"), "{s:?} must be quoted");
        }
        // A bracketed IPv6 host name must be quoted, or reading the document
        // back yields a one-element flow sequence instead of a string.
        assert_eq!(yaml_string("[::1]"), "'[::1]'");
        assert_eq!(yaml_string("[2001:db8::1]:2222"), "'[2001:db8::1]:2222'");
        // Padding survives only if quoted; a plain scalar is trimmed on read.
        assert_eq!(yaml_string(" pad"), "' pad'");
        assert_eq!(yaml_string("pad "), "'pad '");
        // `-`/`?`/`:` may open a plain scalar when not followed by a space.
        assert_eq!(yaml_string("-SP3"), "-SP3");
        assert_eq!(yaml_string("sle-ha"), "sle-ha");
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
        // unchanged; it must render as `version: ''` — NOT `version: ` with a
        // trailing space.
        let mut s = String::new();
        push_version(&mut s, &Value::String(String::new()), "  ");
        assert_eq!(s, "  version: ''\n");
    }

    #[test]
    fn list_products_yaml_json_shape_is_pinned() {
        // Key order and separators for the `host_spec` document, with the
        // hostname repeated as both `host` and `name`.
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
        // Same document for a tumbleweed host with no addons — the version
        // stays a bare string and `addons` is `[]`.
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
    fn list_products_yaml_shape_is_pinned() {
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
