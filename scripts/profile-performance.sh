#!/usr/bin/env bash
#
# P0.4 release CPU/allocation/I/O profiling: builds once in release mode,
# runs one P0.1 mock workload under the platform's profiling tools, and
# writes a normalized manifest (scripts/validate-performance-profile.jq)
# plus raw tool output. Only `mock`-kind workloads are supported today
# (see tests/performance/README.md for the ssh-kind gap).
#
# Platform adapters:
#   darwin: `sample` (CPU), `heap` (live allocation footprint — a
#           point-in-time snapshot, not a full allocation-count profile),
#           `fs_usage` (I/O; requires root).
#   linux:  `perf record`/`perf report` (CPU), `heaptrack` (allocation,
#           wraps the launched process rather than attaching), `strace -c`
#           (I/O syscall counts).
#
# A requested class that cannot run (missing tool, insufficient privilege,
# unsupported OS) is recorded with `"status": "skipped"` and a reason, and
# the script exits nonzero — never a silent partial success.
#
# Usage:
#   scripts/profile-performance.sh <workload-id> [--classes cpu,alloc,io]
#       [--reps N] [--duration SECS] [--out DIR] [--raw-dir DIR]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKLOADS="$ROOT/tests/performance/workloads.json"
VALIDATOR="$ROOT/scripts/validate-performance-profile.jq"
OUT="$ROOT/tests/performance/profiles"
RAW_DIR="$ROOT/tests/performance/profiles/raw"
CLASSES="cpu,alloc"
REPS=200000
DURATION=5

[[ $# -ge 1 ]] || {
	echo "usage: $0 <workload-id> [--classes cpu,alloc,io] [--reps N] [--duration SECS] [--out DIR] [--raw-dir DIR]" >&2
	exit 2
}
ID="$1"
shift
while [[ $# -gt 0 ]]; do
	case "$1" in
	--classes)
		CLASSES="$2"
		shift 2
		;;
	--reps)
		REPS="$2"
		shift 2
		;;
	--duration)
		DURATION="$2"
		shift 2
		;;
	--out)
		OUT="$2"
		shift 2
		;;
	--raw-dir)
		RAW_DIR="$2"
		shift 2
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
jq -e --arg id "$ID" '.workloads[] | select(.id == $id and .kind == "mock")' "$WORKLOADS" >/dev/null ||
	{
		echo "$ID: not a mock-kind workload in $WORKLOADS" >&2
		exit 2
	}
mkdir -p "$OUT" "$RAW_DIR"

echo "== building release artifacts =="
cargo build --release --locked -p repose-core --example baseline_report
BASELINE_REPORT="$ROOT/target/release/examples/baseline_report"

# Long-running proxy: enough repetitions to stay alive through the whole
# profiling window (this measures steady-state per-command cost, not
# process startup — the profiler attaches after the process is already
# running the measured loop).
MEASURED_CMD="$BASELINE_REPORT $ID $REPS 1"
echo "measured command: $MEASURED_CMD"
$MEASURED_CMD >/tmp/repose-profile-stdout.$$ &
PID=$!
sleep 0.5
if ! kill -0 "$PID" 2>/dev/null; then
	echo "measured process exited before profiling could start" >&2
	exit 1
fi
cleanup() { kill "$PID" 2>/dev/null || true; }
trap cleanup EXIT

FAILED=0
CLASSES_JSON="{}"
IFS=',' read -ra CLASS_LIST <<<"$CLASSES"

add_class() {
	# add_class NAME JSON_FRAGMENT
	CLASSES_JSON="$(jq --arg name "$1" --argjson v "$2" '. + {($name): $v}' <<<"$CLASSES_JSON")"
}

