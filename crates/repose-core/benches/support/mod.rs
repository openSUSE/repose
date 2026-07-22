//! Fleet-scenario builders shared by the P0.3 Criterion benchmark
//! (`benches/fleet.rs`) and the P0.1 baseline-report harness
//! (`examples/baseline_report.rs`), which includes this file verbatim via
//! `#[path]` since Cargo does not let example/bench targets share a module
//! directly. Not part of the published `repose_core` crate.
//!
//! Scenarios are deterministic: every host starts from a fresh [`MockHost`]
//! with a shared [`MockMetrics`] tracker, so callers can assert exact
//! command/probe counts and peak concurrency alongside timing.

use repose_core::commands::CommandOptions;
use repose_core::console::OutputFormat;
use repose_core::mock::{MetricProbe, MockGate, MockHost, MockHostGroup, MockMetrics, MockOpKind};
use repose_core::repa::Repa;
use repose_core::types::{Product, System};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Fixed, documented delay used to model one slow host among a fleet.
/// Applied only by the benchmark/baseline harness (never inside
/// `repose-core::mock`, which stays sleep-free for deterministic unit
/// tests) — Criterion/the baseline report explicitly measure wall time, so
/// a small real delay is the honest way to produce a measurable tail.
pub const SLOW_HOST_DELAY: Duration = Duration::from_millis(20);

/// `tests/vectors/template/sample.yml`, shared with the crate's own unit
/// tests (defines SLES/QA/PackageHub repository templates).
#[must_use]
pub fn sample_config() -> PathBuf {
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

/// Zero-padded so `BTreeMap` (lexical) order matches numeric host order.
fn host_key(i: usize, total: usize) -> String {
    let width = total.max(1).to_string().len();
    format!("h{i:0width$}")
}

/// Dimensions from `tests/performance/workloads.json`.
pub struct ScenarioConfig {
    pub host_count: usize,
    /// Gate host 1's `run` operation on [`Scenario::slow_gate`], modeling
    /// one slow host among a fast fleet (see [`Scenario::arm_slow_host`]).
    pub slow_host: bool,
    /// Pass the same REPA argument twice, so identical URL candidates are
    /// resolved and probed within one host (see `add_one`/`install_one`
    /// dedup and future P3 probe-cache work).
    pub repeated_urls: bool,
    pub output_format: OutputFormat,
}

impl ScenarioConfig {
    #[must_use]
    pub fn new(host_count: usize) -> Self {
        Self {
            host_count,
            slow_host: false,
            repeated_urls: false,
            output_format: OutputFormat::Text,
        }
    }
}

/// One fleet-scale scenario: hosts, probe, shared metrics, and options.
pub struct Scenario {
    pub group: MockHostGroup,
    pub metrics: MockMetrics,
    pub probe: MetricProbe,
    pub opts: CommandOptions,
    /// Present when the scenario has a gated slow host; call
    /// [`Scenario::arm_slow_host`] from inside the runtime that will drive
    /// the command so the delayed release races the command's own futures.
    pub slow_gate: Option<Arc<MockGate>>,
}

impl Scenario {
    /// Spawn the timer that releases the slow host's gate after
    /// [`SLOW_HOST_DELAY`]. No-op if the scenario has no slow host. Must be
    /// called from within the Tokio runtime driving the command.
    pub fn arm_slow_host(&self) {
        if let Some(gate) = self.slow_gate.clone() {
            tokio::spawn(async move {
                tokio::time::sleep(SLOW_HOST_DELAY).await;
                gate.release();
            });
        }
    }
}

/// Build a fresh `add`/`install` scenario resolving `repa` on every host.
#[must_use]
pub fn build_scenario(cfg: &ScenarioConfig, repa: &[&str]) -> Scenario {
    let metrics = MockMetrics::new();
    let mut group = MockHostGroup::new();
    let mut slow_gate = None;
    for i in 1..=cfg.host_count {
        let key = host_key(i, cfg.host_count);
        let mut host = MockHost::new(key)
            .with_products(sles_system())
            .with_metrics(metrics.clone());
        if cfg.slow_host && i == 1 {
            let gate = MockGate::new();
            host = host.with_gate(MockOpKind::Run, gate.clone());
            slow_gate = Some(gate);
        }
        group.insert(host);
    }

    let mut repa_args: Vec<&str> = repa.to_vec();
    if cfg.repeated_urls {
        repa_args.extend(repa);
    }
    let opts = CommandOptions {
        config: sample_config(),
        repa: repa_args
            .iter()
            .map(|r| Repa::parse(r).expect("valid REPA in scenario fixture"))
            .collect(),
        no_probe: false,
        probe_timeout: Duration::from_secs(5),
        format: cfg.output_format,
        ..Default::default()
    };
    let probe = MetricProbe::new(true).with_metrics(metrics.clone());

    Scenario {
        group,
        metrics,
        probe,
        opts,
        slow_gate,
    }
}

/// Build a `list-products` scenario (no repository resolution/probing).
#[must_use]
pub fn build_list_products_scenario(cfg: &ScenarioConfig) -> Scenario {
    build_scenario(cfg, &[])
}

/// One documented async runtime boundary shared by every scenario/command
/// invocation, so benchmark/report comparisons attribute cost consistently
/// rather than to runtime construction (see P0.3 assumptions).
#[must_use]
pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("build current-thread benchmark runtime")
}
