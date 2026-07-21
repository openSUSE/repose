# Performance workloads, reports, and baselines

This directory defines `repose`'s representative performance workloads
(P0.1), the machine-readable report contract every workload produces, and
the committed baseline summaries used to catch regressions (P0.5).

## Layout

- `workloads.json` — the workload matrix: every workload's dimensions
  (command, host count, repeated URLs, a slow host, output format, debug)
  and its **reviewed expectation** (exit code, exact command/probe counts,
  a peak-concurrency ceiling, host ordering, and — where output is
  deterministic — a content digest). A workload's expectation is checked
  against the observed result *before* any timing is reported; an
  unreviewed behavior change fails the run instead of silently shipping a
  new number.
- `baselines/<runner-class>.json` — one compact, committed summary per
  runner identity (semantic counters + latency percentiles, no raw sample
  arrays or per-host output). `<runner-class>` is `local-dev` unless
  `REPOSE_PERF_RUNNER_CLASS` is set.
- `baselines/raw/` (gitignored) — the full per-workload reports the
  orchestration script produces, including every wall-time sample and
  (for small fleets) host ordering. Local/CI artifacts only.
- `opportunity-matrix.md` (P0.4) — the top profiled optimization
  candidates, ranked and evidence-linked.
- `comparator-fixtures/` (P0.5) — fixtures for `scripts/compare-performance.sh`.

## The report contract

Every report is a single JSON object; `scripts/validate-performance-report.jq`
checks its shape:

| Field                     | Meaning                                                                 |
| ------------------------- | ------------------------------------------------------------------------ |
| `contract_version`        | Always `1`.                                                             |
| `workload_id`              | Matches an id in `workloads.json`.                                      |
| `kind`                     | `"mock"` (in-process `repose-core` API) or `"ssh"` (real CLI + fixture). |
| `runner`                   | `{os, arch, toolchain, runner_class}`.                                  |
| `generated_at`             | RFC3339 UTC timestamp.                                                  |
| `repetitions` / `warmup_repetitions` | Measured / discarded warmup runs.                              |
| `wall_time_ns`             | One nanosecond sample per repetition (warmups excluded), ascending.     |
| `latency_ns.{p50,p95,p99}` | Nearest-rank percentiles over `wall_time_ns`.                            |
| `throughput_ops_per_sec`   | `host_count / (p50_latency_seconds)` — hosts processed per second at the median run. |
| `peak_rss_bytes`           | Process peak RSS for the whole measured run, or `null` if the platform collector was unavailable. |
| `command_count`            | Exact remote/mock commands completed (see `MockMetricsSnapshot::commands_completed`; for `ssh`-kind reports this is coarse — see Limits below). |
| `probe_count`              | Exact URL-liveness probes issued.                                       |
| `peak_concurrency`         | Maximum observed in-flight host operations.                            |
| `exit_code`                | Process/aggregate exit code.                                            |
| `stdout_digest` / `stderr_digest` | `"<algo>:<hex>"` content fingerprint — `fnv1a64` for `mock`-kind reports (deterministic, no new dependency; **not** cryptographic), `sha256` for `ssh`-kind reports. |
| `host_order`               | Host keys in aggregation order.                                        |

## Workload dimensions

`workloads.json` covers `add`, `install`, and `list-products` at 1/20/100
hosts, with variants for repeated repository URLs, one slow host, and
text/JSON output; `ssh`-kind entries add real transport, product
discovery, a dry-run preview, `--debug` verbose logging, and a
first-contact `accept-new` known_hosts scenario against the Docker OpenSSH
fixture (`tests/ssh/`).

Two dimensions are intentionally *not* simulated in the mock harness:

- **Slow host**: `slow_host: true` gates one host's `run` operation behind
  a real ~20ms delay (`support::SLOW_HOST_DELAY` in
  `crates/repose-core/benches/support/mod.rs`), spawned as a concurrent
  Tokio task. This is the one place a benchmark/report harness
  deliberately uses a real sleep — `repose_core::mock` itself stays
  sleep-free (deterministic gates only) for flake-free unit tests; the
  baseline report and Criterion bench explicitly measure wall time, so a
  small documented delay is the honest way to produce a measurable tail.
- **Debug/verbose output**: only meaningful for the compiled CLI (log
  level), so it is only a dimension on `ssh`-kind workloads.

### A known limitation: `peak_concurrency` in mock workloads

`MockHost`'s operations only yield to the async executor at an explicit
gate/barrier. Without one, `join_all`-driven fan-out completes each host's
future synchronously, so `peak_concurrency` reports `1` even though the
fan-out code path is exercised. Only the `slow-host` variants (which gate
one host) observe real overlap. This means the mock benchmark's
`peak_concurrency <= host_count` check is a safety ceiling, not proof that
fan-out is concurrent — that proof lives in `crates/repose-core/src/mock.rs`'s
own gated/barrier unit tests (P0.2) and `add.rs`'s `concurrent_hosts_overlap_in_run`.

