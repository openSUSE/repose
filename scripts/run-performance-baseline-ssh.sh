#!/usr/bin/env bash
#
# Runs every `ssh`-kind workload in $REPOSE_PERF_WORKLOADS against the live
# fixture started by tests/ssh/run.sh (which invokes this script with
# REPOSE_SSH_* set). Not meant to be run directly — see
# scripts/run-performance-baseline.sh.
set -euo pipefail

: "${REPOSE_SSH_TARGET:?tests/ssh/run.sh must set this}"
: "${REPOSE_SSH_IDENTITY:?tests/ssh/run.sh must set this}"
: "${REPOSE_SSH_KNOWN_HOSTS:?tests/ssh/run.sh must set this}"
: "${REPOSE_BIN:?run-performance-baseline.sh must set this}"
: "${REPOSE_PERF_WORKLOADS:?run-performance-baseline.sh must set this}"
: "${REPOSE_PERF_VALIDATOR:?run-performance-baseline.sh must set this}"
: "${REPOSE_PERF_OUT:?run-performance-baseline.sh must set this}"

REPS="${REPOSE_PERF_SSH_REPS:-5}"
WARMUP="${REPOSE_PERF_SSH_WARMUP:-1}"
RUNNER_CLASS="${REPOSE_PERF_RUNNER_CLASS:-local-dev}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
: >"$tmp/empty_known_hosts"

sha256_of() {
	if command -v sha256sum >/dev/null; then
		sha256sum | awk '{print "sha256:" $1}'
	else
		shasum -a 256 | awk '{print "sha256:" $1}'
	fi
}

# Build the repose argv for one ssh-kind workload (jq object on stdin as $w).
build_args() {
	local w="$1"
	local args=()
	local host_key_policy known_hosts output_format debug dry command

	host_key_policy="$(jq -r '.host_key_policy // "yes"' <<<"$w")"
	known_hosts_state="$(jq -r '.known_hosts_state // "prepopulated"' <<<"$w")"
	output_format="$(jq -r '.output_format // "text"' <<<"$w")"
	debug="$(jq -r '.debug // false' <<<"$w")"
	dry="$(jq -r '.dry // false' <<<"$w")"
	command="$(jq -r '.command' <<<"$w")"

	if [[ "$known_hosts_state" == "empty" ]]; then
		known_hosts="$tmp/empty_known_hosts"
	else
		known_hosts="$REPOSE_SSH_KNOWN_HOSTS"
	fi

	args+=(--strict-host-key-checking="$host_key_policy" --known-hosts "$known_hosts")
	[[ "$output_format" == "json" ]] && args+=(--format=json)
	[[ "$debug" == "true" ]] && args+=(--debug)
	[[ "$dry" == "true" ]] && args+=(--print)
	args+=("$command" -t "$REPOSE_SSH_TARGET")
	printf '%s\n' "${args[@]}"
}

percentile_ns() {
	# $1: nearest-rank percentile (e.g. 50, 95, 99); reads a jq array on stdin.
	jq -c "sort | .[(((${1} / 100) * length) | ceil) - 1]"
}

run_workload() {
	local id="$1" w
	w="$(jq -c --arg id "$id" '.workloads[] | select(.id == $id)' "$REPOSE_PERF_WORKLOADS")"
	mapfile -t args < <(build_args "$w")

	echo "-- $id --"
	local i exit_code=0 stdout="" stderr=""
	for ((i = 0; i < WARMUP; i++)); do
		stdout="$(env -u SSH_AUTH_SOCK -u COLOR NO_COLOR=1 "$REPOSE_BIN" -i "$REPOSE_SSH_IDENTITY" "${args[@]}" 2>"$tmp/stderr")" || exit_code=$?
	done

	local samples="[]"
	for ((i = 0; i < REPS; i++)); do
		local start end
		start="$(date +%s%N)"
		exit_code=0
		stdout="$(env -u SSH_AUTH_SOCK -u COLOR NO_COLOR=1 "$REPOSE_BIN" -i "$REPOSE_SSH_IDENTITY" "${args[@]}" 2>"$tmp/stderr")" || exit_code=$?
		end="$(date +%s%N)"
		stderr="$(cat "$tmp/stderr")"
		samples="$(jq -c --argjson ns "$((end - start))" '. + [$ns]' <<<"$samples")"

		local want_exit
		want_exit="$(jq -r '.expect.exit_code' <<<"$w")"
		if [[ "$exit_code" -ne "$want_exit" ]]; then
			echo "$id: exit code changed (want $want_exit, got $exit_code)" >&2
			echo "$stderr" >&2
			return 1
		fi
		while IFS= read -r needle; do
			[[ -z "$needle" ]] && continue
			if [[ "$stdout" != *"$needle"* ]]; then
				echo "$id: stdout no longer contains expected substring: $needle" >&2
				return 1
			fi
		done < <(jq -r '.expect.stdout_contains[]?' <<<"$w")
	done

	local p50 p95 p99 host_count
	p50="$(percentile_ns 50 <<<"$samples")"
	p95="$(percentile_ns 95 <<<"$samples")"
	p99="$(percentile_ns 99 <<<"$samples")"
	host_count="$(jq -r '.host_count' <<<"$w")"

	jq -n \
		--arg id "$id" \
		--argjson reps "$REPS" \
		--argjson warmup "$WARMUP" \
		--argjson samples "$samples" \
		--argjson p50 "$p50" \
		--argjson p95 "$p95" \
		--argjson p99 "$p99" \
		--argjson host_count "$host_count" \
		--argjson exit_code "$exit_code" \
		--arg stdout_digest "$(printf '%s' "$stdout" | sha256_of)" \
		--arg stderr_digest "$(printf '%s' "$stderr" | sha256_of)" \
		--arg toolchain "$(rustc --version)" \
		--arg runner_class "$RUNNER_CLASS" \
		--arg os "$(uname -s)" \
		--arg arch "$(uname -m)" \
		--arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
		'{
            contract_version: 1,
            workload_id: $id,
            kind: "ssh",
            runner: { os: $os, arch: $arch, toolchain: $toolchain, runner_class: $runner_class },
            generated_at: $generated_at,
            repetitions: $reps,
            warmup_repetitions: $warmup,
            wall_time_ns: ($samples | sort),
            latency_ns: { p50: $p50, p95: $p95, p99: $p99 },
            throughput_ops_per_sec: ($host_count / ($p50 / 1e9)),
            peak_rss_bytes: null,
            command_count: 1,
            probe_count: 0,
            peak_concurrency: $host_count,
            exit_code: $exit_code,
            stdout_digest: $stdout_digest,
            stderr_digest: $stderr_digest,
            host_order: [$id]
        }' >"$REPOSE_PERF_OUT/$id.json.tmp"

	jq -e -f "$REPOSE_PERF_VALIDATOR" "$REPOSE_PERF_OUT/$id.json.tmp" >/dev/null
	mv "$REPOSE_PERF_OUT/$id.json.tmp" "$REPOSE_PERF_OUT/$id.json"
	echo "  wrote $REPOSE_PERF_OUT/$id.json"
}

failed=0
mapfile -t ids < <(jq -r '.workloads[] | select(.kind == "ssh") | .id' "$REPOSE_PERF_WORKLOADS")
for id in "${ids[@]}"; do
	run_workload "$id" || failed=1
done
exit "$failed"
