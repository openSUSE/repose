//! `repose uninstall` — strip repos + remove products.

use crate::commands::remove::{calculate_patterns, calculate_repolist};
use crate::commands::{aggregate, report_target, CommandOptions};
use crate::console::Console;
use crate::repa::Repa;
use crate::shell::cmd;
use crate::traits::{Host, HostGroup};
use crate::types::ExitCode;
use std::io::Write;

pub async fn run_uninstall<W: Write>(
    opts: &CommandOptions,
    group: &mut dyn HostGroup,
    console: &mut Console<W>,
) -> ExitCode {
    // Force repo=None on REPAs (Python dataclasses.replace).
    let orepa: Vec<Repa> = opts
        .repa
        .iter()
        .map(|r| Repa::from_parts(r.product.clone(), r.version.clone(), r.arch.clone(), None))
        .collect();

    group.connect_and_prune().await;
    group.read_repos().await;
    group.parse_repos().await;

    let keys = group.keys();
    let mut results = Vec::new();
    for key in keys {
        let Some(host) = group.get_mut(&key) else {
            results.push(false);
            continue;
        };
        results.push(uninstall_one(opts, host, &orepa, console).await);
    }
    group.close().await;
    aggregate(results)
}

async fn uninstall_one<W: Write>(
    opts: &CommandOptions,
    host: &mut dyn Host,
    orepa: &[Repa],
    console: &mut Console<W>,
) -> bool {
    let products = host.products().map(|s| s.flatten()).unwrap_or_default();
    let transactional = host
        .products()
        .map(|s| s.is_transactional())
        .unwrap_or(false);
    let patterns = calculate_patterns(orepa, &products);
    if patterns.is_empty() {
        return true;
    }

    let aliases = host
        .repos()
        .map(|r| r.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    // Uninstall uses substring match like remove with :: stripped patterns —
    // patterns already end with :: when repo is None.
    let repolist = calculate_repolist(aliases.into_iter(), &patterns);

    let product_names: Vec<String> = patterns
        .iter()
        .map(|p| p.split(':').next().unwrap_or(p).to_string())
        .collect();
    // Dedup preserve order
    let mut seen = std::collections::BTreeSet::new();
    let product_names: Vec<String> = product_names
        .into_iter()
        .filter(|n| seen.insert(n.clone()))
        .collect();
    let name_refs: Vec<&str> = product_names.iter().map(String::as_str).collect();
    let pdcmd = if transactional {
        cmd::transactional_rm_products(&name_refs)
    } else {
        cmd::zypper_rm_products(&name_refs)
    };

    if opts.dry {
        if !repolist.is_empty() {
            let refs: Vec<&str> = repolist.iter().map(String::as_str).collect();
            let _ = console.dry(host.key(), &cmd::zypper_rr(&refs));
        }
        let _ = console.dry(host.key(), &pdcmd);
        if transactional && !opts.no_reboot {
            let _ = console.dry(host.key(), cmd::REBOOT);
        }
        return true;
    }

    let mut ok = true;
    if !repolist.is_empty() {
        let refs: Vec<&str> = repolist.iter().map(String::as_str).collect();
        let rr = cmd::zypper_rr(&refs);
        if host.run(&rr).await.is_ok() {
            if !report_target(host) {
                ok = false;
            }
        } else {
            ok = false;
        }
    }
    if host.run(&pdcmd).await.is_ok() {
        if !report_target(host) {
            ok = false;
        } else if transactional && !opts.no_reboot {
            match host.reboot(cmd::REBOOT).await {
                Ok(true) => {
                    let _ = host.read_products().await;
                    if let Some(sys) = host.products() {
                        let flat = sys.flatten();
                        for n in &product_names {
                            if flat.iter().any(|p| &p.name == n) {
                                ok = false; // still present
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