profile_darwin_cpu() {
	local raw="$RAW_DIR/${ID}.cpu.sample.txt"
	sample "$PID" "$DURATION" -f "$raw" >/dev/null 2>&1 || {
		add_class cpu "$(jq -n --arg reason "sample failed (see $raw if present)" '{status:"skipped",reason:$reason}')"
		FAILED=1
		return
	}
	local top_json total
	top_json="$(awk '/^Sort by top of stack/{flag=1; next} /^$/{if (flag) exit} flag' "$raw" |
		awk '{n=$NF; $NF=""; sym=$0; gsub(/^[ \t]+|[ \t]+$/, "", sym); print n"\t"sym}' |
		head -10 |
		jq -R -s '
            split("\n") | map(select(length > 0)) |
            map(split("\t")) | map({symbol: .[1], samples: (.[0] | tonumber)})
        ')"
	total="$(awk '/^Sort by top of stack/{flag=1; next} /^$/{if (flag) exit} flag {sum+=$NF} END{print sum+0}' "$raw")"
	add_class cpu "$(jq -n --arg tool sample --arg artifact "$raw" --argjson top "$top_json" --argjson total "${total:-0}" \
		'{tool: $tool, raw_artifact: $artifact, total_top_of_stack_samples: $total, top_symbols: $top}')"
}

profile_darwin_alloc() {
	local raw="$RAW_DIR/${ID}.alloc.heap.txt"
	heap "$PID" >"$raw" 2>&1 || {
		add_class alloc "$(jq -n --arg reason "heap failed (see $raw if present)" '{status:"skipped",reason:$reason}')"
		FAILED=1
		return
	}
	local footprint nodes bytes
	footprint="$(awk -F': *' '/^Physical footprint:/{print $2}' "$raw" | tr -d '[:space:]')"
	nodes="$(grep -oE '[0-9]+ nodes malloced' "$raw" | awk '{print $1}')"
	bytes="$(grep -oE '\([0-9]+ bytes\)' "$raw" | tail -1 | tr -dc '0-9')"
	add_class alloc "$(jq -n --arg tool heap --arg artifact "$raw" \
		--arg footprint "${footprint:-null}" --argjson nodes "${nodes:-0}" --argjson bytes "${bytes:-0}" \
		'{tool: $tool, raw_artifact: $artifact, physical_footprint: $footprint, node_count: $nodes, total_bytes: $bytes}')"
}

profile_darwin_io() {
	local raw="$RAW_DIR/${ID}.io.fs_usage.txt"
	fs_usage -w -f filesys "$PID" >"$raw" 2>&1 &
	local fs_pid=$!
	sleep 1
	kill "$fs_pid" 2>/dev/null || true
	wait "$fs_pid" 2>/dev/null || true
	if grep -q "must be run as root" "$raw" 2>/dev/null; then
		add_class io "$(jq -n --arg reason "fs_usage requires root (rerun this script with sudo, or via sudo -v beforehand)" \
			'{status:"skipped",reason:$reason}')"
		FAILED=1
		return
	fi
	local lines
	lines="$(wc -l <"$raw" | tr -d '[:space:]')"
	add_class io "$(jq -n --arg tool fs_usage --arg artifact "$raw" --argjson events "${lines:-0}" \
		'{tool: $tool, raw_artifact: $artifact, filesystem_events: $events}')"
}

profile_linux_cpu() {
	local raw="$RAW_DIR/${ID}.cpu.perf.data" report="$RAW_DIR/${ID}.cpu.perf.txt"
	if ! command -v perf >/dev/null; then
		add_class cpu "$(jq -n '{status:"skipped",reason:"perf not installed"}')"
		FAILED=1
		return
	fi
	if ! perf record -o "$raw" -p "$PID" -- sleep "$DURATION" 2>"$RAW_DIR/${ID}.cpu.perf.stderr"; then
		add_class cpu "$(jq -n --arg reason "perf record failed (likely needs CAP_PERFMON/perf_event_paranoid; see $RAW_DIR/${ID}.cpu.perf.stderr)" \
			'{status:"skipped",reason:$reason}')"
		FAILED=1
		return
	fi
	perf report -i "$raw" --stdio -n >"$report" 2>/dev/null || true
	local top_json
	top_json="$(awk '/^#/{next} NF>0{print}' "$report" | head -10 |
		jq -R -s 'split("\n") | map(select(length > 0)) | map({line: .})')"
	add_class cpu "$(jq -n --arg tool perf --arg artifact "$report" --argjson top "$top_json" \
		'{tool: $tool, raw_artifact: $artifact, top_symbols: $top}')"
}

