//! Command algorithms against [`crate::traits::Host`] / [`crate::traits::HostGroup`].

mod add;
mod clear;
mod install;
mod list_cmd;
mod remove;
mod reset;
mod uninstall;

pub use add::run_add;
pub use clear::run_clear;
pub use install::run_install;
pub use list_cmd::{run_known_products, run_list_products, run_list_repos};
pub use remove::run_remove;
pub use reset::run_reset;
pub use uninstall::run_uninstall;

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use crate::console::{Console, Level, OutputFormat};
use crate::probe::HttpProbe;
use crate::repa::Repa;
use crate::repoq::Repoq;
use crate::shell::cmd;
use crate::template::{TemplateError, load_template};
use crate::traits::{Host, Probe};
use crate::types::ExitCode;

/// Shared options for mutation / list commands.
pub struct CommandOptions {
    pub dry: bool,
    pub config: PathBuf,
    pub repa: Vec<Repa>,
    pub probe_timeout: Duration,
    pub no_probe: bool,
    pub no_reboot: bool,
    pub format: OutputFormat,
    /// `list-products --yaml`: emit a YAML refhost spec instead of text/json.
    pub yaml: bool,
    /// Emit ANSI color in `list-*` text output (resolved from `--color` /
    /// `--no-color` / `NO_COLOR` / TTY by the CLI).
    pub color: bool,
}

impl Default for CommandOptions {
    fn default() -> Self {
        Self {
            dry: false,
            config: PathBuf::from("/etc/repose/products.yml"),
            repa: Vec::new(),
            probe_timeout: Duration::from_secs_f64(5.0),
            no_probe: false,
            no_reboot: false,
            format: OutputFormat::Text,
            yaml: false,
            color: false,
        }
    }
}

pub(crate) fn load_repoq(config: &std::path::Path) -> Result<Repoq, TemplateError> {
    Ok(Repoq::new(load_template(config)?))
}

/// Console shared by the concurrent per-host futures of a mutation command.
///
/// Each emission takes the lock only for the duration of one synchronous
/// write — never across an `.await` — so live output stays streaming and
/// host-prefixed; lines from different hosts may interleave, matching the
/// Python asyncio worker model.
pub(crate) struct SharedConsole<'a, W: Write> {
    inner: Mutex<&'a mut Console<W>>,
}

impl<'a, W: Write> SharedConsole<'a, W> {
    pub(crate) const fn new(console: &'a mut Console<W>) -> Self {
        Self {
            inner: Mutex::new(console),
        }
    }

    /// Run a synchronous emission block under the lock.
    ///
    /// A poisoned lock only means a sibling host future panicked mid-write;
    /// keep reporting from the remaining hosts.
    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut Console<W>) -> R) -> R {
        let mut guard = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut guard)
    }

    pub(crate) fn dry(&self, host: &str, cmd: &str) {
        self.with(|c| {
            let _ = c.dry(host, cmd);
        });
    }

    pub(crate) fn error(&self, host: &str, msg: &str) {
        self.with(|c| {
            let _ = c.error(host, msg);
        });
    }

    pub(crate) fn info(&self, msg: &str) {
        self.with(|c| {
            let _ = c.info(msg);
        });
    }
}

/// Report the last command's output for `host` via the console and classify
/// its exit code (Python `_report_target`).
///
/// - exit `0`: stdout lines at info level, success.
/// - informational success codes (100-103, 106, 107): the work completed but
///   the code carries a follow-up note that zypper may write to *either*
///   stream, so surface both at warning level rather than dropping it.
/// - any other exit: both streams at error level, failure — some diagnostics
///   (e.g. "repository already exists") go to stdout, so reporting stderr
///   alone would leave a non-zero exit unexplained.
pub(crate) fn report_target<W: Write>(host: &dyn Host, console: &mut Console<W>) -> bool {
    let Some((_, stdout, stderr, exitcode, _)) = host.out().last() else {
        return false;
    };
    if *exitcode == 0 {
        for line in stdout.lines() {
            let _ = console.report(host.key(), line, true, Level::Info);
        }
        return true;
    }
    if crate::types::zypper_exit_ok(*exitcode) {
        for stream in [stdout, stderr] {
            for line in stream.lines() {
                let _ = console.report(host.key(), line, true, Level::Warning);
            }
        }
        return true;
    }
    for stream in [stdout, stderr] {
        for line in stream.lines() {
            let _ = console.report(host.key(), line, false, Level::Error);
        }
    }
    false
}

