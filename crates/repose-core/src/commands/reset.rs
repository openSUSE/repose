//! `repose reset` — rr then ar only if full live replacement set.

use crate::commands::{aggregate, filter_live, load_repoq, run_reported, CommandOptions};
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
            // Python `_clear` collects into a set: sorted + unique.
            a.sort();
            a.dedup();
            a
        })
        .unwrap_or_default();
    if aliases.is_empty() {
        let _ = console.info(&format!("No repositories to clear from {}", host.key()));
    }

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
    let mut dropped: Vec<_> = candidates
        .iter()
        .filter(|r| !live_keys.contains(&(r.name.clone(), r.url.clone())))
        .map(|r| r.name.clone())
        .collect();
    // Python reports `", ".join(sorted(dropped))`.
    dropped.sort();

    let mut cmds: Vec<String> = live
        .iter()
        .map(|r| cmd::zypper_ar(r.refresh, &r.name, &r.url))
        .collect();
    cmds.sort();
    // Python `_add` collects into a set; dedup identical ar strings so a repo
    // key listed twice in `default_repos` is not added (and run) twice.
    cmds.dedup();

    // Guards BEFORE dry-run (wording mirrors Python `reset._run`).
    if cmds.is_empty() {
        let _ = console.error(
            host.key(),
            "no live replacement repositories resolved; aborting reset to \
             avoid leaving the host without any repositories",
        );
        return false;
    }
    if !dropped.is_empty() {
        let _ = console.error(
            host.key(),
            &format!(
                "live-URL probe dropped {} of the resolved replacement \
                 repositories ({}); aborting reset to avoid permanently \
                 losing repositories over a transient mirror failure",
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
        if !run_reported(host, &rr, console).await {
            ok = false;
        }
    }
    for c in cmds {
        if !run_reported(host, &c, console).await {
            ok = false;
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::seq;
    use crate::console::Buffer;
    use crate::mock::{ConstProbe, MapProbe, MockHost, MockHostGroup};
    use crate::types::{Product, Repository, System};
    use std::path::PathBuf;

    /// URL that `solve_product` derives for the SLES `pool` repo from sample.yml.
    const POOL_URL: &str = "http://example.com/15-SP3/x86_64/pool/";

    fn sample_config() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/vectors/template/sample.yml")
    }

    fn sles_system() -> System {
        System {
            base: Product {
                name: "SLES".into(),
                version: "15-SP3".into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional: false,
        }
    }

    fn raw_repo(alias: &str) -> Repository {
        Repository {
            alias: alias.into(),
            name: "n".into(),
            url: "http://x".into(),
            state: true,
        }
    }

    fn opts(dry: bool) -> CommandOptions {
        CommandOptions {
            config: sample_config(),
            dry,
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
        let code = run_reset(&opts, &mut g, probe, &mut c).await.unwrap();
        let ran = g
            .get_mock_mut("h1")
            .map(|h| h.ran.clone())
            .unwrap_or_default();
        (code, ran, buf.0)
    }

    #[tokio::test]
    async fn live_success() {
        let c = seq::case("reset", "live_success");
        let host = MockHost::new("h1")
            .with_products(sles_system())
            .with_raw_repos(vec![raw_repo("existing-repo2"), raw_repo("existing-repo1")]);
        let (code, ran, _) = run(host, opts(false), &ConstProbe { live: true }).await;
        assert_eq!(ran, c.ran);
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn dry_success() {
        let c = seq::case("reset", "dry_success");
        let host = MockHost::new("h1")
            .with_products(sles_system())
            .with_raw_repos(vec![raw_repo("existing-repo2"), raw_repo("existing-repo1")]);
        let (code, ran, buf) = run(host, opts(true), &ConstProbe { live: true }).await;
        assert!(ran.is_empty());
        assert_eq!(buf, c.dry_buffer("h1"));
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn empty_aliases_skip_rr() {
        let c = seq::case("reset", "empty_aliases_skip_rr");
        let host = MockHost::new("h1").with_products(sles_system());
        let (code, ran, buf) = run(host, opts(false), &ConstProbe { live: true }).await;
        assert_eq!(ran, c.ran);
        assert!(!ran.iter().any(|cmd| cmd.starts_with("zypper -n rr")));
        // Python logs the INFO no-op for the skipped rr step.
        assert!(buf.contains("No repositories to clear from h1"));
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn abort_no_live() {
        let c = seq::case("reset", "abort_no_live");
        let host = MockHost::new("h1")
            .with_products(sles_system())
            .with_raw_repos(vec![raw_repo("existing-repo1")]);
        let (code, ran, buf) = run(host, opts(false), &ConstProbe { live: false }).await;
        assert!(ran.is_empty());
        assert!(buf.contains("no live"));
        assert!(!buf.contains("zypper"));
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn abort_partial_drop() {
        let c = seq::case("reset", "abort_partial_drop");
        let host = MockHost::new("h1")
            .with_products(sles_system())
            .with_raw_repos(vec![raw_repo("existing-repo1")]);
        let (code, ran, buf) = run(host, opts(false), &MapProbe::dead([POOL_URL])).await;
        assert!(ran.is_empty());
        assert!(buf.contains("probe dropped"));
        assert!(buf.contains("SLES:15-SP3::pool"));
        assert!(!buf.contains("zypper -n ar"));
        assert!(!buf.contains("zypper -n rr"));
        assert_eq!(code, c.exit_code());
    }

    #[tokio::test]
    async fn abort_partial_drop_dry() {
        // Guards run before the dry preview, so a dropped probe emits no dry lines.
        let c = seq::case("reset", "abort_partial_drop_dry");
        let host = MockHost::new("h1")
            .with_products(sles_system())
            .with_raw_repos(vec![raw_repo("existing-repo1")]);
        let (code, ran, buf) = run(host, opts(true), &MapProbe::dead([POOL_URL])).await;
        assert!(ran.is_empty());
        assert!(buf.contains("probe dropped"));
        assert!(!buf.contains("zypper -n ar"));
        assert!(!buf.contains("zypper -n rr"));
        assert_eq!(code, c.exit_code());
    }
}
