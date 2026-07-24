//! `repose uninstall` — strip repos + remove products.

use crate::commands::remove::{calculate_patterns, calculate_repolist};
use crate::commands::{
    CommandOptions, SharedConsole, aggregate, reboot_and_verify_shared, report_pruned,
    run_reported_shared,
};
use crate::console::Console;
use crate::repa::Repa;
use crate::shell::cmd;
use crate::traits::{Host, HostGroup};
use crate::types::ExitCode;
use futures_util::future::join_all;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::Semaphore;

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

    let pruned = group.connect_and_prune().await;
    group.read_repos().await;
    group.parse_repos().await;

    // Fan out per-host work concurrently (Python spawned one worker task per
    // target); `join_all` preserves key order for exit aggregation. Bounded
    // by a semaphore (P1 step 18) — see `add.rs`'s `run_add` for why this
    // avoids both the head-of-line blocking `.buffered(cap)` would cause
    // and the index/sort step `buffer_unordered` would need.
    let cap = group.host_operation_limit().get();
    let semaphore = Arc::new(Semaphore::new(cap));
    let console = SharedConsole::new(console);
    let mut results = join_all(group.hosts_mut().into_iter().map(|host| {
        let semaphore = Arc::clone(&semaphore);
        let orepa = &orepa;
        let console = &console;
        async move {
            let _permit = semaphore
                .acquire()
                .await
                .expect("host-operation semaphore is never closed");
            uninstall_one(opts, host, orepa, console).await
        }
    }))
    .await;
    report_pruned(&pruned, &mut results, &console);
    group.close().await;
    aggregate(results)
}

