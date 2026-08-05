#!/usr/bin/env bash
#
# P0.5 regression comparator: compares two contract-valid performance
# reports (scripts/validate-performance-report.jq) for the *same* workload
# and runner, using the rules in tests/performance/thresholds.json.
#
# Exact semantic metrics (exit code, command/probe counts, output digests)
# tolerate no unexplained change. Peak concurrency may not exceed the
# baseline's observed value. Latency/RSS/throughput use the documented,
# variance-derived tolerances — see tests/performance/thresholds.json.
# Any metric that is `null` on either side is SKIPped, not compared.
#
# Usage:
#   scripts/compare-performance.sh <baseline.json> <candidate.json>
#       [--allow-cross-runner] [--semantic-only]
#   scripts/compare-performance.sh --workload <id> <baseline> <candidate> [...]
#
# `--workload <id>` extracts one workload's data from either input: a
# standalone report is used as-is (workload_id must match `<id>`); a
# compact baseline summary (tests/performance/baselines/<runner-class>.json)
# has its `<id>` entry merged with the summary's `runner`/`contract_version`
# and checked against the comparable subset of the contract (see
# scripts/validate-performance-comparable.jq — no `wall_time_ns` there, so
# no percentile recomputation for that side).
#
# `--semantic-only` runs the exact + ceiling checks only (deterministic,
# runner-independent), implying `--allow-cross-runner` and skipping the
# repetition-count gate — see tests/performance/README.md for why latency/
# RSS gating stays opt-in until a controlled/scheduled runner has its own
# variance study.
#
# Exit codes (distinguished so callers/tests can tell failure kinds apart):
#   0  pass (including a real improvement)
#   1  regression (an exact metric changed, or a threshold was crossed)
#   2  contract failure (either input, or an extracted --workload entry,
#      is not a valid performance report)
#   3  incomparable metadata (different workload_id, a runner/environment
#      field mismatch, a --workload id absent from a summary, or below
#      the minimum repetition count)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPTS="$ROOT/scripts"
VALIDATOR="$SCRIPTS/validate-performance-report.jq"
THRESHOLDS="$ROOT/tests/performance/thresholds.json"

WORKLOAD_ID=""
ALLOW_CROSS_RUNNER=0
SEMANTIC_ONLY=0
POSITIONAL=()
while [[ $# -gt 0 ]]; do
	case "$1" in
	--workload)
		WORKLOAD_ID="$2"
		shift 2
		;;
	--allow-cross-runner)
		ALLOW_CROSS_RUNNER=1
		shift
		;;
	--semantic-only)
		SEMANTIC_ONLY=1
		ALLOW_CROSS_RUNNER=1
		shift
		;;
	*)
		POSITIONAL+=("$1")
		shift
		;;
	esac
done
set -- "${POSITIONAL[@]}"