profile_linux_alloc() {
	# heaptrack wraps the launch rather than attaching; the already-running
	# $PID from this script cannot be retrofitted, so this class documents
	# the correct invocation for a follow-up run instead of faking data.
	if ! command -v heaptrack >/dev/null; then
		add_class alloc "$(jq -n '{status:"skipped",reason:"heaptrack not installed"}')"
		FAILED=1
		return
	fi
	add_class alloc "$(jq -n --arg reason "heaptrack profiles a fresh launch, not an attached PID; rerun as: heaptrack $MEASURED_CMD" \
		'{status:"skipped",reason:$reason}')"
	FAILED=1
}

profile_linux_io() {
	if ! command -v strace >/dev/null; then
		add_class io "$(jq -n '{status:"skipped",reason:"strace not installed"}')"
		FAILED=1
		return
	fi
	local raw="$RAW_DIR/${ID}.io.strace.txt"
	strace -c -p "$PID" -o "$raw" 2>"$RAW_DIR/${ID}.io.strace.stderr" &
	local strace_pid=$!
	sleep "$DURATION"
	kill -INT "$strace_pid" 2>/dev/null || true
	wait "$strace_pid" 2>/dev/null
	local strace_status=$?
	if [[ "$strace_status" -ne 0 && "$strace_status" -ne 130 ]]; then
		add_class io "$(jq -n --arg reason "strace could not attach (may need CAP_SYS_PTRACE / ptrace_scope=0); see $RAW_DIR/${ID}.io.strace.stderr" \
			'{status:"skipped",reason:$reason}')"
		FAILED=1
		return
	fi
	add_class io "$(jq -n --arg tool strace --arg artifact "$raw" '{tool: $tool, raw_artifact: $artifact}')"
}

OS="$(uname -s)"
for class in "${CLASS_LIST[@]}"; do
	case "$OS-$class" in
	Darwin-cpu) profile_darwin_cpu ;;
	Darwin-alloc) profile_darwin_alloc ;;
	Darwin-io) profile_darwin_io ;;
	Linux-cpu) profile_linux_cpu ;;
	Linux-alloc) profile_linux_alloc ;;
	Linux-io) profile_linux_io ;;
	*)
		add_class "$class" "$(jq -n --arg reason "unsupported OS $OS for class $class" '{status:"skipped",reason:$reason}')"
		FAILED=1
		;;
	esac
done

kill "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true
trap - EXIT

MANIFEST="$OUT/${ID}.$(echo "$OS" | tr '[:upper:]' '[:lower:]').json"
jq -n \
	--arg id "$ID" \
	--arg os "$(uname -s | tr '[:upper:]' '[:lower:]')" \
	--arg arch "$(uname -m)" \
	--arg cmd "$MEASURED_CMD" \
	--argjson pid "$PID" \
	--arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
	--argjson classes "$CLASSES_JSON" \
	'{
        contract_version: 1,
        workload_id: $id,
        platform: {os: $os, arch: $arch},
        measured_command: $cmd,
        measured_pid: $pid,
        setup_excluded: true,
        generated_at: $generated_at,
        classes: $classes
    }' >"$MANIFEST"

jq -e -f "$VALIDATOR" "$MANIFEST" >/dev/null
echo "wrote $MANIFEST"

if [[ "$FAILED" -ne 0 ]]; then
	echo "one or more requested profile classes were skipped (see manifest reasons above) — not a silent partial success" >&2
	exit 1
fi