## Running it

Prerequisites: a release build, `jq`, `rustc`; the `ssh`-kind workloads
additionally need `docker` and `ssh-keygen` (see `tests/ssh/run.sh`) — the
script skips them with a message if unavailable.

```sh
# Everything (mock workloads always run; ssh workloads run if Docker is available):
scripts/run-performance-baseline.sh

# Faster local iteration:
scripts/run-performance-baseline.sh --mock-reps 5 --mock-warmup 1 --skip-ssh

# One mock workload directly (skips RSS collection/toolchain metadata):
cargo build --release -p repose-core --example baseline_report
target/release/examples/baseline_report mock-add-100h 20 3 \
  | jq -e -f scripts/validate-performance-report.jq

# Fleet-scale Criterion benchmark (local algorithm overhead only —
# see crates/repose-core/benches/fleet.rs doc comment):
cargo bench -p repose-core --bench fleet --locked -- --test   # smoke: every ID, one sample
cargo bench -p repose-core --bench fleet --locked             # full statistical run
```

`scripts/run-performance-baseline.sh` builds once in release mode, runs
every `mock`-kind workload through `baseline_report` (which checks the
reviewed expectation on every repetition, then reports timing), runs every
`ssh`-kind workload against the Docker fixture via
`scripts/run-performance-baseline-ssh.sh`, validates every report against
the contract, writes raw per-workload reports to `--out` (default
`tests/performance/baselines/raw/`, gitignored), and finally writes the
compact committed summary to `tests/performance/baselines/<runner-class>.json`.

### Warmup / repetition policy

Mock workloads default to 3 warmup + 20 measured repetitions (fast,
in-process); `ssh`-kind workloads default to 1 warmup + 5 measured
repetitions (real network round trips per rep). Override with
`--mock-reps`/`--mock-warmup`/`--ssh-reps`/`--ssh-warmup`. Warmup
repetitions are still checked against the reviewed expectation (so a
warmup crash still fails loudly) but are excluded from `wall_time_ns`.

### Result interpretation

- `mock`-kind reports measure `repose-core`'s command algorithms against
  test doubles — no SSH/SFTP/zypper cost. Use them to catch local
  algorithmic regressions (allocation, redundant work, O(n²) loops) at
  fleet scale.
