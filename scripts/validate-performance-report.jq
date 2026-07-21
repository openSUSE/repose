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

. as $r
| check($r.contract_version; . == 1; "contract_version must be 1")
| check($r.workload_id; is_nonempty_string; "workload_id must be a non-empty string")
| check($r.kind; . == "mock" or . == "ssh"; "kind must be \"mock\" or \"ssh\"")
| check($r.runner; type == "object"; "runner must be an object")
| check($r.runner.os; is_nonempty_string; "runner.os must be a non-empty string")
| check($r.runner.arch; is_nonempty_string; "runner.arch must be a non-empty string")
| check($r.repetitions; is_nonneg_int and . >= 1; "repetitions must be an integer >= 1")
| check($r.warmup_repetitions; is_nonneg_int; "warmup_repetitions must be a non-negative integer")
| check(
    $r.wall_time_ns;
    type == "array" and length == $r.repetitions and (all(.[]; is_nonneg_int));
    "wall_time_ns must have exactly `repetitions` non-negative nanosecond samples"
  )
| check($r.latency_ns; type == "object"; "latency_ns must be an object")
| check($r.latency_ns.p50; is_nonneg_int; "latency_ns.p50 must be a non-negative integer")
| check($r.latency_ns.p95; is_nonneg_int and . >= $r.latency_ns.p50; "latency_ns.p95 must be >= p50")
| check($r.latency_ns.p99; is_nonneg_int and . >= $r.latency_ns.p95; "latency_ns.p99 must be >= p95")
| check($r.throughput_ops_per_sec; is_nonneg_num; "throughput_ops_per_sec must be a non-negative number")
| check(
    $r.peak_rss_bytes;
    . == null or is_nonneg_int;
    "peak_rss_bytes must be a non-negative integer, or null before RSS collection runs"
  )
| check($r.command_count; is_nonneg_int; "command_count must be a non-negative integer")
| check($r.probe_count; is_nonneg_int; "probe_count must be a non-negative integer")
| check($r.peak_concurrency; is_nonneg_int and . >= 1; "peak_concurrency must be an integer >= 1")
| check($r.exit_code; is_nonneg_int; "exit_code must be a non-negative integer")
| check($r.stdout_digest; is_nonempty_string; "stdout_digest must be a non-empty string")
| check($r.stderr_digest; is_nonempty_string; "stderr_digest must be a non-empty string")
| check(
    $r.host_order;
    type == "array" and (all(.[]; type == "string"));
    "host_order must be an array of host-key strings"
  )
| .
