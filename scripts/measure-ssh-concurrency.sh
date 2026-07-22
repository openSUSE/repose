#!/usr/bin/env bash
#
# P1 decision-gate evidence (step 2): open several *concurrent* SSH sessions
# against the tests/ssh/ Docker OpenSSH fixture and record per-session
# connect+auth+product-discovery latency percentiles under load.
#
# The one-container fixture cannot model true 100-host network fan-out (see
# tests/performance/README.md's documented P0 limitation) — this measures
# real (not fabricated) transport/auth/SFTP timing under concurrent load
# against one sshd, used only to calibrate proposed P1 caps/deadlines with
# real headroom, not to claim fleet-scale throughput.
#
# Invoked by tests/ssh/run.sh, which sets REPOSE_SSH_*:
#   tests/ssh/run.sh bash scripts/measure-ssh-concurrency.sh OUT_DIR
set -euo pipefail

: "${REPOSE_SSH_TARGET:?tests/ssh/run.sh must set this}"
: "${REPOSE_SSH_KNOWN_HOSTS:?tests/ssh/run.sh must set this}"
: "${REPOSE_BIN:?caller must set this}"

OUT="${1:?usage: measure-ssh-concurrency.sh OUT_DIR}"
LEVELS="${REPOSE_PERF_SSH_CONCURRENCY_LEVELS:-1 5 10 20}"
mkdir -p "$OUT"

percentile() {
	# $1: nearest-rank percentile; stdin: one integer (ns) per line.
	local pct="$1" n rank
	mapfile -t vals < <(sort -n)
	n="${#vals[@]}"
	rank=$((((pct * n) + 99) / 100))
	((rank < 1)) && rank=1
	((rank > n)) && rank=n
	echo "${vals[$((rank - 1))]}"
}

report="$OUT/ssh-concurrency.json"
echo '{"contract_version":1,"kind":"ssh-concurrency","levels":[' >"$report"
first_level=1
for k in $LEVELS; do
	echo "-- concurrency=$k --" >&2
	tmp="$(mktemp -d)"
	pids=()
	for ((i = 0; i < k; i++)); do
		(
			start="$(date +%s%N)"
			set +e
			# Identity resolution goes through ~/.ssh/config's IdentityFile
			# (set up by the caller), matching how RusshSession resolves
			# candidate keys — there is no `-i`/identity CLI flag.
			env -u SSH_AUTH_SOCK NO_COLOR=1 "$REPOSE_BIN" \
				--strict-host-key-checking=yes --known-hosts "$REPOSE_SSH_KNOWN_HOSTS" \
				list-products -t "$REPOSE_SSH_TARGET" >"$tmp/$i.out" 2>"$tmp/$i.err"
			code=$?
			set -e
			end="$(date +%s%N)"
			printf '%s %s\n' "$((end - start))" "$code" >"$tmp/$i.timing"
		) &
		pids+=("$!")
	done
	fail=0
	for pid in "${pids[@]}"; do
		wait "$pid" || fail=1
	done

	: >"$tmp/samples_ns"
	for ((i = 0; i < k; i++)); do
		read -r ns code <"$tmp/$i.timing"
		echo "$ns" >>"$tmp/samples_ns"
		if [[ "$code" -ne 0 ]]; then
			echo "concurrency=$k worker $i failed (exit $code):" >&2
			cat "$tmp/$i.err" >&2
			fail=1
		fi
	done

	p50="$(percentile 50 <"$tmp/samples_ns")"
	p95="$(percentile 95 <"$tmp/samples_ns")"
	p99="$(percentile 99 <"$tmp/samples_ns")"
	max="$(sort -n "$tmp/samples_ns" | tail -1)"

	[[ "$first_level" -eq 1 ]] || echo ',' >>"$report"
	first_level=0
	jq -n --argjson k "$k" --argjson p50 "$p50" --argjson p95 "$p95" \
		--argjson p99 "$p99" --argjson max "$max" --argjson failures "$fail" \
		'{concurrency: $k, latency_ns: {p50: $p50, p95: $p95, p99: $p99, max: $max}, any_failure: ($failures != 0)}' \
		>>"$report"
	rm -rf "$tmp"

	if [[ "$fail" -ne 0 ]]; then
		echo "concurrency=$k had at least one failed session" >&2
	fi
done
echo ']}' >>"$report"
jq -e . "$report" >/dev/null
echo "wrote $report"
