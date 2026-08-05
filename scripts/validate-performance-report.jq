# Structural validator for the P0.1 performance-report contract
# (tests/performance/README.md documents each field's meaning).
#
# Usage:
#   jq -e -f scripts/validate-performance-report.jq report.json
#
# Exits 0 (and prints the input unchanged) when the report is well-formed;
# `error(...)` aborts with a nonzero exit and the message on stderr on the
# first violation. This checks *shape*, not whether the numbers are good —
# reviewed-expectation equivalence is enforced separately, before timing is
# ever printed (see `crates/repose-core/examples/baseline_report.rs` for
# `kind: "mock"` reports, and `scripts/run-performance-baseline.sh` for
# `kind: "ssh"` reports).

def check(val; pred; msg):
  if (val | pred) then . else error("invalid report: " + msg) end;

def is_nonneg_int: type == "number" and (. == (. | floor)) and . >= 0;
def is_nonneg_num: type == "number" and . >= 0;
def is_nonempty_string: type == "string" and length > 0;

# Nearest-rank percentile, matching both emitters
# (crates/repose-core/examples/baseline_report.rs's `percentile` and
# scripts/run-performance-baseline-ssh.sh's `percentile_ns`): rank =
# ceil((pct/100) * n), 1-indexed into the ascending sample array.
def nearest_rank(sorted; pct):
  (sorted | length) as $n
  | sorted[(((pct / 100) * $n) | ceil) - 1];

. as $r
| check($r.contract_version; . == 2; "contract_version must be 2")
| check($r.workload_id; is_nonempty_string; "workload_id must be a non-empty string")
| check($r.kind; . == "mock" or . == "ssh"; "kind must be \"mock\" or \"ssh\"")
| check($r.runner; type == "object"; "runner must be an object")
| check($r.runner.os; is_nonempty_string; "runner.os must be a non-empty string")
| check($r.runner.arch; is_nonempty_string; "runner.arch must be a non-empty string")
| check($r.runner.profile; is_nonempty_string; "runner.profile must be a non-empty string")
| check($r.runner.rustflags; type == "string"; "runner.rustflags must be a string (may be empty)")
| check(
    $r.runner.fixture_runtime;
    . == null or is_nonempty_string;
    "runner.fixture_runtime must be a non-empty string, or null before the container pair resolves it"
  )
| check(
    $r.runner.runner_image;
    . == null or is_nonempty_string;
    "runner.runner_image must be a non-empty string, or null before the runner image is built"
  )
| check($r.host_count; is_nonneg_int and . >= 1; "host_count must be an integer >= 1")
| check($r.repetitions; is_nonneg_int and . >= 1; "repetitions must be an integer >= 1")
| check($r.warmup_repetitions; is_nonneg_int; "warmup_repetitions must be a non-negative integer")
| check(
    $r.wall_time_ns;
    type == "array" and length == $r.repetitions and (all(.[]; is_nonneg_int));
    "wall_time_ns must have exactly `repetitions` non-negative nanosecond samples"
  )
| check($r.wall_time_ns; . == sort; "wall_time_ns must be sorted ascending")
| check($r.latency_ns; type == "object"; "latency_ns must be an object")
| check($r.latency_ns.p50; is_nonneg_int; "latency_ns.p50 must be a non-negative integer")
| check($r.latency_ns.p95; is_nonneg_int and . >= $r.latency_ns.p50; "latency_ns.p95 must be >= p50")
| check($r.latency_ns.p99; is_nonneg_int and . >= $r.latency_ns.p95; "latency_ns.p99 must be >= p95")
| check(
    $r.latency_ns.p50;
    . == nearest_rank($r.wall_time_ns; 50);
    "latency_ns.p50 is not the nearest-rank p50 of wall_time_ns"
  )
| check(
    $r.latency_ns.p95;
    . == nearest_rank($r.wall_time_ns; 95);
    "latency_ns.p95 is not the nearest-rank p95 of wall_time_ns"
  )
| check(
    $r.latency_ns.p99;
    . == nearest_rank($r.wall_time_ns; 99);
    "latency_ns.p99 is not the nearest-rank p99 of wall_time_ns"
  )
| check($r.throughput_ops_per_sec; is_nonneg_num; "throughput_ops_per_sec must be a non-negative number")
| check(
    $r.throughput_ops_per_sec;
    ($r.latency_ns.p50 == 0) or (
      (. - ($r.host_count / ($r.latency_ns.p50 / 1e9))) / ($r.host_count / ($r.latency_ns.p50 / 1e9))
      | fabs
      | . <= 1e-9
    );
    "throughput_ops_per_sec is inconsistent with host_count and latency_ns.p50"
  )
| check(
    $r.peak_rss_bytes;
    . == null or is_nonneg_int;
    "peak_rss_bytes must be a non-negative integer, or null before RSS collection runs"
  )

# A `ssh` report may leave these unknown: observing them means instrumenting
# the fixture, and the deterministic `mock` workloads already gate all three.
# A `mock` report has no such excuse.
| check(
    $r.command_count;
    is_nonneg_int or (. == null and $r.kind == "ssh");
    "command_count must be a non-negative integer, or null for a kind == \"ssh\" report"
  )
| check(
    $r.probe_count;
    is_nonneg_int or (. == null and $r.kind == "ssh");
    "probe_count must be a non-negative integer, or null for a kind == \"ssh\" report"
  )
| check(
    $r.peak_concurrency;
    (is_nonneg_int and . >= 1) or (. == null and $r.kind == "ssh");
    "peak_concurrency must be an integer >= 1, or null for a kind == \"ssh\" report"
  )
| check($r.exit_code; is_nonneg_int; "exit_code must be a non-negative integer")
| check($r.stdout_digest; is_nonempty_string; "stdout_digest must be a non-empty string")
| check(
    $r.stderr_digest;
    . == null or is_nonempty_string;
    "stderr_digest must be a non-empty string, or null for a workload whose stderr provably cannot reproduce (it must then declare `expect.stderr_digest: null` with a reason)"
  )
| check(
    $r.host_order;
    type == "array" and (all(.[]; type == "string"));
    "host_order must be an array of host-key strings"
  )
| .