- `ssh`-kind reports exercise one real host through the fixture and are
  the only source of real transport/host-key timing in this baseline —
  they do not model true 100-host network fan-out (a single-container
  fixture cannot); use them for transport realism, not fleet-scaling
  claims (see the P0.1 plan's risk log).
- Compare same-runner-class reports only; `os`/`arch`/`toolchain` differ
  in ways that make cross-runner latency comparisons meaningless
  (`scripts/compare-performance.sh`, P0.5, enforces this).

### Platform notes for `peak_rss_bytes`

- macOS: `/usr/bin/time -l`, `maximum resident set size` (already bytes).
- Linux: GNU `time -v`, `Maximum resident set size` (KB, converted to
  bytes). Falls back to `null` if GNU `time` isn't installed (BusyBox/
  POSIX `time` doesn't support `-v`).

### Artifact retention

Raw per-workload reports (`--out`, default `tests/performance/baselines/raw/`)
and Criterion's own HTML/statistics output (`target/criterion/`) are
gitignored local/CI artifacts. Only the compact
`tests/performance/baselines/<runner-class>.json` summary and this
directory's definitions/docs are committed.

## Release profiling (P0.4)

`scripts/profile-performance.sh <workload-id>` builds once in release mode,
launches the `baseline_report` harness against one `mock`-kind workload
(a high repetition count so the process stays alive through the profiling
window — the profiler attaches after the measured loop is already
running, so process startup/build time is never attributed to `repose`),
runs the requested profile classes, and writes a normalized manifest to
`tests/performance/profiles/<id>.<os>.json` (validated against
`scripts/validate-performance-profile.jq`). Raw tool output goes to
`tests/performance/profiles/raw/` (gitignored).

```sh
# CPU + allocation (works without elevated privileges on both platforms):
scripts/profile-performance.sh mock-add-100h --classes cpu,alloc

# I/O (needs root on macOS/Linux for fs_usage/strace):
sudo scripts/profile-performance.sh mock-add-100h --classes io
```

A requested class that cannot run (missing tool, insufficient privilege,
unsupported OS) is recorded as `"status": "skipped"` with a `"reason"`, and
the script exits nonzero — it never reports a silent partial success.

### Prerequisites and supported tools

| Platform | CPU | Allocation | I/O |
| --- | --- | --- | --- |
| macOS | `sample` (built-in; no root) | `heap` (built-in; no root) — a **live footprint snapshot**, not a full allocation-count history | `fs_usage` (built-in; **requires root**) |
| Linux | `perf record`/`perf report` (needs `perf_event_paranoid` low enough, or `CAP_PERFMON`) | `heaptrack` (wraps a fresh launch — cannot attach to an already-running PID; the script documents the correct invocation instead of faking data) | `strace -c` (needs `CAP_SYS_PTRACE` / `ptrace_scope=0`) |

### Known comparability limits

- macOS `heap`/`leaks`-family tools report point-in-time live footprint, not
  a cumulative allocation count; only compare `heap` output to other `heap`
  output on the same platform, never to `heaptrack`'s cumulative counts.
- macOS `sample`'s "top of stack" view is flat — it cannot attribute
  allocator cost (`malloc`/`free`/`memmove`) to a specific caller without a
  full call-tree capture. Aggregate allocator percentages are reported as
  "the workload is allocation-bound", not as a ranked, actionable candidate
  (see `opportunity-matrix.md`).
- `perf`/`heaptrack`/`strace` numbers are not comparable across kernel
  versions or container/VM boundaries; compare within one controlled
  runner class, same as the report-contract percentiles above.
- The harness process itself (equivalence checking against
  `workloads.json`'s reviewed expectations) shows up in these profiles —
  see `opportunity-matrix.md`'s "excluded as harness artifact" note before
  ranking a symbol found only in `baseline_report`.

## Regression thresholds and the comparator (P0.5)

`scripts/compare-performance.sh <baseline.json> <candidate.json>` compares
two contract-valid reports for the *same* workload and runner class,
applying the rules in `tests/performance/thresholds.json`:

- **Exact** (`exit_code`, `command_count`, `probe_count`, `stdout_digest`,
  `stderr_digest`): any change is a regression. These are deterministic for
  `mock`-kind workloads, so there is no legitimate noise to tolerate.
- **Ceiling** (`peak_concurrency`): the candidate may not exceed the
  baseline's observed value.
- **Threshold** (`latency_ns.{p50,p95,p99}`, `throughput_ops_per_sec`,
  `peak_rss_bytes`): a variance-derived relative tolerance — see
  `thresholds.json` for the exact ratios and the repeated-run evidence each
  one cites. `peak_rss_bytes` is skipped (not failed) when either side is
  `null`.

Exit codes distinguish failure kinds: `0` pass (including a real
improvement), `1` regression, `2` contract failure (a report doesn't
satisfy the schema), `3` incomparable metadata (different
`workload_id`/`runner_class`, or below `minimum_repetitions`).

```sh
make perf-compare-test   # comparator-fixtures/*.json + one real injected-slowdown guardrail
scripts/compare-performance.sh before.json after.json
```

`scripts/test-compare-performance.sh` (`make perf-compare-test`) checks all
six fixture categories under `comparator-fixtures/` (pass, improvement,
regression, missing-metric, contract-failure, incomparable-metadata), then
runs one real end-to-end guardrail: two genuine `baseline_report` runs of
the same workload must compare as a pass, and a third run with
`REPOSE_PERF_INJECT_DELAY_MS` set (a real, controllable per-repetition
delay — not a fabricated fixture) must be caught as a regression.

### Why this isn't in CI yet

CI enforcement is intentionally deferred: the thresholds above come from a
handful of runs on one shared, uncontrolled development machine (see the
`evidence` fields in `thresholds.json`), not a dedicated/scheduled runner.
Shared CI runners are noisier still, so enabling latency/RSS gating there
without runner-specific variance data would either pass everything (too
loose) or flap on noise (too tight). Deterministic checks (the exact/ceiling
metrics above) are cheap to run anywhere and can gate PRs today via `make
perf-compare-test`; wall-clock/RSS gating should wait for a controlled or
scheduled runner with its own variance study, per `thresholds.json`'s
`baseline_refresh_policy`.

### Baseline update review requirements

A baseline is only replaced with reviewed evidence, never silently widened:

1. A stated reason (an approved optimization landed, an accepted
   regression trade-off, or a runner-class migration).
2. Linked before/after reports (and, for a claimed improvement/regression,
   the profile evidence from `scripts/profile-performance.sh` that
   explains it).
3. `scripts/compare-performance.sh` output showing what changed and by how
   much.
4. Explicit reviewer sign-off in the PR description.

This keeps a baseline change a small, auditable diff — not a bulk
regeneration that could quietly absorb a regression.
