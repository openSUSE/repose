# Shared "comparable subset" checks for a performance report reduced to
# what a compact baseline summary retains (tests/performance/README.md):
# no `wall_time_ns` samples, so no percentile/throughput recomputation
# against raw data — see scripts/validate-performance-report.jq for the
# full standalone-report contract, which this deliberately does not
# duplicate wholesale.
#
# A *library* only (jq requires an `include`d file to contain nothing but
# definitions): callers run
#   jq -L scripts -e 'include "validate-performance-comparable"; check_comparable'
# against a single object shaped like one summary `workloads[]` entry
# merged with that summary's `runner` and `contract_version`. Used by
# scripts/validate-performance-summary.jq (once per entry) and
# scripts/compare-performance.sh's `--workload` mode (the same merge, for
# one extracted entry).

def check(val; pred; msg):
  if (val | pred) then . else error("invalid report: " + msg) end;

def is_nonneg_int: type == "number" and (. == (. | floor)) and . >= 0;
def is_nonneg_num: type == "number" and . >= 0;
def is_nonempty_string: type == "string" and length > 0;

def check_comparable:
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
      "runner.fixture_runtime must be a non-empty string, or null"
    )
  | check(
      $r.runner.runner_image;
      . == null or is_nonempty_string;
      "runner.runner_image must be a non-empty string, or null"
    )
  | check($r.host_count; is_nonneg_int and . >= 1; "host_count must be an integer >= 1")
  | check($r.repetitions; is_nonneg_int and . >= 1; "repetitions must be an integer >= 1")
  | check($r.warmup_repetitions; is_nonneg_int; "warmup_repetitions must be a non-negative integer")
  | check($r.latency_ns; type == "object"; "latency_ns must be an object")
  | check($r.latency_ns.p50; is_nonneg_int; "latency_ns.p50 must be a non-negative integer")
  | check($r.latency_ns.p95; is_nonneg_int and . >= $r.latency_ns.p50; "latency_ns.p95 must be >= p50")
  | check($r.latency_ns.p99; is_nonneg_int and . >= $r.latency_ns.p95; "latency_ns.p99 must be >= p95")
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
      "peak_rss_bytes must be a non-negative integer, or null"
    )
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
      "stderr_digest must be a non-empty string, or null"
    )
  ;
