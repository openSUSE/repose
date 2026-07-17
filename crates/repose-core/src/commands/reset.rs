//! `repose reset` — rr then ar only if full live replacement set.

use crate::commands::{aggregate, filter_live, load_repoq, report_target, CommandOptions};
use crate::console::Console;
use crate::shell::cmd;
use crate::traits::{Host, HostGroup, Probe};
use crate::types::ExitCode;
use std::io::Write;

pub async fn run_reset<W: Write>(
    opts: &CommandOptions,
    group: &mut dyn HostGroup,
    probe: &dyn Probe,
    console: &mut Console<W>,
) -> Result<ExitCode, crate::template::TemplateError> {
    let repoq = load_repoq(&opts.config)?;
    group.connect_and_prune().await;
    group.read_products().await;
    group.read_repos().await;

    let keys = group.keys();
    let mut results = Vec::new();
    for key in keys {
        let Some(host) = group.get_mut(&key) else {
            results.push(false);
            continue;
        };
        results.push(reset_one(opts, host, probe, &repoq, console).await);
    }
    group.close().await;
    Ok(aggregate(results))
}

async fn reset_one<W: Write>(
    opts: &CommandOptions,
    host: &mut dyn Host,
    probe: &dyn Probe,
    repoq: &crate::repoq::Repoq,
    console: &mut Console<W>,
) -> bool {
    let aliases: Vec<String> = host
        .raw_repos()
        .map(|r| {
            let mut a: Vec<_> = r.iter().map(|x| x.alias.clone()).collect();
            a.sort();
            a
        })
        .unwrap_or_default();

    let Some(products) = host.products() else {
        let _ = console.error(host.key(), "no products discovered");
        return false;
    };

    let resolved = match repoq.solve_product(products) {
        Ok(m) => m,
        Err(e) => {
            let _ = console.error(host.key(), &e.to_string());
            return false;
        }
    };
    let candidates: Vec<_> = resolved.into_values().flatten().collect();
    let live = filter_live(probe, candidates.clone(), opts.probe_timeout, opts.no_probe).await;

    // dropped = candidates not in live (by name+url identity)
    let live_keys: std::collections::BTreeSet<_> = live
        .iter()
        .map(|r| (r.name.clone(), r.url.clone()))
        .collect();
    let dropped: Vec<_> = candidates
        .iter()
        .filter(|r| !live_keys.contains(&(r.name.clone(), r.url.clone())))
        .map(|r| r.name.clone())
        .collect();

    let mut cmds: Vec<String> = live
        .iter()
        .map(|r| cmd::zypper_ar(r.refresh, &r.name, &r.url))
        .collect();
    cmds.sort();

    // Guards BEFORE dry-run.
    if cmds.is_empty() {
        let _ = console.error(
            host.key(),
            "no live replacement repositories; aborting reset",
        );
        return false;
    }
    if !dropped.is_empty() {
        let _ = console.error(
            host.key(),
            &format!(
                "probe dropped {} replacement repos ({}); aborting reset",
                dropped.len(),
                dropped.join(", ")
            ),
        );
        return false;
    }

    if opts.dry {
        if !aliases.is_empty() {
            let refs: Vec<&str> = aliases.iter().map(String::as_str).collect();
            let _ = console.dry(host.key(), &cmd::zypper_rr(&refs));
        }
        for c in &cmds {
            let _ = console.dry(host.key(), c);
        }
        return true;
    }

    let mut ok = true;
    if !aliases.is_empty() {
        let refs: Vec<&str> = aliases.iter().map(String::as_str).collect();
        let rr = cmd::zypper_rr(&refs);
        if host.run(&rr).await.is_ok() {
            if !report_target(host) {
                ok = false;
            }
        } else {
            ok = false;
        }
    }
    for c in cmds {
        if host.run(&c).await.is_ok() {
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
    use crate::console::Buffer;
    use crate::mock::{ConstProbe, MockHost, MockHostGroup};
    use crate::traits::Host;
    use crate::types::{Product, Repository, System};
    use std::path::PathBuf;

    #[tokio::test]
    async fn abort_when_probe_drops() {
        let mut g = MockHostGroup::new();
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h = h
            .with_products(System {
                base: Product {
                    name: "SLES".into(),
                    version: "15-SP3".into(),
                    arch: "x86_64".into(),
                },
                addons: vec![],
                transactional: false,
            })
            .with_raw_repos(vec![Repository {
                alias: "old".into(),
                name: "n".into(),
                url: "http://x".into(),
                state: true,
            }]);
        g.insert(h);
        let opts = CommandOptions {
            config: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/oracle/template/sample.yml"),
            dry: true,
            no_probe: false,
            ..Default::default()
        };
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        // All dead → empty cmds → abort
        let probe = ConstProbe { live: false };
        let code = run_reset(&opts, &mut g, &probe, &mut c).await.unwrap();
        assert_eq!(code, ExitCode::AllFailed);
        assert!(buf.0.contains("abort") || buf.0.contains("no live"));
    }
}
