#!/usr/bin/env bash
#
# P0.1 baseline orchestration: build once in release mode, run every
# `mock`-kind workload in tests/performance/workloads.json through the
# `baseline_report` harness (crates/repose-core/examples/baseline_report.rs),
# and every `ssh`-kind workload against the Docker OpenSSH fixture
# (tests/ssh/run.sh) — for real transport, product discovery, and a
# first-contact `accept-new` known_hosts scenario.
#
# Every report is checked against the workload's reviewed `expect` block
# (see tests/performance/workloads.json) before it is trusted, then
# validated against the report contract (scripts/validate-performance-report.jq)
# before being written out. A failure at either stage is a nonzero exit;
# no partial/misleading report is left behind.
#
# Usage:
#   scripts/run-performance-baseline.sh [--out DIR] [--mock-reps N]
#       [--mock-warmup N] [--ssh-reps N] [--ssh-warmup N] [--skip-ssh]
#
# Env:
#   REPOSE_PERF_RUNNER_CLASS   identity of this runner (default: local-dev).
#
# Run from the repository root; see tests/performance/README.md.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKLOADS="$ROOT/tests/performance/workloads.json"
VALIDATOR="$ROOT/scripts/validate-performance-report.jq"
OUT="$ROOT/tests/performance/baselines/raw"
MOCK_REPS=20
MOCK_WARMUP=3
SSH_REPS=5
SSH_WARMUP=1
SKIP_SSH=0
RUNNER_CLASS="${REPOSE_PERF_RUNNER_CLASS:-local-dev}"

while [[ $# -gt 0 ]]; do
	case "$1" in
	--out)
		OUT="$2"
		shift 2
		;;
	--mock-reps)
		MOCK_REPS="$2"
		shift 2
		;;
	--mock-warmup)
		MOCK_WARMUP="$2"
		shift 2
		;;
	--ssh-reps)
		SSH_REPS="$2"
		shift 2
		;;
	--ssh-warmup)
		SSH_WARMUP="$2"
		shift 2
		;;
	--skip-ssh)
		SKIP_SSH=1
		shift
		;;
	*)
		echo "unknown argument: $1" >&2
		exit 2
		;;
	esac
done

command -v jq >/dev/null || {
	echo "jq is required" >&2
	exit 2
}
mkdir -p "$OUT"

FAILED=0

# --- shared report finishing: inject toolchain/runner_class/generated_at,
# measure this-process peak RSS via the platform's `time` wrapper, validate
# against the report contract, write the file. ---
finish_report() {
	local id="$1" tmp_stdout="$2" time_file="$3" dest="$4"
	local rss_bytes="null"
	case "$(uname -s)" in
	Darwin)
		if [[ -s "$time_file" ]]; then
			rss_bytes="$(awk '/maximum resident set size/ { print $1 }' "$time_file")"
			[[ -n "$rss_bytes" ]] || rss_bytes="null"
		fi
		;;
	Linux)
		if [[ -s "$time_file" ]]; then
			local kb
			kb="$(awk -F': ' '/Maximum resident set size/ { print $2 }' "$time_file" | tr -d '[:space:]')"
			[[ -n "$kb" ]] && rss_bytes=$((kb * 1024))
		fi
		;;
	esac

	jq -e \
		--argjson rss "$rss_bytes" \
		--arg toolchain "$(rustc --version)" \
		--arg runner_class "$RUNNER_CLASS" \
		--arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
		'.peak_rss_bytes = $rss
         | .runner.toolchain = $toolchain
         | .runner.runner_class = $runner_class
         | .generated_at = $generated_at' \
		"$tmp_stdout" >"$dest.tmp"
	jq -e -f "$VALIDATOR" "$dest.tmp" >/dev/null
	mv "$dest.tmp" "$dest"
	echo "  wrote $dest"
}

