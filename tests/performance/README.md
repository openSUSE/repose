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
  `REPOSE_PERF_RUNNER_CLASS` is set. One `runner` object covers every
  workload in the file — see "The container pair" below.
- `baselines/raw/` (gitignored) — the full per-workload reports the
  orchestration script produces, including every wall-time sample and
  (for small fleets) host ordering. Local/CI artifacts only.
- `runner/Dockerfile` — the pinned build+measure environment both `mock`
  and `ssh` workloads run in; see "The container pair" below.
- `opportunity-matrix.md` (P0.4) — the top profiled optimization
  candidates, ranked and evidence-linked.
- `comparator-fixtures/` (P0.5) — fixtures for `scripts/compare-performance.sh`.

## The report contract

Every report is a single JSON object; `scripts/validate-performance-report.jq`
checks its shape:

| Field                     | Meaning                                                                 |
| ------------------------- | ------------------------------------------------------------------------ |
| `contract_version`        | Always `2`.                                                             |
| `workload_id`              | Matches an id in `workloads.json`.                                      |
| `kind`                     | `"mock"` (in-process `repose-core` API) or `"ssh"` (real CLI + fixture). |
| `runner`                   | `{os, arch, toolchain, profile, rustflags, fixture_runtime, runner_image, runner_class}` — see field notes below and "The container pair". |
| `generated_at`             | RFC3339 UTC timestamp.                                                  |
| `host_count`               | Hosts the workload targets (from `workloads.json`).                     |
| `repetitions` / `warmup_repetitions` | Measured / discarded warmup runs.                              |
| `wall_time_ns`             | One nanosecond sample per repetition (warmups excluded), ascending.     |
| `latency_ns.{p50,p95,p99}` | Nearest-rank percentiles over `wall_time_ns`.                            |
| `throughput_ops_per_sec`   | `host_count / (p50_latency_seconds)` — hosts processed per second at the median run. |
| `peak_rss_bytes`           | Process peak RSS for the whole measured run, or `null` if the platform collector was unavailable. |
| `command_count`            | Exact remote/mock commands completed (see `MockMetricsSnapshot::commands_completed`), or `null` for a `kind == "ssh"` report — see "The null-counter policy" below. |
| `probe_count`              | Exact URL-liveness probes issued, or `null` for `kind == "ssh"`.        |
| `peak_concurrency`         | Maximum observed in-flight host operations, or `null` for `kind == "ssh"`. |
| `exit_code`                | Process/aggregate exit code.                                            |
| `stdout_digest` / `stderr_digest` | `"<algo>:<hex>"` content fingerprint — `fnv1a64` for `mock`-kind reports (deterministic, no new dependency; **not** cryptographic), `sha256` for `ssh`-kind reports (of the byte-exact, target-normalized stream — see "Digest normalization for `ssh`-kind reports"). `stderr_digest` may be `null` for a workload whose stderr provably cannot reproduce (declared as `expect.stderr_digest: null` with the reason in `workloads.json`; `stdout_digest` is never exempt). |
| `host_order`               | Host keys in aggregation order.                                        |

### `runner` field notes

- `os`/`arch`/`toolchain`: the measuring process's platform and exact
  `rustc --version`. Under the container pair these describe the runner
  container (always Linux), not the contributor's host.
- `profile`: always `"release"` — the harness only ever measures a release
  build.
- `rustflags`: the invoking shell's `RUSTFLAGS`, verbatim (may be empty).
- `fixture_runtime`: which container CLI built and ran the pair —
  `"docker"`, `"podman"`, or `"container"` — or `null` for a `--skip-ssh`
  host-native mock-only run, which never touches a container.
- `runner_image`: the runner image's digest (`sha256:...`), or `null`
  under `--skip-ssh`. This is what makes "these two runs were built the
  same way" checkable rather than asserted — see "The container pair".
- `runner_class`: the contributor/CI identity for this baseline
  (`REPOSE_PERF_RUNNER_CLASS`, default `local-dev`); names the committed
  `baselines/<runner-class>.json` file.

### The null-counter policy

