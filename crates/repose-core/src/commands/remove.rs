//! `repose remove` — pattern match aliases, zypper rr.

use crate::commands::{aggregate, report_target, CommandOptions};
use crate::console::Console;
use crate::repa::Repa;
use crate::shell::cmd;
use crate::traits::HostGroup;
use crate::types::{ExitCode, Product};
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

    let keys = group.keys();
    let mut results = Vec::new();
    for key in keys {
        let Some(host) = group.get_mut(&key) else {
            results.push(false);
            continue;
        };
        let products = host.products().map(|s| s.flatten()).unwrap_or_default();
        let patterns = calculate_patterns(&opts.repa, &products);
        if patterns.is_empty() {
            results.push(true);
            continue;
        }
        let aliases = host
            .repos()
            .map(|r| r.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let repolist = calculate_repolist(aliases.into_iter(), &patterns);
        if repolist.is_empty() {
            results.push(true);
            continue;
        }
        let refs: Vec<&str> = repolist.iter().map(String::as_str).collect();
        let c = cmd::zypper_rr(&refs);
        if opts.dry {
            let _ = console.dry(host.key(), &c);
            results.push(true);
            continue;
        }
        let ok = match host.run(&c).await {
            Ok(()) => report_target(host),
            Err(_) => false,
        };
        results.push(ok);
    }

    group.close().await;
    aggregate(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Product;

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
    fn matches_oracle_repolist() {
        let raw = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/oracle/remove_match/repolist.json"),
        )
        .expect("oracle remove_match/repolist.json");
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
