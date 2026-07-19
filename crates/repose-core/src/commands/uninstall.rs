//! `repose uninstall` — strip repos + remove products.

use crate::commands::remove::{calculate_patterns, calculate_repolist};
use crate::commands::{aggregate, reboot_and_verify, run_reported, CommandOptions};
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
        let _ = console.info(&format!("For {} no products for remove found", host.key()));
        return true;
    }

    // Skip the sentinel aliases whose repo name is not a 4-part product string
    // (Python `_calculate_repodict` skips entries where `product.name is None`).
    let aliases = host
        .repos()
        .map(|r| {
            r.iter()
                .filter(|(_, product)| product.is_some())
                .map(|(alias, _)| alias.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // Patterns already end with `::` (repo forced None) → substring match.
    let repolist = calculate_repolist(aliases.into_iter(), &patterns);
    if repolist.is_empty() {
        let _ = console.info(&format!("For {} no repos for remove found", host.key()));
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
        if !run_reported(host, &rr, console).await {
            ok = false;
        }
    }
    // Short-circuit: a failed product-removal report skips the reboot+verify
    // (Python `elif transactional`).
    if !run_reported(host, &pdcmd, console).await
        || (transactional
            && !reboot_and_verify(host, &product_names, false, opts.no_reboot, console).await)
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
}
