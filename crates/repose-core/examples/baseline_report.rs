//! P0.1 baseline-report harness for the `mock`-kind workloads in
//! `tests/performance/workloads.json`. Emits one contract-valid JSON report
//! (see `tests/performance/README.md`) to stdout for a single workload ID.
//!
//! `ssh`-kind workloads are driven separately by
//! `scripts/run-performance-baseline.sh` against the Docker OpenSSH
//! fixture, since they exercise the compiled CLI binary and real transport
//! rather than this crate's public command API.
//!
//! Usage: `baseline_report <workload-id> [repetitions] [warmup]`
//!
//! Every observed result is checked against the workload's reviewed
//! `expect` block *before* any timing is printed — a changed command
//! history, count, or exit code fails the run instead of silently shipping
//! a report (P0.1 plan item 3 / P0.3 plan item 4).

#[path = "../benches/support/mod.rs"]
mod support;

use repose_core::commands::{run_add, run_install, run_list_products};
use repose_core::console::{Buffer, Console, OutputFormat};
use repose_core::mock::MockMetricsSnapshot;
use repose_core::traits::HostGroup;
use repose_core::types::ExitCode;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Instant;
use support::{Scenario, ScenarioConfig, build_list_products_scenario, build_scenario, runtime};

#[derive(Debug, Deserialize)]
struct WorkloadFile {
    workloads: Vec<Workload>,
}

#[derive(Debug, Deserialize)]
struct Workload {
    id: String,
    kind: String,
    command: String,
    host_count: usize,
    #[serde(default)]
    repa: Vec<String>,
    #[serde(default)]
    repeated_urls: bool,
    #[serde(default)]
    slow_host: bool,
    #[serde(default)]
    output_format: String,
    expect: Expect,
}

#[derive(Debug, Deserialize)]
struct Expect {
    exit_code: i32,
    // Mock-kind-only fields; absent (and unchecked) on `ssh`-kind entries,
    // which this binary never runs (see `scripts/run-performance-baseline.sh`).
    #[serde(default)]
    command_count: Option<usize>,
    #[serde(default)]
    probe_count: Option<usize>,
    #[serde(default)]
    peak_operations_max: Option<usize>,
    #[serde(default)]
    host_order: Option<Vec<String>>,
    #[serde(default)]
    stdout_digest: Option<String>,
}

/// One command outcome, checked against `Expect` before timing is trusted.
struct Observed {
    exit_code: i32,
    stdout: String,
    metrics: MockMetricsSnapshot,
    host_order: Vec<String>,
}

fn workloads_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/performance/workloads.json")
}

fn load_workload(id: &str) -> Workload {
    let raw = std::fs::read_to_string(workloads_path())
        .unwrap_or_else(|e| panic!("read {}: {e}", workloads_path().display()));
    let file: WorkloadFile = serde_json::from_str(&raw).expect("parse workloads.json");
    file.workloads
        .into_iter()
        .find(|w| w.id == id)
        .unwrap_or_else(|| panic!("no workload {id:?} in {}", workloads_path().display()))
}

fn output_format(s: &str) -> OutputFormat {
    match s {
        "json" => OutputFormat::Json,
        "text" => OutputFormat::Text,
        other => panic!("unknown output_format {other:?}"),
    }
}