[[ $# -ge 2 ]] || {
	echo "usage: $0 [--workload <id>] <baseline.json> <candidate.json> [--allow-cross-runner] [--semantic-only]" >&2
	exit 3
}
BASELINE="$1"
CANDIDATE="$2"

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

TMP_FILES=()
cleanup() { rm -f "${TMP_FILES[@]}"; }
trap cleanup EXIT

contract_fail() {
	# contract_fail <what> <error-file>
	echo "CONTRACT FAILURE: $1 does not satisfy the report contract:" >&2
	cat "$2" >&2
	exit 2
}

# Extract workload $WORKLOAD_ID's data from $1 into $2: a standalone report
# is validated as-is (after confirming its workload_id matches); a summary
# has the matching `workloads[]` entry merged with the summary's
# `runner`/`contract_version` and checked against the comparable subset of
# the contract.
extract_workload() {
	local file="$1" dest="$2" errfile
	errfile="$(mktemp)"
	TMP_FILES+=("$errfile")
	if jq -e 'has("workloads")' "$file" >/dev/null 2>&1; then
		local entry
		entry="$(jq -c --arg id "$WORKLOAD_ID" '.workloads[] | select(.workload_id == $id)' "$file")"
		[[ -n "$entry" ]] || {
			echo "INCOMPARABLE: workload $WORKLOAD_ID not found in summary $file" >&2
			exit 3
		}
		jq -n --argjson entry "$entry" --slurpfile meta <(jq '{runner, contract_version}' "$file") \
			'$entry + $meta[0]' >"$dest"
		if ! jq -L "$SCRIPTS" -e 'include "validate-performance-comparable"; check_comparable' "$dest" \
			>/dev/null 2>"$errfile"; then
			contract_fail "$file's $WORKLOAD_ID entry" "$errfile"
		fi
	else
		local got_id
		got_id="$(jq -r '.workload_id // empty' "$file")"
		[[ "$got_id" == "$WORKLOAD_ID" ]] || {
			echo "INCOMPARABLE: $file is workload_id=${got_id:-<none>}, want $WORKLOAD_ID" >&2
			exit 3
		}
		if ! jq -e -f "$VALIDATOR" "$file" >"$dest" 2>"$errfile"; then
			contract_fail "$file" "$errfile"
		fi
	fi
}

if [[ -n "$WORKLOAD_ID" ]]; then
	tmp_base="$(mktemp)"
	tmp_cand="$(mktemp)"
	TMP_FILES+=("$tmp_base" "$tmp_cand")
	extract_workload "$BASELINE" "$tmp_base"
	extract_workload "$CANDIDATE" "$tmp_cand"
	BASELINE="$tmp_base"
	CANDIDATE="$tmp_cand"
else
	for f in "$BASELINE" "$CANDIDATE"; do
		errfile="$(mktemp)"
		TMP_FILES+=("$errfile")
		if ! jq -e -f "$VALIDATOR" "$f" >/dev/null 2>"$errfile"; then
			contract_fail "$f" "$errfile"
		fi
	done
fi

base_id="$(jq -r '.workload_id' "$BASELINE")"
cand_id="$(jq -r '.workload_id' "$CANDIDATE")"
if [[ "$base_id" != "$cand_id" ]]; then
	echo "INCOMPARABLE: workload_id differs (baseline=$base_id, candidate=$cand_id)" >&2
	exit 3
fi

# Everything an identically-built runner should agree on. With the pinned
# container image (P0.1 step 7/8) all of these but `.arch`/`.runner_class`/
# `.fixture_runtime` should now match across contributors; `.runner_image`
# is the cheapest single check for "these two runs were not built the same
# way".
check_metadata() {
	local field="$1" b c
	b="$(jq -r "$field // \"null\"" "$BASELINE")"
	c="$(jq -r "$field // \"null\"" "$CANDIDATE")"
	if [[ "$b" != "$c" && "$ALLOW_CROSS_RUNNER" -ne 1 ]]; then
		echo "INCOMPARABLE: $field differs (baseline=$b, candidate=$c); pass --allow-cross-runner to override" >&2
		exit 3
	fi
}
check_metadata ".contract_version"
for field in os arch toolchain profile rustflags fixture_runtime runner_image runner_class; do
	check_metadata ".runner.$field"
done

base_reps="$(jq -r '.repetitions' "$BASELINE")"
cand_reps="$(jq -r '.repetitions' "$CANDIDATE")"
if [[ "$SEMANTIC_ONLY" -ne 1 ]]; then
	min_reps="$(jq -r '.minimum_repetitions' "$THRESHOLDS")"
	if [[ "$base_reps" -lt "$min_reps" || "$cand_reps" -lt "$min_reps" ]]; then
		echo "INCOMPARABLE: repetitions below minimum $min_reps (baseline=$base_reps, candidate=$cand_reps)" >&2
		exit 3
	fi
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
	if [[ "$b" == "null" || "$c" == "null" ]]; then
		report SKIP "$metric: null in baseline or candidate (not observed for this workload's kind)"
		return
	fi
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
	if [[ "$b" == "null" || "$c" == "null" ]]; then
		report SKIP "$metric: null in baseline or candidate (not observed for this workload's kind)"
		return
	fi
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
	if [[ "$b" == "null" || "$c" == "null" ]]; then
		report SKIP "$metric: null in baseline or candidate (not yet collected on this platform)"
		return
	fi
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

if [[ "$SEMANTIC_ONLY" -ne 1 ]]; then
	check_max_increase "latency_ns.p50" "$(jq -r '.metrics."latency_ns.p50".value' "$THRESHOLDS")"
	check_max_increase "latency_ns.p95" "$(jq -r '.metrics."latency_ns.p95".value' "$THRESHOLDS")"
	tail_min_reps="$(jq -r '.tail_minimum_repetitions' "$THRESHOLDS")"
	if [[ "$base_reps" -ge "$tail_min_reps" && "$cand_reps" -ge "$tail_min_reps" ]]; then
		check_max_increase "latency_ns.p99" "$(jq -r '.metrics."latency_ns.p99".value' "$THRESHOLDS")"
	else
		report SKIP "latency_ns.p99: repetitions below tail_minimum_repetitions=$tail_min_reps (baseline=$base_reps, candidate=$cand_reps)"
	fi
	check_max_decrease "throughput_ops_per_sec" "$(jq -r '.metrics.throughput_ops_per_sec.value' "$THRESHOLDS")"
	check_max_increase "peak_rss_bytes" "$(jq -r '.metrics.peak_rss_bytes.value' "$THRESHOLDS")"
fi

if [[ "$FAILED" -ne 0 ]]; then
	echo "RESULT: REGRESSION" >&2
	exit 1
fi
echo "RESULT: PASS"
