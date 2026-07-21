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

use criterion::{Criterion, criterion_group, criterion_main};
use repose_core::commands::{run_add, run_install, run_list_products};
use repose_core::console::{Buffer, Console};
use repose_core::traits::HostGroup;
use support::{Scenario, ScenarioConfig, build_list_products_scenario, build_scenario, runtime};

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

criterion_group!(benches, bench_add, bench_install, bench_list_products);
criterion_main!(benches);
