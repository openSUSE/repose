//! `repose clear` — `zypper rr` every raw alias; never `_report_target`.

use crate::commands::{aggregate, CommandOptions};
use crate::console::Console;
use crate::shell::cmd;
use crate::traits::HostGroup;
use crate::types::ExitCode;
use std::io::Write;

pub async fn run_clear<W: Write>(
    opts: &CommandOptions,
    group: &mut dyn HostGroup,
    console: &mut Console<W>,
) -> ExitCode {
    group.connect_and_prune().await;
    group.read_repos().await;

    let keys = group.keys();
    let mut results = Vec::new();
    for key in keys {
        let Some(host) = group.get_mut(&key) else {
            results.push(false);
            continue;
        };
        let mut aliases: Vec<String> = host
            .raw_repos()
            .map(|r| r.iter().map(|x| x.alias.clone()).collect())
            .unwrap_or_default();
        if aliases.is_empty() {
            let _ = console.info(&format!("No repositories to clear from {}", host.key()));
            results.push(true);
            continue;
        }
        // Python `_clear` collects into a set: sorted + unique.
        aliases.sort();
        aliases.dedup();
        let refs: Vec<&str> = aliases.iter().map(String::as_str).collect();
        let c = cmd::zypper_rr(&refs);
        if opts.dry {
            let _ = console.dry(host.key(), &c);
            results.push(true);
            continue;
        }
        // Never report_target (Python parity); only a transport-level error
        // (worker exception in Python) fails the host.
        match host.run(&c).await {
            Ok(()) => {
                let _ = console.info(&format!("Repositories cleared from {}", host.key()));
                results.push(true);
            }
            Err(_) => results.push(false),
        }
    }

    group.close().await;
    aggregate(results)
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
}
