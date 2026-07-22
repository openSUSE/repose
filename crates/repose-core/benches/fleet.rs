//! P0.3 fleet-scale benchmark: `run_add` / `run_install` / `run_list_products`
//! at 1/20/100 hosts, including repeated-URL and slow-host variants.
//!
//! Labels honestly: this measures local command-algorithm overhead against
//! [`repose_core::mock`] test doubles, not remote SSH/SFTP/zypper latency
//! (see `tests/performance/README.md`). Every scenario is fresh per
//! iteration and checked against the reviewed expectations in
//! `tests/performance/workloads.json` before Criterion records a sample —
//! an unreviewed behavior change fails the benchmark run rather than
//! silently publishing a new number.

mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use repose_core::commands::{run_add, run_install, run_list_products};
use repose_core::console::{Buffer, Console};
use repose_core::traits::HostGroup;
use std::time::Duration;
use support::{
    Scenario, ScenarioConfig, build_list_products_scenario, build_scenario, run_fully_gated_add,
    runtime,
};

/// Reviewed baseline REPA for the add/install fleet scenarios (see
/// `tests/vectors/template/sample.yml`: `SLES."15-SP3".update`).
const ADD_REPA: &str = "SLES:15-SP3:x86_64:update";

fn bench_add(c: &mut Criterion) {
    let rt = runtime();
    let mut group = c.benchmark_group("fleet_add");
    // 100-host iterations cost more (more mock work, and the slow-host
    // variant carries a real ~20ms delay); a smaller sample count keeps a
    // full `cargo bench` run within a reasonable wall-clock budget.
    group.sample_size(20);
    for hosts in [1usize, 20, 100] {
        group.bench_function(format!("{hosts}h"), |b| {
            b.iter(|| {
                let cfg = ScenarioConfig::new(hosts);
                let Scenario {
                    mut group,
                    metrics,
                    probe,
                    opts,
                    ..
                } = build_scenario(&cfg, &[ADD_REPA]);
                rt.block_on(async {
                    let mut buf = Buffer::default();
                    let mut console = Console::new(&mut buf);
                    let code = run_add(&opts, &mut group, &probe, &mut console)
                        .await
                        .expect("template load");
                    assert_eq!(
                        code.as_i32(),
                        0,
                        "fleet_add/{hosts}h regressed to non-zero exit"
                    );
                    let snap = metrics.snapshot();
                    assert_eq!(snap.current_operations, 0, "leaked in-flight operation");
                    assert_eq!(snap.current_probes, 0, "leaked in-flight probe");
                    assert!(
                        snap.peak_operations <= hosts,
                        "peak concurrency {} exceeds host count {hosts}",
                        snap.peak_operations
                    );
                });
            });
        });
    }
    group.bench_function("100h_repeated_urls", |b| {
        b.iter(|| {
            let mut cfg = ScenarioConfig::new(100);
            cfg.repeated_urls = true;
            let Scenario {
                mut group,
                probe,
                opts,
                ..
            } = build_scenario(&cfg, &[ADD_REPA]);
            rt.block_on(async {
                let mut buf = Buffer::default();
                let mut console = Console::new(&mut buf);
                let code = run_add(&opts, &mut group, &probe, &mut console)
                    .await
                    .expect("template load");
                assert_eq!(code.as_i32(), 0);
            });
        });
    });
    group.bench_function("100h_slow_host", |b| {
        b.iter(|| {
            let mut cfg = ScenarioConfig::new(100);
            cfg.slow_host = true;
            let mut scenario = build_scenario(&cfg, &[ADD_REPA]);
            rt.block_on(async {
                scenario.arm_slow_host();
                let mut buf = Buffer::default();
                let mut console = Console::new(&mut buf);
                let code = run_add(
                    &scenario.opts,
                    &mut scenario.group,
                    &scenario.probe,
                    &mut console,
                )
                .await
                .expect("template load");
                assert_eq!(code.as_i32(), 0);
            });
        });
    });
    group.finish();
}

fn bench_install(c: &mut Criterion) {
    let rt = runtime();
    let mut group = c.benchmark_group("fleet_install");
    group.sample_size(20);
    for hosts in [1usize, 20, 100] {
        group.bench_function(format!("{hosts}h"), |b| {
            b.iter(|| {
                let cfg = ScenarioConfig::new(hosts);
                let Scenario {
                    mut group,
                    metrics,
                    probe,
                    opts,
                    ..
                } = build_scenario(&cfg, &[ADD_REPA]);
                rt.block_on(async {
                    let mut buf = Buffer::default();
                    let mut console = Console::new(&mut buf);
                    let code = run_install(&opts, &mut group, &probe, &mut console)
                        .await
                        .expect("template load");
                    assert_eq!(
                        code.as_i32(),
                        0,
                        "fleet_install/{hosts}h regressed to non-zero exit"
                    );
                    let snap = metrics.snapshot();
                    assert_eq!(snap.current_operations, 0, "leaked in-flight operation");
                    assert!(snap.peak_operations <= hosts);
                });
            });
        });
    }
    group.finish();
}

