//! `repose clear` — `zypper rr` every raw alias; never `_report_target`.

use crate::commands::{CommandOptions, SharedConsole, aggregate};
use crate::console::Console;
use crate::shell::cmd;
use crate::traits::{Host, HostGroup};
use crate::types::ExitCode;
use futures_util::future::join_all;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub async fn run_clear<W: Write>(
    opts: &CommandOptions,
    group: &mut dyn HostGroup,
    console: &mut Console<W>,
) -> ExitCode {
    group.connect_and_prune().await;
    group.read_repos().await;

    // Fan out per-host work concurrently (Python spawned one worker task per
    // target); `join_all` preserves key order for exit aggregation. Bounded
    // by a semaphore (P1 step 17) — see `add.rs`'s `run_add` for why this
    // avoids both the head-of-line blocking `.buffered(cap)` would cause
    // and the index/sort step `buffer_unordered` would need.
    let cap = group.host_operation_limit().get();
    let semaphore = Arc::new(Semaphore::new(cap));
    let console = SharedConsole::new(console);
    let results = join_all(group.hosts_mut().into_iter().map(|host| {
        let semaphore = Arc::clone(&semaphore);
        let console = &console;
        async move {
            let _permit = semaphore
                .acquire()
                .await
                .expect("host-operation semaphore is never closed");
            clear_one(opts, host, console).await
        }
    }))
    .await;

    group.close().await;
    aggregate(results)
}

async fn clear_one<W: Write>(
    opts: &CommandOptions,
    host: &mut dyn Host,
    console: &SharedConsole<'_, W>,
) -> bool {
    let mut aliases: Vec<String> = host
        .raw_repos()
        .map(|r| r.iter().map(|x| x.alias.clone()).collect())
        .unwrap_or_default();
    if aliases.is_empty() {
        console.info(&format!("No repositories to clear from {}", host.key()));
        return true;
    }
    // Python `_clear` collects into a set: sorted + unique.
    aliases.sort();
    aliases.dedup();
    let refs: Vec<&str> = aliases.iter().map(String::as_str).collect();
    let c = cmd::zypper_rr(&refs);
    if opts.dry {
        console.dry(host.key(), &c);
        return true;
    }
    // Never report_target (Python parity); only a transport-level error
    // (worker exception in Python) fails the host.
    match host.run(&c).await {
        Ok(()) => {
            console.info(&format!("Repositories cleared from {}", host.key()));
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::Buffer;
    use crate::mock::{MockHost, MockHostGroup};
    use crate::traits::Host;
    use crate::types::Repository;

    #[tokio::test]
    async fn clear_empty_is_ok() {
        let mut g = MockHostGroup::new();
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        g.insert(h);
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        let code = run_clear(&CommandOptions::default(), &mut g, &mut c).await;
        assert_eq!(code, ExitCode::Ok);
        // Python logs the INFO no-op instead of staying silent.
        assert!(buf.0.contains("No repositories to clear from h1"));
    }

    #[tokio::test]
    async fn clear_live_dedups_aliases_and_reports_cleared() {
        let mut g = MockHostGroup::new();
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        let repo = |alias: &str| Repository {
            alias: alias.into(),
            name: "n".into(),
            url: "http://x".into(),
            state: true,
        };
        // Duplicate alias: Python `_clear` returns a set → unique args.
        h = h.with_raw_repos(vec![repo("b"), repo("a"), repo("a")]);
        g.insert(h);
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        let code = run_clear(&CommandOptions::default(), &mut g, &mut c).await;
        assert_eq!(code, ExitCode::Ok);
        let ran = g.get_mock_mut("h1").unwrap().ran.clone();
        assert_eq!(ran, vec![cmd::zypper_rr(&["a", "b"])]);
        assert!(buf.0.contains("Repositories cleared from h1"));
    }

    #[tokio::test]
    async fn clear_dry_lists_rr() {
        let mut g = MockHostGroup::new();
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h = h.with_raw_repos(vec![Repository {
            alias: "a".into(),
            name: "n".into(),
            url: "http://x".into(),
            state: true,
        }]);
        g.insert(h);
        let opts = CommandOptions {
            dry: true,
            ..Default::default()
        };
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        let code = run_clear(&opts, &mut g, &mut c).await;
        assert_eq!(code, ExitCode::Ok);
        assert!(buf.0.contains("zypper") && buf.0.contains("rr"));
    }

    /// P1 step 17: a configured host-operation limit below the host count
    /// bounds the per-host worker semaphore, while every host still
    /// clears exactly once.
    #[tokio::test]
    async fn bounded_clear_never_exceeds_the_configured_host_operation_limit() {
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
                .with_raw_repos(vec![Repository {
                    alias: "a".into(),
                    name: "n".into(),
                    url: "http://x".into(),
                    state: true,
                }])
                .with_metrics(metrics.clone())
                .with_gate(MockOpKind::Run, gate.clone());
            g.insert(h);
        }
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
        let opts = CommandOptions::default();
        let (code, ()) = tokio::join!(run_clear(&opts, &mut g, &mut c), driver);
        assert_eq!(code, ExitCode::Ok);

        for i in 0..HOSTS {
            let ran = g.get_mock_mut(&format!("h{i}")).unwrap().ran.clone();
            assert!(
                ran.iter().any(|cmd| cmd.starts_with("zypper -n rr")),
                "h{i} must have cleared exactly once, ran: {ran:?}"
            );
        }
    }
}
