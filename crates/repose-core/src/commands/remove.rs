//! `repose remove` — pattern match aliases, zypper rr.

use crate::commands::{aggregate, run_reported_shared, CommandOptions, SharedConsole};
use crate::console::Console;
use crate::repa::Repa;
use crate::shell::cmd;
use crate::traits::{Host, HostGroup};
use crate::types::{ExitCode, Product};
use futures_util::future::join_all;
use std::collections::BTreeSet;
use std::io::Write;

/// Patterns `product:version::` or `product:version::repo`.
pub(crate) fn calculate_patterns(repas: &[Repa], products: &[Product]) -> BTreeSet<String> {
    let mut patterns = BTreeSet::new();
    for repa in repas {
        for prd in products {
            let product = match &repa.product {
                Some(p) if p == &prd.name => p.clone(),
                Some(_) => continue,
                None => prd.name.clone(),
            };
            let version = match &repa.version {
                Some(v) if v == &prd.version => v.clone(),
                Some(_) => continue,
                None => prd.version.clone(),
            };
            let repo = repa.repo.clone().unwrap_or_default();
            patterns.insert(format!("{product}:{version}::{repo}"));
        }
    }
    patterns
}

/// Exact alias match, or substring if pattern ends with `::`.
pub(crate) fn calculate_repolist(
    aliases: impl Iterator<Item = String>,
    patterns: &BTreeSet<String>,
) -> BTreeSet<String> {
    let aliases: Vec<String> = aliases.collect();
    let mut repolist = BTreeSet::new();
    for pattern in patterns {
        let all_repos = pattern.ends_with("::");
        for repo in &aliases {
            let matched = if all_repos {
                repo.contains(pattern.as_str())
            } else {
                repo == pattern
            };
            if matched {
                repolist.insert(repo.clone());
            }
        }
    }
    repolist
}

pub async fn run_remove<W: Write>(
    opts: &CommandOptions,
    group: &mut dyn HostGroup,
    console: &mut Console<W>,
) -> ExitCode {
    group.connect_and_prune().await;
    group.read_repos().await;
    group.parse_repos().await;

    // Fan out per-host work concurrently (Python spawned one worker task per
    // target); `join_all` preserves key order for exit aggregation.
    let console = SharedConsole::new(console);
    let results = join_all(
        group
            .hosts_mut()
            .into_iter()
            .map(|host| remove_one(opts, host, &console)),
    )
    .await;

    group.close().await;
    aggregate(results)
}

async fn remove_one<W: Write>(
    opts: &CommandOptions,
    host: &mut dyn Host,
    console: &SharedConsole<'_, W>,
) -> bool {
    // Python dereferences `products.flatten()` unguarded — a host whose
    // product read failed raises in the worker and counts as failed.
    let Some(system) = host.products() else {
        return false;
    };
    let products = system.flatten();
    let patterns = calculate_patterns(&opts.repa, &products);
    if patterns.is_empty() {
        console.info(&format!("For {} no repos for remove found", host.key()));
        return true;
    }
    let aliases = host
        .repos()
        .map(|r| r.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let repolist = calculate_repolist(aliases.into_iter(), &patterns);
    if repolist.is_empty() {
        console.info(&format!("For {} no repos for remove found", host.key()));
        return true;
    }
    let refs: Vec<&str> = repolist.iter().map(String::as_str).collect();
    let c = cmd::zypper_rr(&refs);
    if opts.dry {
        console.dry(host.key(), &c);
        return true;
    }
    run_reported_shared(host, &c, console).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::Buffer;
    use crate::mock::{MockHost, MockHostGroup};
    use crate::types::{Product, System};

    async fn run(host: MockHost, repas: &[&str]) -> (ExitCode, String) {
        let mut g = MockHostGroup::new();
        g.insert(host);
        let opts = CommandOptions {
            repa: repas.iter().map(|r| Repa::parse(r).unwrap()).collect(),
            ..Default::default()
        };
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        let code = run_remove(&opts, &mut g, &mut c).await;
        (code, buf.0)
    }

    #[tokio::test]
    async fn no_products_fails_host() {
        // Python dereferences `products.flatten()` → worker raises → host
        // failed; unreadable product state must NOT count as success.
        let (code, _) = run(MockHost::new("h1"), &["SLES:15-SP3"]).await;
        assert_eq!(code, ExitCode::AllFailed);
    }

    #[tokio::test]
    async fn no_matching_repos_is_info_noop() {
        let host = MockHost::new("h1").with_products(System {
            base: Product {
                name: "SLES".into(),
                version: "15-SP3".into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional: false,
        });
        let (code, buf) = run(host, &["OTHER:1.0"]).await;
        assert_eq!(code, ExitCode::Ok);
        assert!(buf.contains("For h1 no repos for remove found"));
    }

    #[test]
    fn exact_not_prefix_repo10() {
        let patterns: BTreeSet<_> = ["SLES:15-SP3::repo1".into()].into();
        let list = calculate_repolist(
            ["SLES:15-SP3::repo1".into(), "SLES:15-SP3::repo10".into()].into_iter(),
            &patterns,
        );
        assert_eq!(list.len(), 1);
        assert!(list.contains("SLES:15-SP3::repo1"));
    }

    #[test]
    fn substring_when_double_colon() {
        let patterns: BTreeSet<_> = ["SLES:15-SP3::".into()].into();
        let list = calculate_repolist(
            ["SLES:15-SP3::update".into(), "other".into()].into_iter(),
            &patterns,
        );
        assert!(list.contains("SLES:15-SP3::update"));
        assert!(!list.contains("other"));
    }

    #[test]
    fn patterns_from_repa() {
        let products = [Product {
            name: "SLES".into(),
            version: "15-SP3".into(),
            arch: "x86_64".into(),
        }];
        let repas = [Repa::parse("SLES").unwrap()];
        let p = calculate_patterns(&repas, &products);
        assert!(p.iter().any(|x| x == "SLES:15-SP3::"));
    }

    #[test]
    fn matches_vector_repolist() {
        let raw = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/vectors/remove_match/repolist.json"),
        )
        .expect("vector remove_match/repolist.json");
        for case in serde_json::from_str::<Vec<serde_json::Value>>(&raw).unwrap() {
            let name = case["name"].as_str().unwrap();
            let patterns: BTreeSet<String> = case["patterns"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            let aliases: Vec<String> = case["aliases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            let expected: Vec<String> = case["expected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            let got: Vec<String> = calculate_repolist(aliases.into_iter(), &patterns)
                .into_iter()
                .collect();
            assert_eq!(got, expected, "case {name}");
        }
    }
}
