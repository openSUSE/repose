//! `repose reset` — rr then ar only if full live replacement set.

use crate::commands::{
    CommandOptions, ProbeBudget, SharedConsole, aggregate, load_repoq, partition_live,
    run_reported_shared,
};
use crate::console::Console;
use crate::shell::cmd;
use crate::traits::{Host, HostGroup, Probe};
use crate::types::ExitCode;
use futures_util::future::join_all;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::Semaphore;

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

    // Fan out per-host work concurrently (Python spawned one worker task per
    // target); `join_all` preserves key order for exit aggregation. Bounded
    // by a semaphore (P1 step 15) — see `add.rs`'s `run_add` for why this
    // avoids both the head-of-line blocking `.buffered(cap)` would cause
    // and the index/sort step `buffer_unordered` would need.
    let cap = group.host_operation_limit().get();
    let semaphore = Arc::new(Semaphore::new(cap));
    // One fleet-wide probe budget (P1 step 23) shared by every host worker,
    // replacing the old per-host `min(16, n)` local cap.
    let probe_budget = ProbeBudget::new(opts.probe_concurrency_limit);
    let console = SharedConsole::new(console);
    let results = join_all(group.hosts_mut().into_iter().map(|host| {
        let semaphore = Arc::clone(&semaphore);
        let probe_budget = probe_budget.clone();
        let repoq = &repoq;
        let console = &console;
        async move {
            let _permit = semaphore
                .acquire()
                .await
                .expect("host-operation semaphore is never closed");
            reset_one(opts, host, probe, repoq, console, &probe_budget).await
        }
    }))
    .await;
    group.close().await;
    Ok(aggregate(results))
}

async fn reset_one<W: Write>(
    opts: &CommandOptions,
    host: &mut dyn Host,
    probe: &dyn Probe,
    repoq: &crate::repoq::Repoq,
    console: &SharedConsole<'_, W>,
    probe_budget: &ProbeBudget,
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
        console.info(&format!("No repositories to clear from {}", host.key()));
    }

    let Some(products) = host.products() else {
        console.error(host.key(), "no products discovered");
        return false;
    };

    let resolved = match repoq.solve_product(products) {
        Ok(m) => m,
        Err(e) => {
            console.error(host.key(), &e.to_string());
            return false;
        }
    };
    let candidates: Vec<_> = resolved.into_values().flatten().collect();
    // Partition in one pass instead of probing a clone of `candidates` and
    // re-deriving the dropped set with per-element (name, url) clones.
    let (live, dead) = partition_live(
        probe,
        candidates,
        opts.probe_timeout,
        opts.no_probe,
        probe_budget,
    )
    .await;
    let mut dropped: Vec<&str> = dead.iter().map(|r| r.name.as_str()).collect();
    // Python reports `", ".join(sorted(dropped))`.
    dropped.sort_unstable();

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
        console.error(
            host.key(),
            "no live replacement repositories resolved; aborting reset to \
             avoid leaving the host without any repositories",
        );
        return false;
    }
    if !dropped.is_empty() {
        console.error(
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
            console.dry(host.key(), &cmd::zypper_rr(&refs));
        }
        for c in &cmds {
            console.dry(host.key(), c);
        }
        return true;
    }

    let mut ok = true;
    if !aliases.is_empty() {
        let refs: Vec<&str> = aliases.iter().map(String::as_str).collect();
        let rr = cmd::zypper_rr(&refs);
        if !run_reported_shared(host, &rr, console).await {
            ok = false;
        }
    }
    for c in cmds {
        if !run_reported_shared(host, &c, console).await {
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

    /// P1 step 15: a configured host-operation limit below the host count
    /// bounds the per-host worker semaphore, while every host still resets
    /// exactly once.
    #[tokio::test]
    async fn bounded_reset_never_exceeds_the_configured_host_operation_limit() {
        use crate::mock::{MockGate, MockMetrics, MockOpKind};
        use std::num::NonZeroUsize;

        let metrics = MockMetrics::new();
        let gate = MockGate::new();
        const LIMIT: usize = 2;
        const HOSTS: usize = 5;
        let mut g =
            MockHostGroup::new().with_host_operation_limit(NonZeroUsize::new(LIMIT).unwrap());
        for i in 0..HOSTS {
            let h = MockHost::new(format!("h{i}"))
                .with_products(sles_system())
                .with_raw_repos(vec![raw_repo("existing-repo1")])
                .with_metrics(metrics.clone())
                .with_gate(MockOpKind::Run, gate.clone());
            g.insert(h);
        }
        let opts = opts(false);
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        let probe = ConstProbe { live: true };

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
        let (code, ()) = tokio::join!(run_reset(&opts, &mut g, &probe, &mut c), driver);
        assert_eq!(code.unwrap(), ExitCode::Ok);

        for i in 0..HOSTS {
            let ran = g.get_mock_mut(&format!("h{i}")).unwrap().ran.clone();
            assert!(
                ran.iter().any(|cmd| cmd.starts_with("zypper -n rr")),
                "h{i} must have reset exactly once, ran: {ran:?}"
            );
        }
    }
}