fn bench_list_products(c: &mut Criterion) {
    let rt = runtime();
    let mut group = c.benchmark_group("fleet_list_products");
    for hosts in [1usize, 20, 100] {
        group.bench_function(format!("{hosts}h"), |b| {
            b.iter(|| {
                let cfg = ScenarioConfig::new(hosts);
                let Scenario {
                    mut group,
                    metrics,
                    opts,
                    ..
                } = build_list_products_scenario(&cfg);
                rt.block_on(async {
                    let mut buf = Buffer::default();
                    let code = run_list_products(&opts, &mut group, &mut buf).await;
                    assert_eq!(code.as_i32(), 0);
                    let snap = metrics.snapshot();
                    assert_eq!(snap.current_operations, 0, "leaked in-flight operation");
                    assert!(snap.peak_operations <= hosts);
                    assert_eq!(group.keys().len(), hosts);
                });
            });
        });
    }
    group.finish();
}

/// P1 decision-gate evidence (step 1): every host's connect/read_products/
/// probe/run/close is gated behind one shared gate per phase, released only
/// once the whole fleet has observably entered it — proving the mock
/// harness exposes true fleet-wide concurrency at 20/100 hosts rather than
/// the misleading `peak_operations == 1` an ungated `join_all` produces in
/// one poll (see `tests/performance/README.md`).
fn bench_gated_fleet_admission(c: &mut Criterion) {
    let rt = runtime();
    let mut group = c.benchmark_group("fleet_gated_admission");
    group.sample_size(10);
    for hosts in [20usize, 100] {
        group.bench_function(format!("{hosts}h"), |b| {
            b.iter(|| {
                rt.block_on(async {
                    let (code, snap) = run_fully_gated_add(hosts).await;
                    assert_eq!(code.as_i32(), 0, "fleet_gated_admission/{hosts}h regressed");
                    assert_eq!(
                        snap.peak_operations, hosts,
                        "gated harness did not expose full {hosts}-host fleet width"
                    );
                    assert_eq!(snap.current_operations, 0, "leaked in-flight operation");
                    assert_eq!(snap.current_probes, 0, "leaked in-flight probe");
                });
            });
        });
    }
    group.finish();
}

/// P1 decision-gate evidence (step 1): admit `host_count` synthetic
/// operations of fixed cost `op_cost` through a bounded unordered stream at
/// `cap` concurrent slots. Independent of any mock/SSH implementation —
/// this is the plain queueing-theory curve (`~ceil(host_count / cap) *
/// op_cost`) that informs how low a host-operation cap can go before it
/// materially serializes a 100-host fleet. See
/// `tests/performance/p1-limit-decision.md` for the `op_cost` calibration
/// against measured SSH connect+auth+command latency and the resulting
/// table of caps.
async fn bounded_fanout_cost(host_count: usize, op_cost: Duration, cap: usize) {
    use futures_util::StreamExt;
    futures_util::stream::iter(0..host_count)
        .map(|_| async move { tokio::time::sleep(op_cost).await })
        .buffer_unordered(cap)
        .collect::<Vec<()>>()
        .await;
}

fn bench_concurrency_cap_sweep(c: &mut Criterion) {
    let rt = runtime();
    // Calibrated against the measured local-dev SSH fixture connect+auth+
    // one-command p50 (see tests/performance/p1-limit-decision.md); a
    // synthetic sleep keeps the sweep independent of any real transport.
    const OP_COST: Duration = Duration::from_millis(5);
    let mut sweep = c.benchmark_group("fleet_concurrency_cap_sweep_100h");
    sweep.sample_size(10);
    for cap in [1usize, 4, 8, 16, 32, 64, 100] {
        sweep.bench_with_input(BenchmarkId::from_parameter(cap), &cap, |b, &cap| {
            b.iter(|| {
                rt.block_on(bounded_fanout_cost(100, OP_COST, cap));
            });
        });
    }
    sweep.finish();
}

criterion_group!(
    benches,
    bench_add,
    bench_install,
    bench_list_products,
    bench_gated_fleet_admission,
    bench_concurrency_cap_sweep
);
criterion_main!(benches);
