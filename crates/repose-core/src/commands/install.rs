//! `repose install` — ar + ref + product install; transactional path.

use crate::commands::{aggregate, filter_live, load_repoq, report_target, CommandOptions};
use crate::console::Console;
use crate::repoq::Repos;
use crate::shell::cmd;
use crate::traits::{Host, HostGroup, Probe};
use crate::types::ExitCode;
use std::collections::BTreeMap;
use std::io::Write;

fn merge_repos(acc: &mut BTreeMap<String, Vec<Repos>>, resolved: BTreeMap<String, Vec<Repos>>) {
    for (product, repos) in resolved {
        let existing = acc.entry(product).or_default();
        for repo in repos {
            if !existing.iter().any(|e| e == &repo) {
                existing.push(repo);
            }
        }
    }
}

pub async fn run_install<W: Write>(
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
        results.push(install_one(opts, host, probe, &repoq, console).await);
    }
    group.close().await;
    Ok(aggregate(results))
}

async fn install_one<W: Write>(
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
    let transactional = products.is_transactional();

    let mut repositories: BTreeMap<String, Vec<Repos>> = BTreeMap::new();
    let mut ok = true;
    for repa in &opts.repa {
        match repoq.solve_repa(repa, &base) {
            Ok(m) => merge_repos(&mut repositories, m),
            Err(e) => {
                let _ = console.error(host.key(), &e.to_string());
                ok = false;
            }
        }
    }

    let all_repos: Vec<_> = repositories.values().flatten().cloned().collect();
    let live = filter_live(probe, all_repos, opts.probe_timeout, opts.no_probe).await;

    for repo in &live {
        let addcmd = cmd::zypper_ar(repo.refresh, &repo.name, &repo.url);
        if opts.dry {
            let _ = console.dry(host.key(), &addcmd);
        } else {
            if host.run(&addcmd).await.is_ok() {
                if !report_target(host) {
                    ok = false;
                }
            } else {
                ok = false;
            }
            // Per-repo ref without report_target (Python parity trap).
            let _ = host.run(cmd::REFCMD).await;
        }
    }

    if repositories.is_empty() {
        let _ = console.error(host.key(), "No products to install");
        return false;
    }

    // Preserve insertion order of product keys (BTreeMap is sorted — intentional
    // stable order; Python uses dict insertion — design allows stable sort delta).
    let product_names: Vec<String> = repositories.keys().cloned().collect();
    let name_refs: Vec<&str> = product_names.iter().map(String::as_str).collect();
    let inscmd = if transactional {
        cmd::transactional_in_products(&name_refs)
    } else {
        cmd::zypper_in_products(&name_refs)
    };

    if opts.dry {
        if transactional {
            let _ = console.dry(host.key(), cmd::REFTCMD);
        }
        let _ = console.dry(host.key(), &inscmd);
        if transactional && !opts.no_reboot {
            let _ = console.dry(host.key(), cmd::REBOOT);
        }
        return ok;
    }

    if transactional {
        let _ = host.run(cmd::REFTCMD).await;
    }
    if host.run(&inscmd).await.is_ok() {
        if !report_target(host) {
            ok = false;
        } else if transactional && !opts.no_reboot {
            match host.reboot(cmd::REBOOT).await {
                Ok(true) => {
                    // verify present — products should still list names
                    if host.read_products().await.is_err() {
                        ok = false;
                    } else if let Some(sys) = host.products() {
                        let flat = sys.flatten();
                        for n in &product_names {
                            if !flat.iter().any(|p| &p.name == n) {
                                ok = false;
                            }
                        }
                    }
                }
                _ => ok = false,
            }
        }
    } else {
        ok = false;
    }
    ok
}