/// Run `command` on `host` and report the result (the Python
/// `targets[host].run(cmd)` + `_report_target` pair).
///
/// A transport-level `Err` (no `out` entry appended) counts as host failure,
/// mirroring the Python worker-exception path into `_aggregate`.
///
/// The console lock is taken only after the `.await` completes, for the
/// synchronous report block.
pub(crate) async fn run_reported_shared<W: Write>(
    host: &mut dyn Host,
    command: &str,
    console: &SharedConsole<'_, W>,
) -> bool {
    match host.run(command).await {
        Ok(()) => console.with(|c| report_target(host, c)),
        Err(_) => false,
    }
}

/// Serial-console convenience wrapper around [`run_reported_shared`], kept
/// for unit tests that drive a single host with a plain `&mut Console`.
#[cfg(test)]
pub(crate) async fn run_reported<W: Write>(
    host: &mut dyn Host,
    command: &str,
    console: &mut Console<W>,
) -> bool {
    run_reported_shared(host, command, &SharedConsole::new(console)).await
}

/// Verify `products` are present/absent in the host's re-read state
/// (Python `_check_products`). Caller must have refreshed products.
fn check_products<W: Write>(
    host: &dyn Host,
    products: &[String],
    present: bool,
    console: &mut Console<W>,
) -> bool {
    // Python: `installed = {...} if system else set()` — an unreadable /
    // empty product state fails every `present` check instead of passing.
    let installed: std::collections::BTreeSet<String> = host
        .products()
        .map(|s| s.flatten().into_iter().map(|p| p.name).collect())
        .unwrap_or_default();
    let mut ok = true;
    for product in products {
        if present && !installed.contains(product) {
            let _ = console.error(
                host.key(),
                &format!("product {product} not installed after reboot"),
            );
            ok = false;
        } else if !present && installed.contains(product) {
            let _ = console.error(
                host.key(),
                &format!("product {product} still present after reboot"),
            );
            ok = false;
        }
    }
    if ok {
        let _ = console.info(&format!(
            "{}: verified product(s) {} after reboot",
            host.key(),
            products.join(", ")
        ));
    }
    ok
}

/// Reboot a transactional host, then verify the change took
/// (Python `_reboot_and_verify`). Shared by install (`present=true`)
/// and uninstall (`present=false`).
///
/// With `no_reboot` the change is left staged and only a reminder is
/// printed (returns `true`). Otherwise the host is rebooted + reconnected
/// and its products are re-read and checked.
pub(crate) async fn reboot_and_verify_shared<W: Write>(
    host: &mut dyn Host,
    products: &[String],
    present: bool,
    no_reboot: bool,
    console: &SharedConsole<'_, W>,
) -> bool {
    if no_reboot {
        console.info(&format!(
            "Reboot {} to activate the staged snapshot (--no-reboot set)",
            host.key()
        ));
        return true;
    }
    if !matches!(host.reboot(cmd::REBOOT).await, Ok(true)) {
        return false;
    }
    if host.read_products().await.is_err() {
        console.error(host.key(), "could not re-read products after reboot");
        return false;
    }
    console.with(|c| check_products(host, products, present, c))
}

/// Serial-console convenience wrapper around [`reboot_and_verify_shared`],
/// kept for unit tests that drive a single host with a plain `&mut Console`.
#[cfg(test)]
pub(crate) async fn reboot_and_verify<W: Write>(
    host: &mut dyn Host,
    products: &[String],
    present: bool,
    no_reboot: bool,
    console: &mut Console<W>,
) -> bool {
    reboot_and_verify_shared(
        host,
        products,
        present,
        no_reboot,
        &SharedConsole::new(console),
    )
    .await
}

