//! list-products, list-repos, known-products.

use crate::commands::CommandOptions;
use crate::display::{CommandDisplay, JsonDisplay, TextDisplay};
use crate::template::load_template;
use crate::traits::HostGroup;
use crate::types::ExitCode;
use std::io::Write;
use std::path::Path;

pub async fn run_list_products<W: Write>(
    opts: &CommandOptions,
    group: &mut dyn HostGroup,
    out: &mut W,
) -> ExitCode {
    group.connect_and_prune().await;
    group.read_products().await;
    let keys = group.keys();
    for key in keys {
        if let Some(host) = group.get(&key) {
            // Port from key if present
            let (hostname, port) = split_key(host.key());
            if let Some(sys) = host.products() {
                if opts.yaml {
                    // Python `--yaml` honors `--format`: YAML documents for
                    // text, per-host `host_spec` NDJSON for json
                    // (`JsonCommandDisplay.list_products_yaml`).
                    match opts.format {
                        crate::console::OutputFormat::Json => {
                            let _ =
                                crate::display::list_products_yaml_json(&mut *out, hostname, sys);
                        }
                        crate::console::OutputFormat::Text => {
                            let _ = crate::display::list_products_yaml(&mut *out, hostname, sys);
                        }
                    }
                } else {
                    match opts.format {
                        crate::console::OutputFormat::Json => {
                            let mut d = JsonDisplay { output: &mut *out };
                            let _ = d.list_products(hostname, port, sys);
                        }
                        crate::console::OutputFormat::Text => {
                            let mut d = TextDisplay { output: &mut *out };
                            let _ = d.list_products(hostname, port, sys);
                        }
                    }
                }
            }
        }
    }
    group.close().await;
    ExitCode::Ok
}

pub async fn run_list_repos<W: Write>(
    opts: &CommandOptions,
    group: &mut dyn HostGroup,
    out: &mut W,
) -> ExitCode {
    group.connect_and_prune().await;
    group.read_repos().await;
    let keys = group.keys();
    for key in keys {
        if let Some(host) = group.get(&key) {
            let (hostname, port) = split_key(host.key());
            let empty = vec![];
            let repos = host.raw_repos().unwrap_or(&empty);
            match opts.format {
                crate::console::OutputFormat::Json => {
                    let mut d = JsonDisplay { output: &mut *out };
                    let _ = d.list_repos(hostname, port, repos);
                }
                crate::console::OutputFormat::Text => {
                    let mut d = TextDisplay { output: &mut *out };
                    let _ = d.list_repos(hostname, port, repos);
                }
            }
        }
    }
    group.close().await;
    ExitCode::Ok // always 0 even if some hosts failed earlier
}

pub fn run_known_products(
    config: &Path,
    format: crate::console::OutputFormat,
    out: &mut impl Write,
) -> Result<ExitCode, crate::template::TemplateError> {
    let tpl = load_template(config)?;
    let mut names: Vec<String> = tpl
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    names.sort();
    match format {
        crate::console::OutputFormat::Json => {
            let mut d = JsonDisplay { output: out };
            let _ = d.list_known_products(&names);
        }
        crate::console::OutputFormat::Text => {
            let mut d = TextDisplay { output: out };
            let _ = d.list_known_products(&names);
        }
    }
    Ok(ExitCode::Ok)
}

fn split_key(key: &str) -> (&str, u16) {
    if let Some((h, p)) = key.rsplit_once(':') {
        if let Ok(port) = p.parse() {
            return (h, port);
        }
    }
    (key, 22)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::Buffer;

    #[test]
    fn known_products_sorted() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/vectors/template/sample.yml");
        let mut buf = Buffer::default();
        run_known_products(&path, crate::console::OutputFormat::Text, &mut buf).unwrap();
        assert!(buf.0.contains("QA") && buf.0.contains("SLES"));
    }

    #[tokio::test]
    async fn list_products_yaml_emits_refhost_spec() {
        use crate::mock::{MockHost, MockHostGroup};
        use crate::types::{Product, System};
        let mut g = MockHostGroup::new();
        g.insert(MockHost::new("h1").with_products(System {
            base: Product {
                name: "SLES".into(),
                version: "15-SP3".into(),
                arch: "x86_64".into(),
            },
            addons: vec![Product {
                name: "sle-module-basesystem".into(),
                version: "15-SP3".into(),
                arch: "x86_64".into(),
            }],
            transactional: false,
        }));
        let opts = CommandOptions {
            yaml: true,
            ..Default::default()
        };
        let mut buf = Buffer::default();
        run_list_products(&opts, &mut g, &mut buf).await;
        assert!(buf.0.contains("location"), "{}", buf.0);
        assert!(buf.0.contains("arch: x86_64"), "{}", buf.0);
        assert!(buf.0.contains("sle-module-basesystem"), "{}", buf.0);
        assert!(buf.0.contains("name: h1"), "{}", buf.0);
    }

    #[tokio::test]
    async fn list_products_yaml_with_json_format_emits_host_spec_ndjson() {
        // Python `repose list-products --yaml --format json` emits one
        // `host_spec` JSON document per host (display.py:121-127), NOT raw
        // YAML. The expected line below is the byte-exact python3 ground
        // truth: json.dumps({"event": "host_spec", "host": "h1",
        // **system.to_refhost_dict_partially_normalized(), "name": "h1"})
        // for base SLES 15-SP3 x86_64 + addon sle-module-basesystem 15-SP3.
        use crate::mock::{MockHost, MockHostGroup};
        use crate::types::{Product, System};
        let mut g = MockHostGroup::new();
        g.insert(MockHost::new("h1").with_products(System {
            base: Product {
                name: "SLES".into(),
                version: "15-SP3".into(),
                arch: "x86_64".into(),
            },
            addons: vec![Product {
                name: "sle-module-basesystem".into(),
                version: "15-SP3".into(),
                arch: "x86_64".into(),
            }],
            transactional: false,
        }));
        let opts = CommandOptions {
            yaml: true,
            format: crate::console::OutputFormat::Json,
            ..Default::default()
        };
        let mut buf = Buffer::default();
        run_list_products(&opts, &mut g, &mut buf).await;
        assert_eq!(
            buf.0,
            "{\"event\": \"host_spec\", \"host\": \"h1\", \"location\": [\"some location\"], \
             \"arch\": \"x86_64\", \"product\": {\"name\": \"SLES\", \
             \"version\": {\"major\": 15, \"minor\": \"SP3\"}}, \
             \"addons\": [{\"name\": \"sle-module-basesystem\", \
             \"version\": {\"major\": 15, \"minor\": \"SP3\"}}], \"name\": \"h1\"}\n"
        );
    }
}