# Wrap `cmd...` with the platform's peak-RSS-reporting `time`, capturing
# stdout separately from the timing report.
run_with_rss() {
	local stdout_file="$1" time_file="$2"
	shift 2
	case "$(uname -s)" in
	Darwin)
		/usr/bin/time -l "$@" >"$stdout_file" 2>"$time_file"
		;;
	Linux)
		if /usr/bin/time -v true >/dev/null 2>&1; then
			/usr/bin/time -v "$@" >"$stdout_file" 2>"$time_file"
		else
			echo "note: GNU time -v unavailable; peak_rss_bytes will be null" >&2
			: >"$time_file"
			"$@" >"$stdout_file"
		fi
		;;
	*)
		echo "note: unsupported OS for RSS collection; peak_rss_bytes will be null" >&2
		: >"$time_file"
		"$@" >"$stdout_file"
		;;
	esac
}

echo "== building release artifacts =="
cargo build --release --locked -p repose-core --example baseline_report
cargo build --release --locked -p repose-cli

echo "== mock-kind workloads =="
BASELINE_REPORT="$ROOT/target/release/examples/baseline_report"
mapfile -t MOCK_IDS < <(jq -r '.workloads[] | select(.kind == "mock") | .id' "$WORKLOADS")
for id in "${MOCK_IDS[@]}"; do
	echo "-- $id --"
	tmp_stdout="$(mktemp)"
	time_file="$(mktemp)"
	if run_with_rss "$tmp_stdout" "$time_file" "$BASELINE_REPORT" "$id" "$MOCK_REPS" "$MOCK_WARMUP"; then
		finish_report "$id" "$tmp_stdout" "$time_file" "$OUT/$id.json"
	else
		echo "FAILED: $id (equivalence check or crash — see above)" >&2
		FAILED=1
	fi
	rm -f "$tmp_stdout" "$time_file"
done

if [[ "$SKIP_SSH" -eq 1 ]]; then
	echo "== ssh-kind workloads: skipped (--skip-ssh) =="
elif ! command -v docker >/dev/null || ! command -v ssh-keygen >/dev/null; then
	echo "== ssh-kind workloads: skipped (docker/ssh-keygen not available) =="
else
	echo "== ssh-kind workloads =="
	REPOSE_BIN="$ROOT/target/release/repose"
	export REPOSE_BIN REPOSE_PERF_OUT="$OUT" REPOSE_PERF_SSH_REPS="$SSH_REPS" \
		REPOSE_PERF_SSH_WARMUP="$SSH_WARMUP" REPOSE_PERF_WORKLOADS="$WORKLOADS" \
		REPOSE_PERF_VALIDATOR="$VALIDATOR" REPOSE_PERF_RUNNER_CLASS="$RUNNER_CLASS"
	if ! "$ROOT/tests/ssh/run.sh" bash "$ROOT/scripts/run-performance-baseline-ssh.sh"; then
		FAILED=1
	fi
fi

if [[ "$FAILED" -ne 0 ]]; then
	echo "one or more workloads failed; see above" >&2
	exit 1
fi
echo "all workloads produced contract-valid, equivalence-checked reports in $OUT"

# Compact, committable summary keyed by runner identity (raw per-workload
# reports, including full sample arrays and host_order, stay uncommitted
# artifacts under $OUT — see tests/performance/README.md).
SUMMARY_DIR="$ROOT/tests/performance/baselines"
mkdir -p "$SUMMARY_DIR"
mapfile -t ALL_REPORTS < <(find "$OUT" -maxdepth 1 -name '*.json' | sort)
if [[ "${#ALL_REPORTS[@]}" -gt 0 ]]; then
	runner_class="$(jq -r '.runner.runner_class' "${ALL_REPORTS[0]}")"
	jq -s '{
        contract_version: 1,
        runner: (.[0].runner),
        generated_at: (map(.generated_at) | max),
        workloads: (
            map({
                workload_id, kind, repetitions, warmup_repetitions,
                latency_ns, throughput_ops_per_sec, peak_rss_bytes,
                command_count, probe_count, peak_concurrency, exit_code,
                stdout_digest, stderr_digest
            }) | sort_by(.workload_id)
        )
    }' "${ALL_REPORTS[@]}" >"$SUMMARY_DIR/$runner_class.json"
	echo "wrote summary $SUMMARY_DIR/$runner_class.json"
fi
