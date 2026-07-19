//! `repose add` — resolve REPA, probe, zypper ar, cohort refresh.

use crate::commands::{
    aggregate, filter_live, load_repoq, run_reported_shared, CommandOptions, SharedConsole,
};
use crate::console::Console;
use crate::shell::cmd;
use crate::traits::{Host, HostGroup, Probe};
use crate::types::ExitCode;
use futures_util::future::join_all;
use std::io::Write;

pub async fn run_add<W: Write>(
    opts: &CommandOptions,
    group: &mut dyn HostGroup,
    probe: &dyn Probe,
    console: &mut Console<W>,
) -> Result<ExitCode, crate::template::TemplateError> {
    let repoq = load_repoq(&opts.config)?;
    group.connect_and_prune().await;
    group.read_products().await;

    // Fan out per-host work concurrently (Python spawned one worker task per
    // target); `join_all` preserves key order for exit aggregation.
    let console = SharedConsole::new(console);
    let results = join_all(
        group
            .hosts_mut()
            .into_iter()
            .map(|host| add_one(opts, host, probe, &repoq, &console)),
    )
    .await;

    if !opts.dry {
        group.run_all(cmd::REFCMD).await;
    }
    group.close().await;
    Ok(aggregate(results))
}

async fn add_one<W: Write>(
    opts: &CommandOptions,
    host: &mut dyn Host,
    probe: &dyn Probe,
    repoq: &crate::repoq::Repoq,
    console: &SharedConsole<'_, W>,
) -> bool {
    let Some(products) = host.products() else {
        return false;
    };
    let base = products.get_base().clone();
    let mut ok = true;
    let mut candidates = Vec::new();
    for repa in &opts.repa {
        match repoq.solve_repa(repa, &base) {
            Ok(map) => {
                for list in map.into_values() {
                    candidates.extend(list);
                }
            }
            Err(e) => {
                console.error(host.key(), &e.to_string());
                ok = false;
            }
        }
    }
    let live = filter_live(probe, candidates, opts.probe_timeout, opts.no_probe).await;
    let mut cmds: Vec<String> = live
        .iter()
        .map(|r| cmd::zypper_ar(r.refresh, &r.name, &r.url))
        .collect();
    cmds.sort();
    // Python `_add` collects into a set; dedup identical ar strings so a
    // duplicate REPA does not issue the same `zypper ar` twice (the second
    // run fails with "repository already exists" and wrongly fails the host).
    cmds.dedup();
    for c in cmds {
        if opts.dry {
            console.dry(host.key(), &c);
        } else if !run_reported_shared(host, &c, console).await {
            ok = false;
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::{Buffer, OutputFormat};
    use crate::mock::{ConstProbe, MockHost, MockHostGroup};
    use crate::repa::Repa;
    use crate::traits::Host;
    use crate::types::{Product, System};
    use std::path::PathBuf;

    fn sample_config() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/vectors/template/sample.yml")
    }

    fn sles_host() -> MockHost {
        MockHost::new("h1").with_products(System {
            base: Product {
                name: "SLES".into(),
                version: "15-SP3".into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional: false,
        })
    }

    async fn run_live(host: MockHost, repas: &[&str]) -> (ExitCode, Vec<String>, String) {
        let mut g = MockHostGroup::new();
        g.insert(host);
        let opts = CommandOptions {
            config: sample_config(),
            repa: repas.iter().map(|r| Repa::parse(r).unwrap()).collect(),
            no_probe: true,
            ..Default::default()
        };
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        let probe = ConstProbe { live: true };
        let code = run_add(&opts, &mut g, &probe, &mut c).await.unwrap();
        let ran = g
            .get_mock_mut("h1")
            .map(|h| h.ran.clone())
            .unwrap_or_default();
        (code, ran, buf.0)
    }

    #[tokio::test]
    async fn duplicate_repa_issues_each_ar_once() {
        // Python collected commands into a set; a REPA given twice must not
        // run the same `zypper ar` twice (2nd run fails → host wrongly FAILED).
        let (code1, ran1, _) = run_live(sles_host(), &["SLES:15-SP3:x86_64:update"]).await;
        let (code2, ran2, _) = run_live(
            sles_host(),
            &["SLES:15-SP3:x86_64:update", "SLES:15-SP3:x86_64:update"],
        )
        .await;
        assert_eq!(code1, ExitCode::Ok);
        assert_eq!(code2, ExitCode::Ok);
        assert!(ran1.iter().any(|c| c.starts_with("zypper -n ar")));
        assert_eq!(ran1, ran2, "duplicate REPA must not add extra commands");
    }

    #[tokio::test]
    async fn live_add_reports_zypper_stdout() {
        // Fix: live mutations must surface zypper's output via Console::report.
        let mut host = sles_host();
        host.push_run(crate::mock::MockRunOutcome::ok_stdout(
            "Repository 'update' successfully added",
        ));
        let (code, _, buf) = run_live(host, &["SLES:15-SP3:x86_64:update"]).await;
        assert_eq!(code, ExitCode::Ok);
        assert!(
            buf.contains("h1 - Repository 'update' successfully added\n"),
            "live run must report stdout lines, got: {buf:?}"
        );
    }

    #[tokio::test]
    async fn concurrent_hosts_overlap_in_run() {
        use crate::mock::RunBarrier;
        // Concurrency proof: both hosts must be inside `Host::run` at the
        // same time — each blocks at the barrier until the other arrives.
        // Under the old serial per-host loop, h2's run could not start
        // before h1's finished, so h1 would spin out and set `timed_out`.
        let barrier = RunBarrier::new(2);
        let mut g = MockHostGroup::new();
        for key in ["h1", "h2"] {
            g.insert(
                MockHost::new(key)
                    .with_products(System {
                        base: Product {
                            name: "SLES".into(),
                            version: "15-SP3".into(),
                            arch: "x86_64".into(),
                        },
                        addons: vec![],
                        transactional: false,
                    })
                    .with_run_barrier(barrier.clone()),
            );
        }
        let opts = CommandOptions {
            config: sample_config(),
            repa: vec![Repa::parse("SLES:15-SP3:x86_64:update").unwrap()],
            no_probe: true,
            ..Default::default()
        };
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        let code = run_add(&opts, &mut g, &ConstProbe { live: true }, &mut c)
            .await
            .unwrap();
        assert_eq!(code, ExitCode::Ok);
        assert!(
            !barrier.timed_out(),
            "hosts ran serially: h2 never entered run() while h1 was inside it"
        );
        for key in ["h1", "h2"] {
            let ran = g.get_mock_mut(key).unwrap().ran.clone();
            assert!(
                ran.iter().any(|c| c.starts_with("zypper -n ar")),
                "{key} must have run zypper ar, ran: {ran:?}"
            );
        }
    }

    #[tokio::test]
    async fn dry_add_prints_ar() {
        let mut g = MockHostGroup::new();
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h = h.with_products(System {
            base: Product {
                name: "SLES".into(),
                version: "15-SP3".into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional: false,
        });
        g.insert(h);
        let opts = CommandOptions {
            dry: true,
            config: sample_config(),
            repa: vec![Repa::parse("SLES:15-SP3:x86_64:update").unwrap()],
            no_probe: true,
            ..Default::default()
        };
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        c.format = OutputFormat::Text;
        let probe = ConstProbe { live: true };
        let code = run_add(&opts, &mut g, &probe, &mut c).await.unwrap();
        assert_eq!(code, ExitCode::Ok);
        assert!(buf.0.contains("zypper") && buf.0.contains("ar"));
    }
}
