//! list-* / known-products display (Python `CommandDisplay` / `JsonCommandDisplay`).

use std::io::{self, Write};

use serde_json::{json, Value};

use crate::types::{Product, Repositories, Repository, System};

/// Addons in a deterministic order (by name, then version, then arch).
///
/// Python stores addons in a `frozenset`, so its iteration order is
/// randomized per process (PYTHONHASHSEED) and is NOT reproducible. We emit a
/// stable sorted order instead; every field except the addon *ordering* is
/// byte-identical to the Python oracle.
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

/// One JSON scalar, using `serde_json` so escaping/quoting matches `json.dumps`.
fn js(s: &str) -> String {
    serde_json::to_string(s).expect("string always serializes")
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

/// Render one YAML scalar (plain style) from a `transform_version_partialy`
/// leaf: numbers unquoted, strings as plain scalars — matching ruamel's safe
/// dumper for the SUSE version shapes seen in real `.prod` files.
fn yaml_scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
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
/// The addon list order is sorted (see [`sorted_addons`]); Python's is a
/// non-reproducible `frozenset` order.
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
            s.push_str(&format!("- name: {}\n", a.name));
            push_version(&mut s, &transform_version_partialy(&a.version), "  ");
        }
    }
    s.push_str(&format!("arch: {}\n", system.arch()));
    s.push_str("location:\n- some location\n");
    s.push_str(&format!("name: {hostname}\n"));
    s.push_str("product:\n");
    s.push_str(&format!("  name: {}\n", base.name));
    push_version(&mut s, &transform_version_partialy(&base.version), "  ");
    s.push_str("...\n");
    out.write_all(s.as_bytes())
}

pub trait CommandDisplay {
    fn list_products(&mut self, hostname: &str, port: u16, system: &System) -> io::Result<()>;
    fn list_repos(&mut self, hostname: &str, port: u16, repos: &[Repository]) -> io::Result<()>;
    fn list_known_products(&mut self, products: &[String]) -> io::Result<()>;
}

pub struct TextDisplay<W: Write> {
    pub output: W,
}

impl<W: Write> CommandDisplay for TextDisplay<W> {
    fn list_products(&mut self, hostname: &str, port: u16, system: &System) -> io::Result<()> {
        // Mirrors Python `CommandDisplay.list_products` + `System.pretty`
        // (color disabled when stdout is not a TTY, as here).
        let base = &system.base;
        writeln!(self.output, "Host: {hostname}:{port}")?;
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
        writeln!(self.output, "Repositories on {hostname}:{port}")?;
        for r in repos {
            writeln!(self.output, "REPO name: {}", r.name)?;
            writeln!(self.output, "REPO URL: {}", r.url)?;
        }
        writeln!(self.output)?;
        Ok(())
    }

    fn list_known_products(&mut self, products: &[String]) -> io::Result<()> {
        writeln!(self.output, "Products known by 'repose':")?;
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
            writeln!(
                self.output,
                "{}",
                json!({
                    "event": "repo",
                    "host": hostname,
                    "port": port,
                    "alias": r.alias,
                    "name": r.name,
                    "url": r.url,
                    "state": r.state,
                })
            )?;
        }
        Ok(())
    }

    fn list_known_products(&mut self, products: &[String]) -> io::Result<()> {
        for name in products {
            writeln!(
                self.output,
                "{}",
                json!({"event": "known_product", "name": name})
            )?;
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
    fn list_products_text_matches_python_pretty() {
        let mut buf = Buffer::default();
        let mut d = TextDisplay { output: &mut buf };
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