`command_count`, `probe_count`, and `peak_concurrency` are `null` on a
`kind == "ssh"` report, never a fabricated or approximated number.
Observing them for real would mean instrumenting the fixture (wrapping its
fake `zypper`, counting SFTP opens) for metrics the deterministic `mock`
workloads already gate exactly; see "Alternatives considered" in
`plans/p0.1-repair-performance-measurement.md` for why that trade was
deferred rather than made. `scripts/compare-performance.sh` SKIPs a metric
that is `null` on either side instead of failing or silently comparing
`null` as if it were a number.

### Digest normalization for `ssh`-kind reports

`ssh`-kind stdout/stderr are hashed as the exact raw bytes (trailing
newlines included) after one substitution: the fixture's container address
and port (`<host>:<port>` and, for OpenSSH's own `known_hosts` bracket form
on a non-default port, `[<host>]:<port>`) are folded to a fixed placeholder
before hashing, since that address is a property of this run's container
network, not of `repose`'s behavior. The raw, unnormalized bytes are kept
under `$REPOSE_PERF_OUT/raw-io/<workload-id>/` for debugging a digest
mismatch. Every repetition's normalized digest is asserted equal to the
first; a mismatch fails the workload rather than reporting the last run's
digest as if it were representative.

## Workload dimensions

`workloads.json` covers `add`, `install`, and `list-products` at 1/20/100
hosts, with variants for repeated repository URLs, one slow host, and
text/JSON output; `ssh`-kind entries add real transport, product
discovery, a dry-run preview, `--debug` verbose logging, and a
first-contact `accept-new` known_hosts scenario against the OpenSSH
fixture (`tests/ssh/`), reached over the container pair described below.

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

Prerequisites: `jq`, `ssh-keygen`, and a live container runtime — Docker,
Podman, or Apple's `container` (macOS-native). `--skip-ssh` needs none of
the container tooling; it also needs a local Rust toolchain and skips the
`ssh`-kind workloads entirely.