/// Aggregate per-host bool results (Python `_aggregate`).
pub fn aggregate(results: impl IntoIterator<Item = bool>) -> ExitCode {
    ExitCode::aggregate(results)
}

/// Probe `repos` and split them into `(live, dropped)`, both preserving
/// input order. With `no_probe` everything is live.
///
/// Reset needs both halves (its partial-drop guard reports the dropped
/// names), so partitioning here avoids probing a clone of the candidate
/// list and re-deriving the dropped set with per-element clones.
pub(crate) async fn partition_live(
    probe: &dyn Probe,
    repos: Vec<crate::repoq::Repos>,
    timeout: Duration,
    no_probe: bool,
) -> (Vec<crate::repoq::Repos>, Vec<crate::repoq::Repos>) {
    use futures_util::StreamExt;
    if no_probe || repos.is_empty() {
        return (repos, Vec::new());
    }
    // Probe concurrently, bounded to 16 in-flight (Python `_afilter_live_urls`
    // uses `asyncio.Semaphore(min(16, n))`); `buffered` preserves input order.
    let cap = std::cmp::min(16, repos.len());
    let alive: Vec<bool> = futures_util::stream::iter(repos.iter())
        .map(|r| probe.is_live(&r.url, timeout))
        .buffered(cap)
        .collect()
        .await;
    let mut live = Vec::new();
    let mut dropped = Vec::new();
    for (repo, ok) in repos.into_iter().zip(alive) {
        if ok {
            live.push(repo);
        } else {
            dropped.push(repo);
        }
    }
    (live, dropped)
}

pub(crate) async fn filter_live(
    probe: &dyn Probe,
    repos: Vec<crate::repoq::Repos>,
    timeout: Duration,
    no_probe: bool,
) -> Vec<crate::repoq::Repos> {
    partition_live(probe, repos, timeout, no_probe).await.0
}

/// Default HTTP probe; tests inject [`crate::mock::ConstProbe`].
///
/// Never panics: if the HTTP client cannot be built (e.g. unloadable native
/// root store) a disabled probe is returned that reports every URL dead
/// (see [`HttpProbe::default`]).
#[must_use]
pub fn default_probe() -> HttpProbe {
    HttpProbe::default()
}

#[cfg(test)]
mod filter_tests {
    use super::*;
    use crate::mock::{ConstProbe, MapProbe};
    use crate::repoq::Repos;

    fn repo(name: &str, url: &str) -> Repos {
        Repos {
            name: name.into(),
            url: url.into(),
            refresh: false,
        }
    }

    #[tokio::test]
    async fn filter_live_preserves_order_and_drops_dead() {
        let repos = vec![
            repo("a", "http://a/"),
            repo("b", "http://b/"),
            repo("c", "http://c/"),
        ];
        let probe = MapProbe::dead(["http://b/"]);
        let live = filter_live(&probe, repos, Duration::from_secs(1), false).await;
        let urls: Vec<&str> = live.iter().map(|r| r.url.as_str()).collect();
        assert_eq!(urls, ["http://a/", "http://c/"]);
    }

    #[tokio::test]
    async fn filter_live_no_probe_returns_all_unprobed() {
        let repos = vec![repo("a", "http://a/"), repo("b", "http://b/")];
        // no_probe=true short-circuits even a dead probe.
        let live = filter_live(
            &ConstProbe { live: false },
            repos,
            Duration::from_secs(1),
            true,
        )
        .await;
        assert_eq!(live.len(), 2);
    }
}

#[cfg(test)]
mod report_tests {
    use super::*;
    use crate::console::Buffer;
    use crate::mock::{MockHost, MockRunOutcome};

    async fn host_after(outcome: MockRunOutcome) -> MockHost {
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h.push_run(outcome);
        h.run("zypper -n ar x").await.unwrap();
        h
    }