async fn uninstall_one<W: Write>(
    opts: &CommandOptions,
    host: &mut dyn Host,
    orepa: &[Repa],
    console: &SharedConsole<'_, W>,
) -> bool {
    // Python dereferences `products.flatten()` / `.is_transactional()`
    // unguarded — a host whose product read failed raises in the worker
    // and counts as failed.
    let Some(system) = host.products() else {
        return false;
    };
    let products = system.flatten();
    let transactional = system.is_transactional();
    let patterns = calculate_patterns(orepa, &products);
    if patterns.is_empty() {
        console.info(&format!("For {} no products for remove found", host.key()));
        return true;
    }

    // Skip the sentinel aliases whose repo name is not a 4-part product string
    // (Python `_calculate_repodict` skips entries where `product.name is None`).
    // A failed repo read leaves `repos` as `None`: removing the products
    // while dropping zero repos would leave stale repos behind.
    let Some(repos) = host.repos() else {
        console.error(host.key(), "could not read repositories");
        return false;
    };
    let aliases = repos
        .iter()
        .filter(|(_, product)| product.is_some())
        .map(|(alias, _)| alias.clone())
        .collect::<Vec<_>>();
    // Patterns already end with `::` (repo forced None) → substring match.
    let repolist = calculate_repolist(aliases.into_iter(), &patterns);
    if repolist.is_empty() {
        console.info(&format!("For {} no repos for remove found", host.key()));
    }

    // Duplicates are intentional: Python `[x.split(":")[0] for x in patterns]`
    // passes duplicate product names straight to `shlex.join` (two patterns
    // sharing a product name → `rm -t product SLES SLES`). Do not dedup.
    let product_names: Vec<String> = patterns
        .iter()
        .map(|p| p.split(':').next().unwrap_or(p).to_string())
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
            console.dry(host.key(), &cmd::zypper_rr(&refs));
        }
        console.dry(host.key(), &pdcmd);
        if transactional && !opts.no_reboot {
            console.dry(host.key(), cmd::REBOOT);
        }
        return true;
    }

    let mut ok = true;
    if !repolist.is_empty() {
        let refs: Vec<&str> = repolist.iter().map(String::as_str).collect();
        let rr = cmd::zypper_rr(&refs);
        if !run_reported_shared(host, &rr, console).await {
            ok = false;
        }
    }
    // Short-circuit: a failed product-removal report skips the reboot+verify
    // (Python `elif transactional`).
    if !run_reported_shared(host, &pdcmd, console).await
        || (transactional
            && !reboot_and_verify_shared(host, &product_names, false, opts.no_reboot, console)
                .await)
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
    use crate::mock::{MockHost, MockHostGroup};
    use crate::types::{Product, Repositories, System};
    use std::path::PathBuf;

    fn product(name: &str, version: &str) -> Product {
        Product {
            name: name.into(),
            version: version.into(),
            arch: "x86_64".into(),
        }
    }

    fn system(base: Product, addons: Vec<Product>, transactional: bool) -> System {
        System {
            base,
            addons,
            transactional,
        }
    }

    fn repos(entries: &[(&str, Option<Product>)]) -> Repositories {
        let mut r = Repositories::new();
        for (alias, p) in entries {
            r.insert((*alias).into(), p.clone());
        }
        r
    }

    fn opts(repa: &[&str], dry: bool, no_reboot: bool) -> CommandOptions {
        CommandOptions {
            config: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/vectors/template/sample.yml"),
            repa: repa.iter().map(|r| Repa::parse(r).unwrap()).collect(),
            dry,
            no_reboot,
            ..Default::default()
        }
    }

    async fn run(host: MockHost, opts: CommandOptions) -> (ExitCode, Vec<String>, String) {
        let mut g = MockHostGroup::new();
        g.insert(host);
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        let code = run_uninstall(&opts, &mut g, &mut c).await;
        let ran = g
            .get_mock_mut("h1")
            .map(|h| h.ran.clone())
            .unwrap_or_default();
        (code, ran, buf.0)
    }

    #[tokio::test]
    async fn uninstall_reports_and_counts_pruned_hosts() {
        let mut g = MockHostGroup::new();
        let mut bad = MockHost::new("bad");
        bad.fail_connect();
        g.insert(bad);
        let sles = product("SLES", "15-SP4");
        g.insert(MockHost::new("h1").with_products(system(sles, vec![], false)));
        // "OTHER" matches no product → patterns empty → the surviving host
        // is a successful no-op; only the pruned host fails.
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        let code = run_uninstall(&opts(&["OTHER:1.0"], false, false), &mut g, &mut c).await;
        assert_eq!(code, ExitCode::Partial);
        assert!(
            buf.0.contains("connect failed:") && buf.0.contains("bad"),
            "pruned host must be reported, got: {:?}",
            buf.0
        );
    }

    #[tokio::test]
    async fn uninstall_failed_repo_read_fails_the_host() {
        let sles = product("SLES", "15-SP4");
        let host = MockHost::new("h1")
            .with_products(system(sles, vec![], false))
            .with_read_repos_err();
        let (code, ran, buf) = run(host, opts(&["SLES:15-SP4"], false, false)).await;
        assert_eq!(code, ExitCode::AllFailed);
        assert!(buf.contains("could not read repositories"));
        // No product removal may run after a failed repo read.
        assert!(
            !ran.iter().any(|c| c.contains("rm -t product")),
            "ran: {ran:?}"
        );
    }

    #[tokio::test]
    async fn basic_two_repos() {
        let c = seq::case("uninstall", "basic_two_repos");
        let sles = product("SLES", "15-SP4");
        let host = MockHost::new("h1")
            .with_products(system(sles.clone(), vec![], false))
            .with_repos(repos(&[
                ("SLES:15-SP4::repo1", Some(sles.clone())),
                ("SLES:15-SP4::repo2", Some(sles.clone())),
                ("other:repo", Some(product("other", "1"))),
            ]));
        let (code, ran, _) = run(host, opts(&["SLES:15-SP4"], false, false)).await;
        assert_eq!(ran, c.ran);
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn dry_basic() {
        let c = seq::case("uninstall", "dry_basic");
        let sles = product("SLES", "15-SP4");
        let host = MockHost::new("h1")
            .with_products(system(sles.clone(), vec![], false))
            .with_repos(repos(&[("SLES:15-SP4::repo1", Some(sles.clone()))]));
        let (code, ran, buf) = run(host, opts(&["SLES:15-SP4"], true, false)).await;
        assert!(ran.is_empty());
        assert_eq!(buf, c.dry_buffer("h1"));
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn sentinel_skip() {
        // The `(None)` sentinel alias must be excluded from the rr command.
        let c = seq::case("uninstall", "sentinel_skip");
        let sles = product("SLES", "15-SP4");
        let host = MockHost::new("h1")
            .with_products(system(sles.clone(), vec![], false))
            .with_repos(repos(&[
                ("SLES:15-SP4::repo1", Some(sles.clone())),
                ("SLES:15-SP4::weird", None),
            ]));
        let (code, ran, _) = run(host, opts(&["SLES:15-SP4"], false, false)).await;
        assert_eq!(ran, c.ran);
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn duplicate_product_names() {
        // Two patterns sharing a product name keep the duplicate in the argv.
        let c = seq::case("uninstall", "duplicate_product_names");
        let base = product("SLES", "15-SP4");
        let addon = product("SLES", "15-SP5");
        let host = MockHost::new("h1")
            .with_products(system(base.clone(), vec![addon], false))
            .with_repos(repos(&[("SLES:15-SP4::repo1", Some(base.clone()))]));
        let (code, ran, _) = run(host, opts(&["SLES"], false, false)).await;
        assert_eq!(ran, c.ran);
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn no_matching_repos_only_pd() {
        let c = seq::case("uninstall", "no_matching_repos_only_pd");
        let sles = product("SLES", "15-SP4");
        let host = MockHost::new("h1")
            .with_products(system(sles.clone(), vec![], false))
            .with_repos(Repositories::new());
        let (code, ran, _) = run(host, opts(&["SLES:15-SP4"], false, false)).await;
        assert_eq!(ran, c.ran);
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn transactional_verify_fails() {
        let c = seq::case("uninstall", "transactional_verify_fails");
        let qa = product("qa", "6.0");
        let host = MockHost::new("h1")
            .with_products(system(qa.clone(), vec![], true))
            .with_repos(repos(&[("qa:6.0::repo1", Some(qa.clone()))]));
        let (code, ran, _) = run(host, opts(&["qa:6.0"], false, false)).await;
        assert_eq!(ran, c.ran);
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn transactional_verify_passes() {
        // Post-reboot products no longer contain the removed product → verify passes.
        let c = seq::case("uninstall", "transactional_verify_passes");
        let qa = product("qa", "6.0");
        let host = MockHost::new("h1")
            .with_products(system(qa.clone(), vec![], true))
            .with_repos(repos(&[("qa:6.0::repo1", Some(qa.clone()))]))
            .with_post_reboot_products(system(product("other", "1"), vec![], true));
        let (code, ran, buf) = run(host, opts(&["qa:6.0"], false, false)).await;
        assert_eq!(ran, c.ran);
        // Python `_check_products` reports the successful verify.
        assert!(buf.contains("h1: verified product(s) qa after reboot"));
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn no_products_fails_host() {
        // Python dereferences `products.flatten()` → worker raises → host
        // failed; unreadable product state must NOT count as success.
        let host = MockHost::new("h1").with_repos(Repositories::new());
        let (code, _, _) = run(host, opts(&["SLES:15-SP4"], false, false)).await;
        assert_eq!(code, ExitCode::AllFailed);
    }

    #[tokio::test]
    async fn transactional_reread_failure_after_reboot_fails() {
        // Python `_reboot_and_verify` catches the re-read exception and
        // fails the host; ignoring the error must not report success.
        let qa = product("qa", "6.0");
        let host = MockHost::new("h1")
            .with_products(system(qa.clone(), vec![], true))
            .with_repos(repos(&[("qa:6.0::repo1", Some(qa.clone()))]))
            .with_read_products_err();
        let (code, ran, buf) = run(host, opts(&["qa:6.0"], false, false)).await;
        assert!(ran.iter().any(|c| c == cmd::REBOOT));
        assert!(buf.contains("could not re-read products after reboot"));
        assert_eq!(code, ExitCode::AllFailed);
    }

    #[tokio::test]
    async fn dry_transactional() {
        let c = seq::case("uninstall", "dry_transactional");
        let qa = product("qa", "6.0");
        let host = MockHost::new("h1")
            .with_products(system(qa.clone(), vec![], true))
            .with_repos(repos(&[("qa:6.0::repo1", Some(qa.clone()))]));
        let (code, ran, buf) = run(host, opts(&["qa:6.0"], true, false)).await;
        assert!(ran.is_empty());
        assert_eq!(buf, c.dry_buffer("h1"));
        assert_eq!(code, c.exit_code());
    }

    /// P1 step 18: a configured host-operation limit below the host count
    /// bounds the per-host worker semaphore, while every host still
    /// uninstalls exactly once.
    #[tokio::test]
    async fn bounded_uninstall_never_exceeds_the_configured_host_operation_limit() {
        use crate::mock::{MockGate, MockMetrics, MockOpKind};
        use std::num::NonZeroUsize;

        let metrics = MockMetrics::new();
        let gate = MockGate::new();
        const LIMIT: usize = 2;
        const HOSTS: usize = 5;
        let sles = product("SLES", "15-SP4");
        let mut g =
            MockHostGroup::new().with_host_operation_limit(NonZeroUsize::new(LIMIT).unwrap());
        for i in 0..HOSTS {
            let h = MockHost::new(format!("h{i}"))
                .with_products(system(sles.clone(), vec![], false))
                .with_repos(repos(&[("SLES:15-SP4::repo1", Some(sles.clone()))]))
                .with_metrics(metrics.clone())
                .with_gate(MockOpKind::Run, gate.clone());
            g.insert(h);
        }
        let opts = opts(&["SLES:15-SP4"], false, false);
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);

        let driver = async {
            let mut saw_limit = false;
            for _ in 0..2_000 {
                let current = metrics.snapshot().current_operations;
                assert!(current <= LIMIT, "admitted {current}, exceeding {LIMIT}");
                if current == LIMIT {
                    saw_limit = true;
                }
                tokio::task::yield_now().await;
            }
            assert!(saw_limit, "never observed the fleet saturate the limit");
            gate.release();
        };
        let (code, ()) = tokio::join!(run_uninstall(&opts, &mut g, &mut c), driver);
        assert_eq!(code, ExitCode::Ok);

        for i in 0..HOSTS {
            let ran = g.get_mock_mut(&format!("h{i}")).unwrap().ran.clone();
            assert!(
                ran.iter().any(|cmd| cmd.starts_with("zypper -n rm")),
                "h{i} must have uninstalled exactly once, ran: {ran:?}"
            );
        }
    }
}