```sh
# Everything, inside the container pair (see below):
scripts/run-performance-baseline.sh

# Faster local iteration — host-native, mock workloads only, no container:
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
`ssh`-kind workload against the fixture via
`scripts/run-performance-baseline-ssh.sh`, validates every report against
the contract, writes raw per-workload reports to `--out` (default
`tests/performance/baselines/raw/`, gitignored), and finally writes the
compact committed summary to `tests/performance/baselines/<runner-class>.json`
(validated against `scripts/validate-performance-summary.jq`).

## The container pair

Everything but `--skip-ssh` runs inside two containers on one private
network: the runner (`tests/performance/runner/Dockerfile` — the pinned
build+measure environment) and the fixture (the unmodified
`tests/ssh/Dockerfile`, container-to-container only, no published port).
This is deliberate, not incidental — see the "Decisions taken up front"
and assumptions 6 and 7 in `plans/p0.1-repair-performance-measurement.md`:

- repose is deployed on Linux, so that is the environment worth measuring
  — not the contributor's laptop OS and toolchain.
- A pinned image is what makes two runs a month apart share a toolchain, a
  libc and a set of system packages; without it, `local-dev` numbers from
  two contributors (or one contributor a month apart) are not the same
  baseline.
- On at least one development machine, host→container networking does not
  work at all (a published port accepts and then resets; a container IP
  never resolves) while container→container does — so the fixture is
  never published to the host, and its address/readiness is resolved and
  proven reachable from inside the runner container.
- One `runner` object describes a whole committed summary, so `mock` and
  `ssh` workloads are measured in the *same* container run — a mock
  workload staying host-native next to a containerized `ssh` workload
  would make that one object's fields describe only some of what it
  claims to cover.

Select the runtime with `REPOSE_PERF_CONTAINER_CLI`: `auto` (default)
tries `docker`, then `podman`, then `container` in turn, with a liveness
call for each (a present binary with a dead daemon counts as
unavailable). An explicit value that isn't live fails loudly instead of
silently trying another runtime — a baseline measured somewhere else is
not the same baseline. There is no host-native fallback for the full run.

`runner.runner_image` (the runner image's digest) is what makes "these two
runs were built the same way" checkable: it changes whenever the image is
rebuilt (`rust-toolchain.toml` resolving `stable` to a new version, or the
Dockerfile itself changing), and `scripts/compare-performance.sh` rejects
comparing across a mismatch unless `--allow-cross-runner` is passed. It
pins the OS, libc, toolchain and system packages the measurement ran
with; it deliberately does **not** pin the host's CPU, kernel, or whether
the container gets its own VM (Apple `container`) or shares the host
kernel (Docker/Podman on Linux) — those remain free, which is why
`runner.arch` and `runner.runner_class` are checked separately, and why a
macOS `local-dev` figure is a regression detector for that machine, never
a proxy for a Linux CI runner.

`REPOSE_PERF_CACHE_DIR` (default `$HOME/.cache/repose-perf`) is a
persistent cargo registry/git/target cache bind-mounted into the runner,
kept outside the repository so a rebuild doesn't start from zero.

### Warmup / repetition policy

Mock workloads default to 3 warmup + 20 measured repetitions (fast,
in-process); `ssh`-kind workloads default to 2 warmup + 10 measured
repetitions (real network round trips per rep) — enough to clear
`thresholds.json`'s `minimum_repetitions` (10) without being asked.
Override with `--mock-reps`/`--mock-warmup`/`--ssh-reps`/`--ssh-warmup`.
Warmup repetitions are still checked against the reviewed expectation (so
a warmup crash still fails loudly) but are excluded from `wall_time_ns`.

### Result interpretation

- `mock`-kind reports measure `repose-core`'s command algorithms against
  test doubles — no SSH/SFTP/zypper cost. Use them to catch local
  algorithmic regressions (allocation, redundant work, O(n²) loops) at
  fleet scale.
- `ssh`-kind reports exercise one real host through the fixture and are
  the only source of real transport/host-key timing in this baseline —
  they do not model true 100-host network fan-out (a single-container
  fixture cannot); use them for transport realism, not fleet-scaling
  claims. Under Apple `container`, each container is its own VM with a
  real network hop between client and fixture, so absolute `ssh`-kind
  latency there is not comparable to a Docker/Podman run on Linux, where
  the pair shares the host kernel — `runner.fixture_runtime` records
  which applies.
- Compare same-runner-class reports only; `os`/`arch`/`toolchain`/
  `profile`/`rustflags`/`fixture_runtime`/`runner_image` differ in ways
  that make cross-runner latency comparisons meaningless
  (`scripts/compare-performance.sh`, P0.5, enforces this).

### Platform notes for `peak_rss_bytes`

- Inside the container pair: GNU `time -v`, `Maximum resident set size`
  (KB, converted to bytes) — the runner image always has it installed.
- `--skip-ssh` (host-native): macOS uses `/usr/bin/time -l`, `maximum
  resident set size` (already bytes); Linux uses GNU `time -v` if
  present, else falls back to `null` (BusyBox/POSIX `time` doesn't
  support `-v`).

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
two contract-valid reports for the *same* workload and runner, applying
the rules in `tests/performance/thresholds.json`:

- **Exact** (`exit_code`, `command_count`, `probe_count`, `stdout_digest`,
  `stderr_digest`): any change is a regression. These are deterministic for
  `mock`-kind workloads, so there is no legitimate noise to tolerate. A
  metric that is `null` on either side is SKIPped, not compared — see "The
  null-counter policy" above.
- **Ceiling** (`peak_concurrency`): the candidate may not exceed the
  baseline's observed value. Also SKIPped when either side is `null`.
- **Threshold** (`latency_ns.{p50,p95,p99}`, `throughput_ops_per_sec`,
  `peak_rss_bytes`): a variance-derived relative tolerance — see
  `thresholds.json` for the exact ratios and the repeated-run evidence each
  one cites. Skipped when either side is `null`. `latency_ns.p99` is
  additionally SKIPped below `tail_minimum_repetitions` (100): a
  nearest-rank p99 drawn from, say, 20 repetitions is just the 2nd-slowest
  run and too noisy a single sample to gate on; `p50`/`p95` still gate at
  the ordinary `minimum_repetitions` (10).

Before any metric check, the comparator rejects a mismatch in
`contract_version`, `runner.os`, `.arch`, `.toolchain`, `.profile`,
`.rustflags`, `.fixture_runtime`, `.runner_image`, and `.runner_class`
with exit `3` — pass `--allow-cross-runner` to compare anyway. With the
pinned container image (see "The container pair"), everything but
`.arch`, `.runner_class`, and `.fixture_runtime` should now match across
contributors; a `.runner_image` mismatch is the cheapest single check for
"these two runs were not built the same way".

Exit codes distinguish failure kinds: `0` pass (including a real
improvement), `1` regression, `2` contract failure (a report, or an
extracted `--workload` entry, doesn't satisfy its contract), `3`
incomparable metadata (different `workload_id`, a runner/environment field
mismatch, a `--workload` id absent from a summary, or below
`minimum_repetitions`).

```sh
make perf-compare-test   # comparator-fixtures/*.json + one real injected-slowdown guardrail
scripts/compare-performance.sh before.json after.json

