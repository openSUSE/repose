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
    if no_probe || repos.is_empty() {
        return repos;
    }
    let mut out = Vec::new();
    for r in repos {
        if probe.is_live(&r.url, timeout).await {
            out.push(r);
        }
    }
    out
}

/// Default HTTP probe; tests inject [`crate::mock::ConstProbe`].
#[must_use]
pub fn default_probe() -> HttpProbe {
    HttpProbe::default()
}
