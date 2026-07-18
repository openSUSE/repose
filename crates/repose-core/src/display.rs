//! list-* / known-products display (Python `CommandDisplay` / `JsonCommandDisplay`).

use std::io::{self, Write};

use serde_json::json;

use crate::types::{Repositories, Repository, System};

/// YAML refhost-spec output for `list-products --yaml` (Python
/// `list_products_yaml` → `System.to_refhost_dict_partially_normalized`).
/// Versions run through `transform_version_partialy` (so a numeric version
/// serializes as a YAML number). Structurally faithful to the Python dict;
/// ruamel's exact byte formatting (`---`/`...`, indent) is not matched.
pub fn list_products_yaml<W: Write>(
    out: &mut W,
    hostname: &str,
    system: &System,
) -> io::Result<()> {
    use crate::transform::transform_version_partialy;

    #[derive(serde::Serialize)]
    struct Prod {
        name: String,
        version: serde_json::Value,
    }
    #[derive(serde::Serialize)]
    struct Spec {
        location: Vec<&'static str>,
        arch: String,
        product: Prod,
        addons: Vec<Prod>,
        name: String,
    }

    let base = system.get_base();
    let spec = Spec {
        location: vec!["some location"],
        arch: system.arch().to_string(),
        product: Prod {
            name: base.name.clone(),
            version: transform_version_partialy(&base.version),
        },
        addons: system
            .get_addons()
            .iter()
            .map(|a| Prod {
                name: a.name.clone(),
                version: transform_version_partialy(&a.version),
            })
            .collect(),
        name: hostname.to_string(),
    };
    let yaml = serde_yaml::to_string(&spec).map_err(io::Error::other)?;
    out.write_all(yaml.as_bytes())
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
        writeln!(self.output, "Host: {hostname}:{port}")?;
        writeln!(
            self.output,
            "  {} {} {}",
            system.base.name, system.base.version, system.base.arch
        )?;
        for a in &system.addons {
            writeln!(self.output, "  + {} {} {}", a.name, a.version, a.arch)?;
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
        let base = &system.base;
        writeln!(
            self.output,
            "{}",
            json!({
                "event": "product",
                "host": hostname,
                "port": port,
                "kind": "base",
                "name": base.name,
                "version": base.version,
                "arch": base.arch,
            })
        )?;
        for a in &system.addons {
            writeln!(
                self.output,
                "{}",
                json!({
                    "event": "product",
                    "host": hostname,
                    "port": port,
                    "kind": "addon",
                    "name": a.name,
                    "version": a.version,
                    "arch": a.arch,
                })
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

    #[test]
    fn list_products_json() {
        let mut buf = Buffer::default();
        let mut d = JsonDisplay { output: &mut buf };
        let sys = System {
            base: Product {
                name: "SLES".into(),
                version: "15-SP6".into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional: false,
        };
        d.list_products("h", 22, &sys).unwrap();
        assert!(buf.0.contains("\"kind\":\"base\""));
    }
}
