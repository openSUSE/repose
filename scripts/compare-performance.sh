#!/usr/bin/env bash
#
# P0.5 regression comparator: compares two contract-valid performance
# reports (scripts/validate-performance-report.jq) for the *same* workload
# and runner class, using the rules in tests/performance/thresholds.json.
#
# Exact semantic metrics (exit code, command/probe counts, output digests)
# tolerate no unexplained change. Peak concurrency may not exceed the
# baseline's observed value. Latency/RSS/throughput use the documented,
# variance-derived tolerances — see tests/performance/thresholds.json.
#
# Usage:
#   scripts/compare-performance.sh <baseline.json> <candidate.json> [--allow-cross-runner]
#
# Exit codes (distinguished so callers/tests can tell failure kinds apart):
#   0  pass (including a real improvement)
#   1  regression (an exact metric changed, or a threshold was crossed)
#   2  contract failure (either input is not a valid performance report)
#   3  incomparable metadata (different workload_id/runner_class, or below
#      the minimum repetition count)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VALIDATOR="$ROOT/scripts/validate-performance-report.jq"
THRESHOLDS="$ROOT/tests/performance/thresholds.json"

[[ $# -ge 2 ]] || {
	echo "usage: $0 <baseline.json> <candidate.json> [--allow-cross-runner]" >&2
	exit 3
}
BASELINE="$1"
CANDIDATE="$2"
shift 2
ALLOW_CROSS_RUNNER=0
while [[ $# -gt 0 ]]; do
	case "$1" in
	--allow-cross-runner)
		ALLOW_CROSS_RUNNER=1
		shift
		;;
	*)
		echo "unknown argument: $1" >&2
		exit 3
		;;
	esac
done

command -v jq >/dev/null || {
	echo "jq is required" >&2
	exit 3
}
[[ -f "$BASELINE" ]] || {
	echo "baseline report not found: $BASELINE" >&2
	exit 3
}
[[ -f "$CANDIDATE" ]] || {
	echo "candidate report not found: $CANDIDATE" >&2
	exit 3
}

for f in "$BASELINE" "$CANDIDATE"; do
	if ! jq -e -f "$VALIDATOR" "$f" >/dev/null 2>"$ROOT/.compare-performance-contract-error.$$"; then
		echo "CONTRACT FAILURE: $f does not satisfy the report contract:" >&2
		cat "$ROOT/.compare-performance-contract-error.$$" >&2
		rm -f "$ROOT/.compare-performance-contract-error.$$"
		exit 2
	fi
	rm -f "$ROOT/.compare-performance-contract-error.$$"
done

base_id="$(jq -r '.workload_id' "$BASELINE")"
cand_id="$(jq -r '.workload_id' "$CANDIDATE")"
if [[ "$base_id" != "$cand_id" ]]; then
	echo "INCOMPARABLE: workload_id differs (baseline=$base_id, candidate=$cand_id)" >&2
	exit 3
fi

base_runner="$(jq -r '.runner.runner_class // "unknown"' "$BASELINE")"
cand_runner="$(jq -r '.runner.runner_class // "unknown"' "$CANDIDATE")"
if [[ "$base_runner" != "$cand_runner" && "$ALLOW_CROSS_RUNNER" -ne 1 ]]; then
	echo "INCOMPARABLE: runner_class differs (baseline=$base_runner, candidate=$cand_runner); pass --allow-cross-runner to override" >&2
	exit 3
fi

min_reps="$(jq -r '.minimum_repetitions' "$THRESHOLDS")"
base_reps="$(jq -r '.repetitions' "$BASELINE")"
cand_reps="$(jq -r '.repetitions' "$CANDIDATE")"
if [[ "$base_reps" -lt "$min_reps" || "$cand_reps" -lt "$min_reps" ]]; then
	echo "INCOMPARABLE: repetitions below minimum $min_reps (baseline=$base_reps, candidate=$cand_reps)" >&2
	exit 3
fi

FAILED=0
report() {
	# report STATUS MESSAGE
	echo "$1: $2"
	if [[ "$1" == "REGRESSION" ]]; then
		FAILED=1
	fi
}

check_exact() {
	local metric="$1" b c
	b="$(jq -c ".$metric" "$BASELINE")"
	c="$(jq -c ".$metric" "$CANDIDATE")"
	if [[ "$b" == "$c" ]]; then
		report OK "$metric unchanged ($b)"
	else
		report REGRESSION "$metric changed: baseline=$b candidate=$c"
	fi
}

check_ceiling() {
	local metric="$1" b c
	b="$(jq -r ".$metric" "$BASELINE")"
	c="$(jq -r ".$metric" "$CANDIDATE")"
	if [[ "$c" -le "$b" ]]; then
		report OK "$metric within ceiling (baseline=$b, candidate=$c)"
	else
		report REGRESSION "$metric exceeded baseline ceiling: baseline=$b candidate=$c"
	fi
}

check_max_increase() {
	local metric="$1" ratio="$2" b c
	b="$(jq -r ".$metric" "$BASELINE")"
	c="$(jq -r ".$metric" "$CANDIDATE")"
	if [[ "$b" == "null" || "$c" == "null" ]]; then
		report SKIP "$metric: null in baseline or candidate (not yet collected on this platform)"
		return
	fi
	local limit within
	limit="$(jq -n --argjson b "$b" --argjson r "$ratio" '$b * (1 + $r)')"
	within="$(jq -n --argjson c "$c" --argjson limit "$limit" '$c <= $limit')"
	if [[ "$within" == "true" ]]; then
		report OK "$metric within +$(jq -n --argjson r "$ratio" '$r*100')% (baseline=$b, candidate=$c, limit=$limit)"
	else
		report REGRESSION "$metric regressed beyond +$(jq -n --argjson r "$ratio" '$r*100')%: baseline=$b candidate=$c limit=$limit"
	fi
}

check_max_decrease() {
	local metric="$1" ratio="$2" b c
	b="$(jq -r ".$metric" "$BASELINE")"
	c="$(jq -r ".$metric" "$CANDIDATE")"
	local floor within
	floor="$(jq -n --argjson b "$b" --argjson r "$ratio" '$b * (1 - $r)')"
	within="$(jq -n --argjson c "$c" --argjson floor "$floor" '$c >= $floor')"
	if [[ "$within" == "true" ]]; then
		report OK "$metric within -$(jq -n --argjson r "$ratio" '$r*100')% (baseline=$b, candidate=$c, floor=$floor)"
	else
		report REGRESSION "$metric regressed beyond -$(jq -n --argjson r "$ratio" '$r*100')%: baseline=$b candidate=$c floor=$floor"
	fi
}

echo "comparing $base_id (baseline=$BASELINE, candidate=$CANDIDATE)"
check_exact "exit_code"
check_exact "command_count"
check_exact "probe_count"
check_exact "stdout_digest"
check_exact "stderr_digest"
check_ceiling "peak_concurrency"
check_max_increase "latency_ns.p50" "$(jq -r '.metrics."latency_ns.p50".value' "$THRESHOLDS")"
check_max_increase "latency_ns.p95" "$(jq -r '.metrics."latency_ns.p95".value' "$THRESHOLDS")"
check_max_increase "latency_ns.p99" "$(jq -r '.metrics."latency_ns.p99".value' "$THRESHOLDS")"
check_max_decrease "throughput_ops_per_sec" "$(jq -r '.metrics.throughput_ops_per_sec.value' "$THRESHOLDS")"
check_max_increase "peak_rss_bytes" "$(jq -r '.metrics.peak_rss_bytes.value' "$THRESHOLDS")"

if [[ "$FAILED" -ne 0 ]]; then
	echo "RESULT: REGRESSION" >&2
	exit 1
fi
echo "RESULT: PASS"
