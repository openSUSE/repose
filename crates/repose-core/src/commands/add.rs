//! `repose add` — resolve REPA, probe, zypper ar, cohort refresh.

use crate::commands::{aggregate, filter_live, load_repoq, report_target, CommandOptions};
use crate::console::Console;
use crate::shell::cmd;
use crate::traits::{Host, HostGroup, Probe};
use crate::types::ExitCode;
use std::io::Write;

pub async fn run_add<W: Write>(
    opts: &CommandOptions,
    group: &mut dyn HostGroup,
    probe: &dyn Probe,
    console: &mut Console<W>,
) -> Result<ExitCode, crate::template::TemplateError> {
    let repoq = load_repoq(&opts.config)?;
    group.connect_and_prune().await;
    group.read_products().await;

    let keys = group.keys();
    let mut results = Vec::new();
    for key in keys {
        let Some(host) = group.get_mut(&key) else {
            results.push(false);
            continue;
        };
        results.push(add_one(opts, host, probe, &repoq, console).await);
    }

    if !opts.dry {
        let keys = group.keys();
        for key in keys {
            if let Some(h) = group.get_mut(&key) {
                let _ = h.run(cmd::REFCMD).await;
            }
        }
    }
    group.close().await;
    Ok(aggregate(results))
}

async fn add_one<W: Write>(
    opts: &CommandOptions,
    host: &mut dyn Host,
    probe: &dyn Probe,
    repoq: &crate::repoq::Repoq,
    console: &mut Console<W>,
) -> bool {
    let Some(products) = host.products() else {
        return false;
    };
    let base = products.get_base().clone();
    let mut ok = true;
    let mut candidates = Vec::new();
    for repa in &opts.repa {
        match repoq.solve_repa(repa, &base) {
            Ok(map) => {
                for list in map.into_values() {
                    candidates.extend(list);
                }
            }
            Err(e) => {
                let _ = console.error(host.key(), &e.to_string());
                ok = false;
            }
        }
    }
    let live = filter_live(probe, candidates, opts.probe_timeout, opts.no_probe).await;
    let mut cmds: Vec<String> = live
        .iter()
        .map(|r| cmd::zypper_ar(r.refresh, &r.name, &r.url))
        .collect();
    cmds.sort();
    for c in cmds {
        if opts.dry {
            let _ = console.dry(host.key(), &c);
        } else if host.run(&c).await.is_ok() {
            if !report_target(host) {
                ok = false;
            }
        } else {
            ok = false;
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::{Buffer, OutputFormat};
    use crate::mock::{ConstProbe, MockHost, MockHostGroup};
    use crate::repa::Repa;
    use crate::traits::Host;
    use crate::types::{Product, System};
    use std::path::PathBuf;

    fn sample_config() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/oracle/template/sample.yml")
    }

    #[tokio::test]
    async fn dry_add_prints_ar() {
        let mut g = MockHostGroup::new();
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h = h.with_products(System {
            base: Product {
                name: "SLES".into(),
                version: "15-SP3".into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional: false,
        });
        g.insert(h);
        let opts = CommandOptions {
            dry: true,
            config: sample_config(),
            repa: vec![Repa::parse("SLES:15-SP3:x86_64:update").unwrap()],
            no_probe: true,
            ..Default::default()
        };
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        c.format = OutputFormat::Text;
        let probe = ConstProbe { live: true };
        let code = run_add(&opts, &mut g, &probe, &mut c).await.unwrap();
        assert_eq!(code, ExitCode::Ok);
        assert!(buf.0.contains("zypper") && buf.0.contains("ar"));
    }
}
