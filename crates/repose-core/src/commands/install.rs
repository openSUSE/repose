//! `repose install` — ar + ref + product install; transactional path.

use crate::commands::{
    aggregate, filter_live, load_repoq, reboot_and_verify, run_reported, CommandOptions,
};
use crate::console::Console;
use crate::repoq::Repos;
use crate::shell::cmd;
use crate::traits::{Host, HostGroup, Probe};
use crate::types::ExitCode;
use std::collections::BTreeMap;
use std::io::Write;

/// Product → repos accumulator preserving REPA/dict insertion order.
///
/// Python `install._merge_repos` mutates an insertion-ordered `dict`, and both
/// the product-install argv (`shlex.join(repositories.keys())`) and the
/// per-repo `ar` order (`chain.from_iterable(repositories.values())`) follow
/// that order. A sorted `BTreeMap` would diverge — design delta #4 permits a
/// stable sort only for *set*-sourced multi-alias commands, not this dict.
type ProductRepos = Vec<(String, Vec<Repos>)>;

fn merge_repos(acc: &mut ProductRepos, resolved: BTreeMap<String, Vec<Repos>>) {
    for (product, repos) in resolved {
        let idx = match acc.iter().position(|(p, _)| p == &product) {
            Some(i) => i,
            None => {
                acc.push((product, Vec::new()));
                acc.len() - 1
            }
        };
        let existing = &mut acc[idx].1;
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

    let mut repositories: ProductRepos = Vec::new();
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

    let all_repos: Vec<_> = repositories
        .iter()
        .flat_map(|(_, v)| v.iter())
        .cloned()
        .collect();
    let live = filter_live(probe, all_repos, opts.probe_timeout, opts.no_probe).await;

    for repo in &live {
        let addcmd = cmd::zypper_ar(repo.refresh, &repo.name, &repo.url);
        if opts.dry {
            let _ = console.dry(host.key(), &addcmd);
        } else {
            if !run_reported(host, &addcmd, console).await {
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

    // Product-install argv follows insertion order (Python dict order); see ProductRepos.
    let product_names: Vec<String> = repositories.iter().map(|(p, _)| p.clone()).collect();
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
    // Short-circuit: a failed install report skips the reboot+verify
    // (Python `elif transactional`).
    if !run_reported(host, &inscmd, console).await
        || (transactional
            && !reboot_and_verify(host, &product_names, true, opts.no_reboot, console).await)
    {
        ok = false;
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::seq;
    use crate::console::Buffer;
    use crate::mock::{ConstProbe, MockHost, MockHostGroup};
    use crate::repa::Repa;
    use crate::types::{Product, System};
    use std::path::PathBuf;

    fn sample_config() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/oracle/template/sample.yml")
    }

    fn system(name: &str, transactional: bool) -> System {
        System {
            base: Product {
                name: name.into(),
                version: "15-SP3".into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional,
        }
    }

    fn opts(repa: &[&str], dry: bool, no_reboot: bool) -> CommandOptions {
        CommandOptions {
            config: sample_config(),
            repa: repa.iter().map(|r| Repa::parse(r).unwrap()).collect(),
            dry,
            no_reboot,
            no_probe: false,
            ..Default::default()
        }
    }

    async fn run(
        host: MockHost,
        opts: CommandOptions,
        probe: &dyn Probe,
    ) -> (ExitCode, Vec<String>, String) {
        let mut g = MockHostGroup::new();
        g.insert(host);
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        let code = run_install(&opts, &mut g, probe, &mut c).await.unwrap();
        let ran = g
            .get_mock_mut("h1")
            .map(|h| h.ran.clone())
            .unwrap_or_default();
        (code, ran, buf.0)
    }

    #[tokio::test]
    async fn live_non_transactional() {
        let c = seq::case("install", "live_non_transactional");
        let host = MockHost::new("h1").with_products(system("SLES", false));
        let (code, ran, _) = run(
            host,
            opts(&["SLES:15-SP3:x86_64"], false, false),
            &ConstProbe { live: true },
        )
        .await;
        assert_eq!(ran, c.ran);
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn dry_non_transactional_ar_only_no_refcmd() {
        let c = seq::case("install", "dry_non_transactional");
        let host = MockHost::new("h1").with_products(system("SLES", false));
        let (code, ran, buf) = run(
            host,
            opts(&["SLES:15-SP3:x86_64"], true, false),
            &ConstProbe { live: true },
        )
        .await;
        assert!(ran.is_empty());
        assert_eq!(buf, c.dry_buffer("h1"));
        // Trap: per-repo refcmd has no dry line, and non-transactional has no reboot.
        assert!(!buf.contains("--gpg-auto-import-keys"));
        assert!(!buf.contains("systemctl reboot"));
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn product_insertion_order() {
        let c = seq::case("install", "product_insertion_order");
        let host = MockHost::new("h1").with_products(system("SLES", false));
        let (code, ran, _) = run(
            host,
            opts(
                &["SLES:15-SP3:x86_64:pool", "QA:15-SP3:x86_64"],
                false,
                false,
            ),
            &ConstProbe { live: true },
        )
        .await;
        assert_eq!(ran, c.ran);
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn all_dead_still_installs() {
        let c = seq::case("install", "all_dead_still_installs");
        let host = MockHost::new("h1").with_products(system("SLES", false));
        let (code, ran, _) = run(
            host,
            opts(&["SLES:15-SP3:x86_64:pool"], false, false),
            &ConstProbe { live: false },
        )
        .await;
        assert_eq!(ran, c.ran);
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn transactional_live() {
        let c = seq::case("install", "transactional_live");
        let host = MockHost::new("h1").with_products(system("SLES", true));
        let (code, ran, buf) = run(
            host,
            opts(&["SLES:15-SP3:x86_64:update"], false, false),
            &ConstProbe { live: true },
        )
        .await;
        assert_eq!(ran, c.ran);
        // Python `_check_products` reports the successful verify.
        assert!(buf.contains("h1: verified product(s) SLES after reboot"));
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn transactional_verify_fails_when_product_absent_after_reboot() {
        // Post-reboot products no longer contain the installed product → verify fails.
        let host = MockHost::new("h1")
            .with_products(system("SLES", true))
            .with_post_reboot_products(system("OTHER", true));
        let (code, ran, buf) = run(
            host,
            opts(&["SLES:15-SP3:x86_64:update"], false, false),
            &ConstProbe { live: true },
        )
        .await;
        assert_eq!(ran, seq::case("install", "transactional_live").ran);
        assert!(buf.contains("product SLES not installed after reboot"));
        assert_eq!(code, ExitCode::AllFailed);
    }

    #[tokio::test]
    async fn transactional_verify_fails_when_products_unreadable_after_reboot() {
        // Re-read succeeded but yielded no product state: Python's
        // `installed = set()` fails every present-check — must NOT pass.
        let host = MockHost::new("h1")
            .with_products(system("SLES", true))
            .with_post_reboot_no_products();
        let (code, ran, buf) = run(
            host,
            opts(&["SLES:15-SP3:x86_64:update"], false, false),
            &ConstProbe { live: true },
        )
        .await;
        assert_eq!(ran, seq::case("install", "transactional_live").ran);
        assert!(buf.contains("product SLES not installed after reboot"));
        assert_eq!(code, ExitCode::AllFailed);
    }

    #[tokio::test]
    async fn transactional_no_reboot_prints_reminder() {
        // --no-reboot leaves the snapshot staged: reminder, no reboot, success.
        let host = MockHost::new("h1").with_products(system("SLES", true));
        let (code, ran, buf) = run(
            host,
            opts(&["SLES:15-SP3:x86_64:update"], false, true),
            &ConstProbe { live: true },
        )
        .await;
        assert!(!ran.iter().any(|c| c == cmd::REBOOT));
        assert!(buf.contains("Reboot h1 to activate the staged snapshot (--no-reboot set)"));
        assert_eq!(code, ExitCode::Ok);
    }

    #[tokio::test]
    async fn dry_transactional() {
        let c = seq::case("install", "dry_transactional");
        let host = MockHost::new("h1").with_products(system("SLES", true));
        let (code, ran, buf) = run(
            host,
            opts(&["SLES:15-SP3:x86_64:update"], true, false),
            &ConstProbe { live: true },
        )
        .await;
        assert!(ran.is_empty());
        assert_eq!(buf, c.dry_buffer("h1"));
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn dry_transactional_no_reboot() {
        let c = seq::case("install", "dry_transactional_no_reboot");
        let host = MockHost::new("h1").with_products(system("SLES", true));
        let (code, ran, buf) = run(
            host,
            opts(&["SLES:15-SP3:x86_64:update"], true, true),
            &ConstProbe { live: true },
        )
        .await;
        assert!(ran.is_empty());
        assert_eq!(buf, c.dry_buffer("h1"));
        assert!(!buf.contains("systemctl reboot"));
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn no_products_fails() {
        let c = seq::case("install", "no_products");
        let host = MockHost::new("h1").with_products(system("SLES", false));
        let (code, ran, buf) = run(host, opts(&[], false, false), &ConstProbe { live: true }).await;
        assert!(ran.is_empty());
        assert!(buf.contains("No products to install"));
        assert_eq!(code, c.exit_code());
    }
}