# Compare one workload straight out of the committed summary — no raw
# per-workload report needed on the baseline side:
scripts/compare-performance.sh --workload mock-add-100h \
  tests/performance/baselines/local-dev.json tests/performance/baselines/raw/mock-add-100h.json

# Deterministic checks only, runner-independent (what the `perf-semantic`
# job in .github/workflows/ci.yml runs on every PR): implies
# --allow-cross-runner and skips the repetition-count gate.
scripts/compare-performance.sh --semantic-only --workload mock-add-100h \
  tests/performance/baselines/local-dev.json fresh-report.json
```

`--workload <id>` extracts one workload's data from either input. A
standalone report (like the ones under `baselines/raw/`) is used as-is,
validated against the full report contract
(`scripts/validate-performance-report.jq`). A compact baseline summary
(like `baselines/<runner-class>.json`) has its `<id>` entry merged with
the summary's `runner`/`contract_version` and validated against the
*comparable subset* of the contract
(`scripts/validate-performance-comparable.jq`) — a summary entry has no
`wall_time_ns` samples, so there is nothing to recompute percentiles from.
`scripts/validate-performance-summary.jq` checks the same subset for every
entry in a whole summary file, plus the summary's own shape
(`contract_version`, `runner`, unique `workload_id`s); both are jq
*libraries* (`include`d, not run standalone) invoked with `-L scripts`.

`scripts/test-compare-performance.sh` (`make perf-compare-test`) checks
every fixture category under `comparator-fixtures/` (pass, improvement,
regression, missing-metric, null-metric, toolchain-mismatch,
tail-reps-skip, contract-failure, incomparable-metadata), then runs one
real end-to-end guardrail: two genuine `baseline_report` runs of the same
workload must compare as a pass, and a third run with
`REPOSE_PERF_INJECT_DELAY_MS` set (a real, controllable per-repetition
delay — not a fabricated fixture) must be caught as a regression.

### What's in CI, and why latency/RSS gating still isn't

Two workflows run the harness:

- `.github/workflows/ci.yml`'s `perf-semantic` job, on every PR/push: the
  same container pair described above, PR-sized (`--mock-reps 10
  --mock-warmup 2 --ssh-reps 3 --ssh-warmup 1`), compared against the
  committed `tests/performance/baselines/local-dev.json` with
  `--semantic-only` — exact + ceiling metrics only, runner-independent, so
  it blocks a PR that changes a command count or an observable digest.
- `.github/workflows/perf.yml`'s `full-baseline` job, weekly (and
  `workflow_dispatch`): the full-sized run (mock 100/10, ssh 20/3),
  tagged `runner_class: github-ubuntu`, uploaded as artifacts and
  compared against a committed `tests/performance/baselines/github-ubuntu.json`.
  Its semantic-metrics comparison blocks the same way; its latency/RSS
  comparison is `continue-on-error: true`.

Latency/RSS gating stays deferred there because the thresholds above come
from a handful of runs on one shared, uncontrolled development machine
(see the `evidence` fields in `thresholds.json`), not `github-ubuntu`.
Shared CI runners are noisier still, so gating on them without
runner-specific variance data would either pass everything (too loose) or
flap on noise (too tight). The weekly job exists in part to collect that
data: once it has supplied five runs' worth, `thresholds.json` gets
runner-class-specific ratios (per its `baseline_refresh_policy`) and its
latency/RSS comparison flips to blocking.

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
