//! Fleet-scenario builders shared by the P0.3 Criterion benchmark
//! (`benches/fleet.rs`) and the P0.1 baseline-report harness
//! (`examples/baseline_report.rs`), which includes this file verbatim via
//! `#[path]` since Cargo does not let example/bench targets share a module
//! directly. Not part of the published `repose_core` crate.
//!
//! Scenarios are deterministic: every host starts from a fresh [`MockHost`]
//! with a shared [`MockMetrics`] tracker, so callers can assert exact
//! command/probe counts and peak concurrency alongside timing.

use repose_core::commands::{CommandOptions, run_add};
use repose_core::console::{Buffer, Console, OutputFormat};
use repose_core::mock::{
    MetricProbe, MockGate, MockHost, MockHostGroup, MockMetrics, MockMetricsSnapshot, MockOpKind,
};
use repose_core::repa::Repa;
use repose_core::types::{ExitCode, Product, System};
use std::num::NonZeroUsize;
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

/// Reviewed baseline REPA used by [`run_fully_gated_add`] (same fixture
/// repository as `benches/fleet.rs`'s `ADD_REPA`).
#[allow(dead_code)]
const GATED_ADD_REPA: &str = "SLES:15-SP3:x86_64:update";

/// Deterministic (no sleep) spin-wait: yields to the executor until `pred`
/// observes the expected in-flight count. Lifts the same technique already
/// used by `repose_core::mock`'s own concurrency-proof unit tests (e.g.
/// `gated_hosts_overlap_reaches_expected_peak_operations`) so fleet-scale
/// scenarios can prove the same thing at 20/100 hosts instead of two.
///
/// Only `benches/fleet.rs` uses this (and [`run_fully_gated_add`]) today —
/// `examples/baseline_report.rs` includes this same module via `#[path]`
/// but doesn't, so each is otherwise flagged dead code in that binary.
#[allow(dead_code)]
async fn wait_until(mut pred: impl FnMut() -> bool) {
    const MAX_SPINS: usize = 200_000;
    for _ in 0..MAX_SPINS {
        if pred() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("wait_until: condition never became true (deadlock guard)");
}

/// P1 decision-gate evidence (step 1): run `add` over `host_count` fresh
/// hosts with every `connect` / `read_products` / `run` / `close` operation
/// and every probe gated behind one shared gate per phase, released only
/// once every host has observably entered that phase and drained only once
/// every host has observably left it.
///
/// P0's `join_all`-driven fan-out completes an ungated mock operation in a
/// single poll, so `peak_operations` reports `1` at any host count even
/// though the fan-out code path is exercised (see
/// `tests/performance/README.md`'s documented limitation). This proves the
/// harness itself can expose true fleet-wide concurrency at 20/100 hosts —
/// evidence that a later bounded-fan-out regression test
/// (`peak_operations <= cap`) measures something real rather than an
/// artifact of synchronous completion.
#[allow(dead_code)]
pub async fn run_fully_gated_add(host_count: usize) -> (ExitCode, MockMetricsSnapshot) {
    let metrics = MockMetrics::new();
    // This harness proves the *gate/metrics machinery* can expose true
    // fleet-wide concurrency (P1 step 1's evidence) — a concern distinct
    // from the now-implemented `host_operation_limit` cap itself (proven
    // separately by `mock::tests::bounded_run_all_never_exceeds_the_configured_limit`).
    // Override the limit so this harness keeps measuring the former.
    let mut group =
        MockHostGroup::new().with_host_operation_limit(NonZeroUsize::new(host_count).unwrap());
    let connect_gate = MockGate::new();
    let read_products_gate = MockGate::new();
    let run_gate = MockGate::new();
    let close_gate = MockGate::new();
    let probe_gate = MockGate::new();
    for i in 1..=host_count {
        let key = host_key(i, host_count);
        let host = MockHost::new(key)
            .with_products(sles_system())
            .with_metrics(metrics.clone())
            .with_gate(MockOpKind::Connect, connect_gate.clone())
            .with_gate(MockOpKind::ReadProducts, read_products_gate.clone())
            .with_gate(MockOpKind::Run, run_gate.clone())
            .with_gate(MockOpKind::Close, close_gate.clone());
        group.insert(host);
    }
    let opts = CommandOptions {
        config: sample_config(),
        repa: vec![Repa::parse(GATED_ADD_REPA).expect("valid REPA in scenario fixture")],
        no_probe: false,
        probe_timeout: Duration::from_secs(5),
        // Same reasoning as the host-operation limit override above: this
        // harness measures gate/metrics fidelity, not the (separately
        // tested) probe budget cap, so it must not itself become the
        // bottleneck at 100 hosts.
        probe_concurrency_limit: NonZeroUsize::new(host_count).unwrap(),
        ..Default::default()
    };
    let probe = MetricProbe::new(true)
        .with_metrics(metrics.clone())
        .with_gate(probe_gate.clone());

    let mut buf = Buffer::default();
    let mut console = Console::new(&mut buf);
    // `tokio::join!` (not `tokio::spawn`) runs the command and the gate
    // driver as sibling futures of the *same* task: `run_add`'s probe
    // fan-out borrows a `&dyn Probe` with a closure lifetime that is not
    // `'static`-general enough for `spawn`'s bound, and does not need to be
    // — both futures only need to interleave, not run on separate tasks.
    let driver_metrics = metrics.clone();
    let driver = async {
        // Every phase below shares one of two global counters
        // (`current_operations`/`current_probes`), and a released gate lets
        // its whole synchronous phase-transition (drain + next phase's
        // fill) run to completion inside a *single* poll of `command` — the
        // transient zero in between is never observable, and checking for
        // it stalls forever. One `yield_now` after each release forces
        // exactly one more poll of `command` before the next check, so the
        // next `wait_until` reads the *next* phase's fill rather than a
        // stale pre-release reading of the same counter.
        wait_until(|| driver_metrics.snapshot().current_operations >= host_count).await;
        connect_gate.release();
        tokio::task::yield_now().await;

        wait_until(|| driver_metrics.snapshot().current_operations >= host_count).await;
        read_products_gate.release();
        tokio::task::yield_now().await;

        wait_until(|| driver_metrics.snapshot().current_probes >= host_count).await;
        probe_gate.release();
        tokio::task::yield_now().await;

        wait_until(|| driver_metrics.snapshot().current_operations >= host_count).await;
        run_gate.release();
        tokio::task::yield_now().await;

        // The post-add cohort refresh (`run_all`) reuses the Run gate,
        // which is already released, so it drains without blocking; only
        // the close phase below still gates.
        wait_until(|| driver_metrics.snapshot().current_operations >= host_count).await;
        close_gate.release();
    };
    let command = async {
        run_add(&opts, &mut group, &probe, &mut console)
            .await
            .expect("template load")
    };
    let (code, ()) = tokio::join!(command, driver);
    (code, metrics.snapshot())
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