/// Deterministic, non-cryptographic content fingerprint (FNV-1a/64) used
/// only for change detection between reviewed expectations and observed
/// output — stable across Rust toolchains by construction (hand-rolled,
/// not `DefaultHasher`). Tagged `"fnv1a64:<hex>"` so the report's digest
/// fields can also hold `"sha256:<hex>"` from the `ssh`-kind bash harness
/// (see `tests/performance/README.md`) without an algorithm mismatch.
fn fnv1a64(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn scenario_config(w: &Workload) -> ScenarioConfig {
    let mut cfg = ScenarioConfig::new(w.host_count);
    cfg.slow_host = w.slow_host;
    cfg.repeated_urls = w.repeated_urls;
    cfg.output_format = output_format(&w.output_format);
    cfg
}

fn repa_refs(w: &Workload) -> Vec<&str> {
    w.repa.iter().map(String::as_str).collect()
}

/// Run one repetition of `workload` and return the observed outcome.
/// Controllable slowdown for P0.5's end-to-end comparator guardrail test
/// (`scripts/test-compare-performance.sh`): with `REPOSE_PERF_INJECT_DELAY_MS`
/// set, every repetition sleeps that long before running the command, so a
/// real slowed run — not just a static fixture — can be shown crossing the
/// regression threshold. Unset in normal use; has no effect by default.
fn injected_delay() -> std::time::Duration {
    std::env::var("REPOSE_PERF_INJECT_DELAY_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map(std::time::Duration::from_millis)
        .unwrap_or_default()
}

fn run_once(rt: &tokio::runtime::Runtime, w: &Workload) -> Observed {
    let cfg = scenario_config(w);
    let repa = repa_refs(w);
    rt.block_on(async {
        let delay = injected_delay();
        if delay > std::time::Duration::ZERO {
            tokio::time::sleep(delay).await;
        }
        match w.command.as_str() {
            "add" | "install" => {
                let scenario = build_scenario(&cfg, &repa);
                scenario.arm_slow_host();
                let Scenario {
                    mut group,
                    metrics,
                    probe,
                    opts,
                    ..
                } = scenario;
                let mut buf = Buffer::default();
                let mut console = Console::new(&mut buf);
                console.format = opts.format;
                let exit_code = if w.command == "add" {
                    run_add(&opts, &mut group, &probe, &mut console)
                        .await
                        .expect("template load")
                } else {
                    run_install(&opts, &mut group, &probe, &mut console)
                        .await
                        .expect("template load")
                };
                Observed {
                    exit_code: exit_code.as_i32(),
                    stdout: buf.0,
                    metrics: metrics.snapshot(),
                    host_order: group.keys(),
                }
            }
            "list-products" => {
                let Scenario {
                    mut group,
                    metrics,
                    opts,
                    ..
                } = build_list_products_scenario(&cfg);
                let mut buf = Buffer::default();
                let exit_code: ExitCode = run_list_products(&opts, &mut group, &mut buf).await;
                Observed {
                    exit_code: exit_code.as_i32(),
                    stdout: buf.0,
                    metrics: metrics.snapshot(),
                    host_order: group.keys(),
                }
            }
            other => panic!("unknown mock command {other:?}"),
        }
    })
}

/// Fail fast (before any timing is printed) on the first deviation from the
/// reviewed expectation.
fn check_expectation(id: &str, expect: &Expect, observed: &Observed) {
    assert_eq!(
        observed.exit_code, expect.exit_code,
        "{id}: exit code changed"
    );
    let command_count = expect
        .command_count
        .unwrap_or_else(|| panic!("{id}: mock workload missing expect.command_count"));
    let probe_count = expect
        .probe_count
        .unwrap_or_else(|| panic!("{id}: mock workload missing expect.probe_count"));
    let peak_operations_max = expect
        .peak_operations_max
        .unwrap_or_else(|| panic!("{id}: mock workload missing expect.peak_operations_max"));
    let host_order = expect
        .host_order
        .as_ref()
        .unwrap_or_else(|| panic!("{id}: mock workload missing expect.host_order"));

    assert_eq!(
        observed.metrics.commands_completed, command_count,
        "{id}: command count changed"
    );
    assert_eq!(
        observed.metrics.probe_total, probe_count,
        "{id}: probe count changed"
    );
    assert!(
        observed.metrics.peak_operations <= peak_operations_max,
        "{id}: peak concurrency {} exceeds bound {}",
        observed.metrics.peak_operations,
        peak_operations_max
    );
    assert_eq!(
        observed.metrics.current_operations, 0,
        "{id}: leaked in-flight operation"
    );
    assert_eq!(
        observed.metrics.current_probes, 0,
        "{id}: leaked in-flight probe"
    );
    if !host_order.is_empty() {
        assert_eq!(
            &observed.host_order, host_order,
            "{id}: host ordering changed"
        );
    }
    if let Some(want) = &expect.stdout_digest {
        let got = fnv1a64(observed.stdout.as_bytes());
        assert_eq!(&got, want, "{id}: stdout content changed");
    }
}

fn percentile(sorted_ns: &[u128], pct: f64) -> u128 {
    // Nearest-rank method.
    let n = sorted_ns.len();
    let rank = ((pct / 100.0) * n as f64).ceil() as usize;
    sorted_ns[rank.clamp(1, n) - 1]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let id = args.next().unwrap_or_else(|| {
        eprintln!("usage: baseline_report <workload-id> [repetitions] [warmup]");
        eprintln!("       baseline_report --dump <workload-id>   (print observed counters, skip expectation check)");
        std::process::exit(2);
    });

    if id == "--dump" {
        let id = args.next().expect("--dump requires a workload id");
        let workload = load_workload(&id);
        assert_eq!(workload.kind, "mock", "{id}: not a mock-kind workload");
        let rt = runtime();
        let observed = run_once(&rt, &workload);
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "exit_code": observed.exit_code,
                "command_count": observed.metrics.commands_completed,
                "probe_count": observed.metrics.probe_total,
                "peak_operations": observed.metrics.peak_operations,
                "current_operations": observed.metrics.current_operations,
                "current_probes": observed.metrics.current_probes,
                "host_order": observed.host_order,
                "stdout_digest": fnv1a64(observed.stdout.as_bytes()),
                "stdout_preview": observed.stdout.lines().take(5).collect::<Vec<_>>(),
            }))
            .unwrap()
        );
        return;
    }

    let repetitions: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(20);
    let warmup: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(3);
    assert!(repetitions >= 1, "repetitions must be >= 1");

    let workload = load_workload(&id);
    assert_eq!(workload.kind, "mock", "{id}: not a mock-kind workload");

    let rt = runtime();

    for _ in 0..warmup {
        let observed = run_once(&rt, &workload);
        check_expectation(&id, &workload.expect, &observed);
    }

    let mut samples_ns: Vec<u128> = Vec::with_capacity(repetitions);
    let mut last: Option<Observed> = None;
    for _ in 0..repetitions {
        let start = Instant::now();
        let observed = run_once(&rt, &workload);
        samples_ns.push(start.elapsed().as_nanos());
        check_expectation(&id, &workload.expect, &observed);
        last = Some(observed);
    }
    let last = last.expect("repetitions >= 1");
    samples_ns.sort_unstable();

    let p50 = percentile(&samples_ns, 50.0);
    let p95 = percentile(&samples_ns, 95.0);
    let p99 = percentile(&samples_ns, 99.0);
    let throughput = workload.host_count as f64 / (p50 as f64 / 1e9);

    let report = serde_json::json!({
        "contract_version": 1,
        "workload_id": id,
        "kind": "mock",
        "runner": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            // Filled in by scripts/run-performance-baseline.sh (`rustc
            // --version`), which knows the invoking toolchain; this binary
            // only knows its own target triple constants.
            "toolchain": serde_json::Value::Null,
        },
        "repetitions": repetitions,
        "warmup_repetitions": warmup,
        "wall_time_ns": samples_ns,
        "latency_ns": { "p50": p50, "p95": p95, "p99": p99 },
        "throughput_ops_per_sec": throughput,
        "peak_rss_bytes": serde_json::Value::Null,
        "command_count": last.metrics.commands_completed,
        "probe_count": last.metrics.probe_total,
        "peak_concurrency": last.metrics.peak_operations,
        "exit_code": last.exit_code,
        "stdout_digest": fnv1a64(last.stdout.as_bytes()),
        "stderr_digest": fnv1a64(b""),
        "host_order": last.host_order,
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
