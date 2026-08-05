#!/usr/bin/env bash
#
# Runs every `ssh`-kind workload in $REPOSE_PERF_WORKLOADS against the live
# fixture started by tests/ssh/run.sh (which invokes this script with
# REPOSE_SSH_* set). Not meant to be run directly — see
# scripts/run-performance-baseline.sh.
set -euo pipefail

# repose has no identity flag: the client key reaches it through the
# `IdentityFile` line the fixture writes into $HOME/.ssh/config.
: "${REPOSE_SSH_TARGET:?tests/ssh/run.sh must set this}"
: "${REPOSE_SSH_HOST:?tests/ssh/run.sh must set this}"
: "${REPOSE_SSH_PORT:?tests/ssh/run.sh must set this}"
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

# Every repetition's exact stdout/stderr bytes, kept for debugging a digest
# mismatch: the digests below are of the *normalized* streams, so the raw
# ones are the only way to see what actually differed.
RAW_IO="$REPOSE_PERF_OUT/raw-io"

sha256_of() {
	if command -v sha256sum >/dev/null; then
		sha256sum | awk '{print "sha256:" $1}'
	else
		shasum -a 256 | awk '{print "sha256:" $1}'
	fi
}

# The fixture's port is ephemeral, so the raw streams differ between runs in
# a way that says nothing about performance. Fold both spellings the output
# carries — `[host]:port` (known_hosts entries, host-key messages) and
# `host:port` (the target echo, the `Host:` header) — to fixed placeholders
# before hashing.
#
# `jq -Rsj` round-trips the stream verbatim, so a stream that does not end in
# a newline still does not after normalizing; `sed` would append one on BSD
# but not on GNU and make the digest depend on the runner's userland.
# `split`/`join` on a plain string is a literal replacement, so a host that
# looks like a regex cannot misfire.
normalized_digest() {
	jq -Rsj \
		--arg bracket "[$REPOSE_SSH_HOST]:$REPOSE_SSH_PORT" \
		--arg plain "$REPOSE_SSH_HOST:$REPOSE_SSH_PORT" \
		'split($bracket) | join("[FIXTURE]:PORT")
		 | split($plain) | join("FIXTURE:PORT")' "$1" | sha256_of
}

# Build the repose argv for one ssh-kind workload ($1: workload JSON,
# $2: the known_hosts path this repetition must use).
build_args() {
	local w="$1" known_hosts="$2"
	local args=()
	local host_key_policy output_format debug dry command

	host_key_policy="$(jq -r '.host_key_policy // "yes"' <<<"$w")"
	output_format="$(jq -r '.output_format // "text"' <<<"$w")"
	debug="$(jq -r '.debug // false' <<<"$w")"
	dry="$(jq -r '.dry // false' <<<"$w")"
	command="$(jq -r '.command' <<<"$w")"

	args+=(--strict-host-key-checking="$host_key_policy" --known-hosts "$known_hosts")
	if [[ "$output_format" == "json" ]]; then
		args+=(--format=json)
	fi
	if [[ "$debug" == "true" ]]; then
		args+=(--debug)
	fi
	if [[ "$dry" == "true" ]]; then
		args+=(--print)
	fi
	args+=("$command" -t "$REPOSE_SSH_TARGET")
	printf '%s\n' "${args[@]}"
}

# One repetition of one workload ($1: workload JSON, $2: id, $3: label).
# Sets RUN_EXIT, RUN_OUT, RUN_ERR, RUN_STDOUT_DIGEST, RUN_STDERR_DIGEST and
# leaves the raw bytes on disk. Wall time is measured by the caller so that
# none of this bookkeeping lands inside the timed window.
run_once() {
	local w="$1" id="$2" label="$3"
	local state known_hosts
	local -a args

	state="$(jq -r '.known_hosts_state // "prepopulated"' <<<"$w")"
	if [[ "$state" == "empty" ]]; then
		# A *fresh* file per repetition, warmups included. Sharing one file
		# would make every repetition after the first a known host, and the
		# workload would silently stop measuring first contact.
		known_hosts="$tmp/known_hosts-$id-$label"
		: >"$known_hosts"
		if [[ -s "$known_hosts" ]]; then
			echo "$id/$label: known_hosts was not empty before the run" >&2
			return 1
		fi
	else
		known_hosts="$REPOSE_SSH_KNOWN_HOSTS"
	fi

	mapfile -t args < <(build_args "$w" "$known_hosts")

	RUN_OUT="$RAW_IO/$id/$label.out"
	RUN_ERR="$RAW_IO/$id/$label.err"
	RUN_EXIT=0
	RUN_START="$(date +%s%N)"
	env -u SSH_AUTH_SOCK -u COLOR NO_COLOR=1 "$REPOSE_BIN" "${args[@]}" \
		>"$RUN_OUT" 2>"$RUN_ERR" || RUN_EXIT=$?
	RUN_END="$(date +%s%N)"

	# That the file gained exactly one entry *is* the proof this repetition
	# accepted the host key on first contact; without it the run could have
	# been trusting a key it already had.
	if [[ "$state" == "empty" ]]; then
		local entries
		entries="$(grep -c . "$known_hosts" || true)"
		if [[ "$entries" -ne 1 ]]; then
			echo "$id/$label: known_hosts has $entries entries after the run, want exactly 1" >&2
			return 1
		fi
	fi

	RUN_STDOUT_DIGEST="$(normalized_digest "$RUN_OUT")"
	RUN_STDERR_DIGEST="$(normalized_digest "$RUN_ERR")"
}