    #[tokio::test]
    async fn report_exit_zero_prints_stdout_only_at_info() {
        let h = host_after(MockRunOutcome::Complete {
            stdout: "line1\nline2".into(),
            stderr: "noise".into(),
            exitcode: 0,
            runtime_secs: 0,
        })
        .await;
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        assert!(report_target(&h, &mut c));
        // stderr is NOT reported on exit 0 (Python `_report_target`).
        assert_eq!(buf.0, "h1 - line1\nh1 - line2\n");
    }

    #[tokio::test]
    async fn report_informational_exit_warns_both_streams() {
        let h = host_after(MockRunOutcome::Complete {
            stdout: "did work".into(),
            stderr: "repository skipped".into(),
            exitcode: 106,
            runtime_secs: 0,
        })
        .await;
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        assert!(report_target(&h, &mut c));
        assert_eq!(buf.0, "h1 - did work\nh1 - repository skipped\n");
    }

    #[tokio::test]
    async fn report_failure_emits_error_report_events_json() {
        let h = host_after(MockRunOutcome::Complete {
            stdout: "already exists".into(),
            stderr: "boom".into(),
            exitcode: 4,
            runtime_secs: 0,
        })
        .await;
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        c.format = OutputFormat::Json;
        assert!(!report_target(&h, &mut c));
        let lines: Vec<serde_json::Value> = buf
            .0
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        for v in &lines {
            assert_eq!(v["event"], "report");
            assert_eq!(v["level"], "error");
            assert_eq!(v["ok"], false);
            assert_eq!(v["host"], "h1");
        }
        assert_eq!(lines[0]["line"], "already exists");
        assert_eq!(lines[1]["line"], "boom");
    }

    #[tokio::test]
    async fn report_empty_out_history_is_failure() {
        let h = MockHost::new("h1");
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        assert!(!report_target(&h, &mut c));
        assert!(buf.0.is_empty());
    }

    #[tokio::test]
    async fn run_reported_transport_err_is_failure() {
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h.push_run(MockRunOutcome::TransportErr("wire cut".into()));
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        assert!(!run_reported(&mut h, "zypper -n lr", &mut c).await);
    }

    #[tokio::test]
    async fn reboot_verify_no_reboot_prints_reminder_and_skips_reboot() {
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        let mut buf = Buffer::default();
        let mut c = Console::new(&mut buf);
        let ok = reboot_and_verify(&mut h, &["SLES".into()], true, true, &mut c).await;
        assert!(ok);
        assert!(h.ran.is_empty(), "no reboot command with --no-reboot");
        assert_eq!(
            buf.0,
            "Reboot h1 to activate the staged snapshot (--no-reboot set)\n"
        );
    }
}

/// L2 sequence vectors under `tests/vectors/sequences/` (expected command sequences).
#[cfg(test)]
pub(crate) mod seq {
    use crate::types::ExitCode;
    use serde::Deserialize;
    use std::path::PathBuf;

    /// One expected command-sequence scenario for a mutation command.
    #[derive(Debug, Deserialize)]
    pub(crate) struct SeqCase {
        pub name: String,
        pub exit: String,
        /// Remote commands issued in order (live path); empty for dry/abort.
        #[serde(default)]
        pub ran: Vec<String>,
        /// Dry-run preview lines in order; empty for live/abort.
        #[serde(default)]
        pub dry: Vec<String>,
    }

    impl SeqCase {
        pub(crate) fn exit_code(&self) -> ExitCode {
            match self.exit.as_str() {
                "ok" => ExitCode::Ok,
                "partial" => ExitCode::Partial,
                "allfailed" => ExitCode::AllFailed,
                other => panic!("unknown exit code {other:?}"),
            }
        }

        /// Expected text-mode console buffer for the `dry` lines against `host`.
        pub(crate) fn dry_buffer(&self, host: &str) -> String {
            self.dry.iter().map(|c| format!("{host} - {c}\n")).collect()
        }
    }

    fn load(cmd: &str) -> Vec<SeqCase> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/vectors/sequences/{cmd}.json"));
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
    }

    /// Load the named scenario from `tests/vectors/sequences/{cmd}.json`.
    pub(crate) fn case(cmd: &str, name: &str) -> SeqCase {
        load(cmd)
            .into_iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no sequence case {cmd}/{name}"))
    }
}
