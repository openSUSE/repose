//! Command algorithms against [`crate::traits::Host`] / [`HostGroup`].

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

use std::path::PathBuf;
use std::time::Duration;

use crate::console::OutputFormat;
use crate::probe::HttpProbe;
use crate::repa::Repa;
use crate::repoq::Repoq;
use crate::template::{load_template, TemplateError};
use crate::traits::{last_out_succeeded, Host, Probe};
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
        }
    }
}

pub(crate) fn load_repoq(config: &std::path::Path) -> Result<Repoq, TemplateError> {
    Ok(Repoq::new(load_template(config)?))
}

pub(crate) fn report_target(host: &dyn Host) -> bool {
    last_out_succeeded(host.out()).unwrap_or(false)
}

/// Aggregate per-host bool results (Python `_aggregate`).
pub fn aggregate(results: impl IntoIterator<Item = bool>) -> ExitCode {
    ExitCode::aggregate(results)
}

pub(crate) async fn filter_live(
    probe: &dyn Probe,
    repos: Vec<crate::repoq::Repos>,
    timeout: Duration,
    no_probe: bool,
) -> Vec<crate::repoq::Repos> {
    use futures_util::StreamExt;
    if no_probe || repos.is_empty() {
        return repos;
    }
    // Probe concurrently, bounded to 16 in-flight (Python `_afilter_live_urls`
    // uses `asyncio.Semaphore(min(16, n))`); `buffered` preserves input order.
    let cap = std::cmp::min(16, repos.len());
    let alive: Vec<bool> = futures_util::stream::iter(repos.iter())
        .map(|r| probe.is_live(&r.url, timeout))
        .buffered(cap)
        .collect()
        .await;
    repos
        .into_iter()
        .zip(alive)
        .filter_map(|(r, live)| live.then_some(r))
        .collect()
}

/// Default HTTP probe; tests inject [`crate::mock::ConstProbe`].
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

/// L2 sequence goldens under `tests/oracle/sequences/` (Python-oracle derived).
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
            .join(format!("../../tests/oracle/sequences/{cmd}.json"));
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
    }

    /// Load the named scenario from `tests/oracle/sequences/{cmd}.json`.
    pub(crate) fn case(cmd: &str, name: &str) -> SeqCase {
        load(cmd)
            .into_iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no sequence case {cmd}/{name}"))
    }
}