# Enforce the workload's reviewed `expect` block against one repetition.
check_expect() {
	local w="$1" id="$2" label="$3"
	local want_exit needle

	want_exit="$(jq -r '.expect.exit_code' <<<"$w")"
	if [[ "$RUN_EXIT" -ne "$want_exit" ]]; then
		echo "$id/$label: exit code changed (want $want_exit, got $RUN_EXIT)" >&2
		cat "$RUN_ERR" >&2
		return 1
	fi
	while IFS= read -r needle; do
		if [[ -z "$needle" ]]; then
			continue
		fi
		if ! grep -Fq -- "$needle" "$RUN_OUT"; then
			echo "$id/$label: stdout no longer contains expected substring: $needle" >&2
			return 1
		fi
	done < <(jq -r '.expect.stdout_contains[]?' <<<"$w")
}

# The first repetition fixes the digests; every later one must reproduce
# them. Reporting only the last run's digest would let genuinely unstable
# output through as a stable-looking number.
#
# A workload may declare `expect.stderr_digest: null` to opt its stderr out,
# for a stream that provably cannot reproduce (see the reason recorded
# alongside it in workloads.json). stdout is never exempt.
assert_stable() {
	local id="$1" label="$2"
	if [[ -z "$FIRST_STDOUT_DIGEST" ]]; then
		FIRST_STDOUT_DIGEST="$RUN_STDOUT_DIGEST"
		FIRST_STDERR_DIGEST="$RUN_STDERR_DIGEST"
		return 0
	fi
	if [[ "$RUN_STDOUT_DIGEST" != "$FIRST_STDOUT_DIGEST" ]] ||
		{ [[ "$SKIP_STDERR_DIGEST" -eq 0 ]] && [[ "$RUN_STDERR_DIGEST" != "$FIRST_STDERR_DIGEST" ]]; }; then
		echo "$id/$label: normalized output is not stable across repetitions" >&2
		echo "  first:  out=$FIRST_STDOUT_DIGEST err=$FIRST_STDERR_DIGEST" >&2
		echo "  $label: out=$RUN_STDOUT_DIGEST err=$RUN_STDERR_DIGEST" >&2
		echo "  raw streams kept under $RAW_IO/$id/" >&2
		return 1
	fi
}

percentile_ns() {
	# $1: nearest-rank percentile (e.g. 50, 95, 99); reads a jq array on stdin.
	jq -c "sort | .[(((${1} / 100) * length) | ceil) - 1]"
}

run_workload() {
	local id="$1" w
	w="$(jq -c --arg id "$id" '.workloads[] | select(.id == $id)' "$REPOSE_PERF_WORKLOADS")"

	echo "-- $id --"
	rm -rf "${RAW_IO:?}/$id"
	mkdir -p "$RAW_IO/$id"

	local i label
	FIRST_STDOUT_DIGEST=""
	FIRST_STDERR_DIGEST=""
	SKIP_STDERR_DIGEST=0
	if [[ "$(jq -r '.expect | has("stderr_digest") and .stderr_digest == null' <<<"$w")" == "true" ]]; then
		SKIP_STDERR_DIGEST=1
	fi

	# Warmups are checked exactly as measured repetitions are; only their
	# timings are discarded.
	# `run_workload` is called as `run_workload ... || failed=1`, which
	# disables errexit for everything it calls: every failure has to be
	# propagated by hand.
	for ((i = 0; i < WARMUP; i++)); do
		label="warmup-$i"
		run_once "$w" "$id" "$label" || return 1
		check_expect "$w" "$id" "$label" || return 1
		assert_stable "$id" "$label" || return 1
	done

	local samples="[]"
	for ((i = 0; i < REPS; i++)); do
		label="rep-$i"
		run_once "$w" "$id" "$label" || return 1
		check_expect "$w" "$id" "$label" || return 1
		assert_stable "$id" "$label" || return 1
		samples="$(jq -c --argjson ns "$((RUN_END - RUN_START))" '. + [$ns]' <<<"$samples")"
	done

	local p50 p95 p99 host_count stderr_digest
	p50="$(percentile_ns 50 <<<"$samples")"
	p95="$(percentile_ns 95 <<<"$samples")"
	p99="$(percentile_ns 99 <<<"$samples")"
	host_count="$(jq -r '.host_count' <<<"$w")"
	if [[ "$SKIP_STDERR_DIGEST" -eq 1 ]]; then
		stderr_digest=null
	else
		stderr_digest="$(jq -n --arg d "$FIRST_STDERR_DIGEST" '$d')"
	fi

	jq -n \
		--arg id "$id" \
		--argjson reps "$REPS" \
		--argjson warmup "$WARMUP" \
		--argjson samples "$samples" \
		--argjson p50 "$p50" \
		--argjson p95 "$p95" \
		--argjson p99 "$p99" \
		--argjson host_count "$host_count" \
		--argjson exit_code "$RUN_EXIT" \
		--arg stdout_digest "$FIRST_STDOUT_DIGEST" \
		--argjson stderr_digest "$stderr_digest" \
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

	jq -e -f "$REPOSE_PERF_VALIDATOR" "$REPOSE_PERF_OUT/$id.json.tmp" >/dev/null || return 1
	mv "$REPOSE_PERF_OUT/$id.json.tmp" "$REPOSE_PERF_OUT/$id.json"
	echo "  wrote $REPOSE_PERF_OUT/$id.json"
}

failed=0
mapfile -t ids < <(jq -r '.workloads[] | select(.kind == "ssh") | .id' "$REPOSE_PERF_WORKLOADS")
for id in "${ids[@]}"; do
	run_workload "$id" || failed=1
done
exit "$failed"
