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
            results.push(true);
            continue;
        }
        aliases.sort();
        let refs: Vec<&str> = aliases.iter().map(String::as_str).collect();
        let c = cmd::zypper_rr(&refs);
        if opts.dry {
            let _ = console.dry(host.key(), &c);
            results.push(true);
            continue;
        }
        let _ = host.run(&c).await;
        results.push(true); // never report_target
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
