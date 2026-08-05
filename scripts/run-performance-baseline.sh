#!/usr/bin/env bash
#
# P0.1 baseline orchestration: measures every `mock`-kind and `ssh`-kind
# workload in tests/performance/workloads.json inside a pinned Linux
# container pair — tests/performance/runner/Dockerfile (the build+measure
# environment) and the unmodified tests/ssh/Dockerfile (the OpenSSH
# fixture), on the same container network — so a run today and a run next
# month share an OS, libc, toolchain and set of system packages instead of
# whatever the contributor's laptop happens to have (see
# tests/performance/README.md). `--skip-ssh` is the one exception: it runs
# `mock`-kind workloads host-natively for fast local iteration, with
# `runner.fixture_runtime`/`runner.runner_image` left `null`.
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
#   REPOSE_PERF_CONTAINER_CLI  auto|docker|podman|container (default: auto).
#                              `auto` tries each in turn with a *liveness*
#                              call (not just `command -v`); an explicit
#                              value that isn't live fails loudly instead
#                              of falling back to another runtime — a
#                              baseline measured somewhere else is not the
#                              same baseline. Unused with --skip-ssh.
#   REPOSE_PERF_CACHE_DIR      persistent cargo/target cache outside the
#                              repo, bind-mounted into the runner container
#                              (default: $HOME/.cache/repose-perf).
#
# Run from the repository root; see tests/performance/README.md.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKLOADS="$ROOT/tests/performance/workloads.json"
VALIDATOR="$ROOT/scripts/validate-performance-report.jq"
SUMMARY_VALIDATOR="$ROOT/scripts/validate-performance-summary.jq"
RUNNER_DOCKERFILE="$ROOT/tests/performance/runner/Dockerfile"
FIXTURE_DOCKERFILE="$ROOT/tests/ssh/Dockerfile"
OUT="$ROOT/tests/performance/baselines/raw"
MOCK_REPS=20
MOCK_WARMUP=3
SSH_REPS=10
SSH_WARMUP=2
SKIP_SSH=0
RUNNER_CLASS="${REPOSE_PERF_RUNNER_CLASS:-local-dev}"
CONTAINER_CLI="${REPOSE_PERF_CONTAINER_CLI:-auto}"

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
		--arg profile "release" \
		--arg rustflags "${RUSTFLAGS:-}" \
		--arg fixture_runtime "${REPOSE_PERF_FIXTURE_RUNTIME:-}" \
		--arg runner_image "${REPOSE_PERF_RUNNER_IMAGE:-}" \
		--arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
		'.peak_rss_bytes = $rss
         | .runner.toolchain = $toolchain
         | .runner.runner_class = $runner_class
         | .runner.profile = $profile
         | .runner.rustflags = $rustflags
         | .runner.fixture_runtime = (if $fixture_runtime == "" then null else $fixture_runtime end)
         | .runner.runner_image = (if $runner_image == "" then null else $runner_image end)
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

run_mock_workloads() {
	echo "== building release artifacts =="
	cargo build --release --locked -p repose-core --example baseline_report
	cargo build --release --locked -p repose-cli

	echo "== mock-kind workloads =="
	local target_dir="${CARGO_TARGET_DIR:-$ROOT/target}"
	BASELINE_REPORT="$target_dir/release/examples/baseline_report"
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
}

# Drives every `ssh`-kind workload against the fixture whose address is
# already in REPOSE_SSH_HOST/REPOSE_SSH_PORT/REPOSE_SSH_TARGET/
# REPOSE_SSH_KNOWN_HOSTS — set by run_in_container_pair's `run` invocation
# before this script re-enters here with REPOSE_PERF_IN_CONTAINER=1.
run_ssh_workloads() {
	: "${REPOSE_SSH_HOST:?run_in_container_pair must set this}"
	: "${REPOSE_SSH_PORT:?run_in_container_pair must set this}"
	: "${REPOSE_SSH_TARGET:?run_in_container_pair must set this}"
	: "${REPOSE_SSH_KNOWN_HOSTS:?run_in_container_pair must set this}"
	REPOSE_BIN="${CARGO_TARGET_DIR:-$ROOT/target}/release/repose"
	export REPOSE_BIN REPOSE_PERF_OUT="$OUT" REPOSE_PERF_SSH_REPS="$SSH_REPS" \
		REPOSE_PERF_SSH_WARMUP="$SSH_WARMUP" REPOSE_PERF_WORKLOADS="$WORKLOADS" \
		REPOSE_PERF_VALIDATOR="$VALIDATOR" REPOSE_PERF_RUNNER_CLASS="$RUNNER_CLASS"
	if ! bash "$ROOT/scripts/run-performance-baseline-ssh.sh"; then
		FAILED=1
	fi
}

# --- container-pair orchestration (P0.1 step 7): resolves a live runtime,
# builds both images, starts the fixture on a private network, proves it
# reachable *from a container on that network* (this machine's host cannot
# reach a container's address at all — see
# plans/p0.1-repair-performance-measurement.md, assumption 6), then
# re-invokes this same script inside the runner with
# REPOSE_PERF_IN_CONTAINER=1 so run_mock_workloads/run_ssh_workloads above
# do the actual measuring, unchanged, on the other side of that boundary. ---

CLI=""
PLATFORM_ARCH=""  # "arm64" | "amd64" — the --platform suffix both CLIs share
EXPECT_UNAME_M="" # "aarch64" | "x86_64" — what the guest should report

resolve_arch() {
	case "$(uname -m)" in
	arm64 | aarch64)
		PLATFORM_ARCH="arm64"
		EXPECT_UNAME_M="aarch64"
		;;
	x86_64 | amd64)
		PLATFORM_ARCH="amd64"
		EXPECT_UNAME_M="x86_64"
		;;
	*)
		echo "unsupported host architecture: $(uname -m)" >&2
		exit 2
		;;
	esac
}

cli_alive() {
	# A present binary with a dead daemon must count as unavailable — hence
	# a liveness call, not `command -v`.
	case "$1" in
	docker) docker info >/dev/null 2>&1 ;;
	podman) podman info >/dev/null 2>&1 ;;
	container) container system status >/dev/null 2>&1 ;;
	*) return 1 ;;
	esac
}

resolve_cli() {
	if [[ "$CONTAINER_CLI" != "auto" ]]; then
		if ! command -v "$CONTAINER_CLI" >/dev/null 2>&1 || ! cli_alive "$CONTAINER_CLI"; then
			echo "REPOSE_PERF_CONTAINER_CLI=$CONTAINER_CLI requested but not live; no fallback — a baseline measured somewhere else is not the same baseline" >&2
			exit 2
		fi
		CLI="$CONTAINER_CLI"
		return
	fi
	local tried=()
	local candidate
	for candidate in docker podman container; do
		tried+=("$candidate")
		if command -v "$candidate" >/dev/null 2>&1 && cli_alive "$candidate"; then
			CLI="$candidate"
			return
		fi
	done
	echo "no live container runtime found (tried: ${tried[*]}); install/start one, or pass --skip-ssh for a host-native mock-only run" >&2
	exit 2
}

# `build` and `run --rm ... uname -m` are identical across docker/podman/
# container; only Apple `container` additionally needs the local-image
# variant list checked, since it will pull and run an amd64 image under
# silent emulation.
assert_image_arch() {
	local image="$1" got
	got="$("$CLI" run --rm --platform "linux/$PLATFORM_ARCH" "$image" uname -m)"
	[[ "$got" == "$EXPECT_UNAME_M" ]] || {
		echo "image $image reports arch $got under --platform linux/$PLATFORM_ARCH, expected $EXPECT_UNAME_M" >&2
		exit 2
	}
	if [[ "$CLI" == "container" ]]; then
		local archs
		archs="$(container image inspect "$image" | jq -r '.[0].variants[].config.architecture' | sort -u)"
		[[ "$archs" == "$PLATFORM_ARCH" ]] || {
			echo "image $image has unexpected local variant(s): $archs (want exactly $PLATFORM_ARCH)" >&2
			exit 2
		}
	fi
}

# The image digest is what makes "these two runs were built the same way"
# checkable rather than asserted (scripts/compare-performance.sh rejects a
# mismatch).
image_digest() {
	local image="$1"
	if [[ "$CLI" == "container" ]]; then
		echo "sha256:$(container image inspect "$image" | jq -r '.[0].id')"
	else
		"$CLI" image inspect --format '{{.Id}}' "$image"
	fi
}

ensure_network() {
	"$CLI" network inspect "$1" >/dev/null 2>&1 || "$CLI" network create "$1" >/dev/null
}

fixture_ipv4() {
	local name="$1"
	if [[ "$CLI" == "container" ]]; then
		container inspect "$name" 2>/dev/null | jq -r '.[0].status.networks[0].ipv4Address // empty' | cut -d/ -f1
	else
		"$CLI" inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$name" 2>/dev/null
	fi
}

run_in_container_pair() {
	resolve_arch
	resolve_cli
	echo "== container runtime: $CLI (platform linux/$PLATFORM_ARCH) =="

	command -v ssh-keygen >/dev/null || {
		echo "ssh-keygen is required" >&2
		exit 2
	}

	local net="repose-perf-net"
	local runner_image="repose-perf-runner:local"
	local fixture_image="repose-perf-fixture:local"
	local fixture_name="repose-perf-fixture-$$"
	local secrets
	secrets="$(mktemp -d)"
	local cache_dir="${REPOSE_PERF_CACHE_DIR:-$HOME/.cache/repose-perf}"
	mkdir -p "$cache_dir/cargo-registry" "$cache_dir/cargo-git" "$cache_dir/target"

	cleanup() {
		"$CLI" stop "$fixture_name" >/dev/null 2>&1 || true
		rm -rf "$secrets"
	}
	trap cleanup EXIT INT TERM

	echo "== building runner image (tests/performance/runner/Dockerfile) =="
	"$CLI" build --platform "linux/$PLATFORM_ARCH" -f "$RUNNER_DOCKERFILE" -t "$runner_image" "$ROOT"
	assert_image_arch "$runner_image"

	echo "== building fixture image (tests/ssh/Dockerfile, unmodified) =="
	"$CLI" build --platform "linux/$PLATFORM_ARCH" -f "$FIXTURE_DOCKERFILE" -t "$fixture_image" "$ROOT"
	assert_image_arch "$fixture_image"

	ensure_network "$net"

	ssh-keygen -q -t ed25519 -N '' -C repose-perf -f "$secrets/client_key"
	ssh-keygen -q -t ed25519 -N '' -C repose-perf-host -f "$secrets/host_key"
	cp "$secrets/client_key.pub" "$secrets/authorized_keys"
	chmod 0600 "$secrets/authorized_keys" "$secrets/client_key" "$secrets/host_key"

	echo "== starting fixture =="
	"$CLI" run -d --rm --name "$fixture_name" --platform "linux/$PLATFORM_ARCH" --network "$net" \
		--mount "type=bind,source=$secrets,target=/fixture,readonly" "$fixture_image" >/dev/null

	local ip=""
	local _try
	for _try in $(seq 1 40); do
		ip="$(fixture_ipv4 "$fixture_name")"
		[[ -n "$ip" ]] && break
		sleep 0.25
	done
	[[ -n "$ip" ]] || {
		echo "could not resolve the fixture container's network address" >&2
		exit 1
	}

	# sshd's readiness is proven from *inside* the runner network — the
	# host cannot reach a container's address at all on this machine (see
	# plans/p0.1-repair-performance-measurement.md, assumption 6).
	echo "== waiting for the fixture to answer on $ip:22 =="
	if ! "$CLI" run --rm --platform "linux/$PLATFORM_ARCH" --network "$net" "$runner_image" \
		sh -c "for i in \$(seq 1 60); do ssh-keyscan -T 2 -p 22 '$ip' 2>/dev/null && exit 0; sleep 0.5; done; exit 1" >/dev/null; then
		echo "fixture did not become reachable from the runner network" >&2
		exit 1
	fi

	# Port 22 is the fixture's real listening port now that there is no
	# published host port to discover: container-to-container needs no
	# publishing at all. OpenSSH's own known_hosts convention leaves a
	# default-port host unbracketed (repose-ssh's hostkey.rs), unlike the
	# ephemeral, always-bracketed port the old published-port setup used.
	local host_public
	host_public="$(cut -d' ' -f1,2 "$secrets/host_key.pub")"
	printf '%s %s\n' "$ip" "$host_public" >"$secrets/known_hosts"

	mkdir -p "$secrets/home/.ssh"
	cat >"$secrets/home/.ssh/config" <<-EOF
		Host $ip
		  IdentityFile $secrets/client_key
		  IdentitiesOnly yes
	EOF
	chmod 0700 "$secrets/home" "$secrets/home/.ssh"
	chmod 0600 "$secrets/home/.ssh/config"

	local digest
	digest="$(image_digest "$runner_image")"

	echo "== running the measured build inside the runner =="
	local rc=0
	"$CLI" run --rm --platform "linux/$PLATFORM_ARCH" --network "$net" \
		--mount "type=bind,source=$ROOT,target=$ROOT" \
		--mount "type=bind,source=$OUT,target=$OUT" \
		--mount "type=bind,source=$secrets,target=$secrets,readonly" \
		--mount "type=bind,source=$cache_dir/cargo-registry,target=/usr/local/cargo/registry" \
		--mount "type=bind,source=$cache_dir/cargo-git,target=/usr/local/cargo/git" \
		--mount "type=bind,source=$cache_dir/target,target=/cache/target" \
		-w "$ROOT" \
		-e HOME="$secrets/home" \
		-e CARGO_TARGET_DIR=/cache/target \
		-e RUSTFLAGS="${RUSTFLAGS:-}" \
		-e REPOSE_PERF_IN_CONTAINER=1 \
		-e REPOSE_PERF_RUNNER_CLASS="$RUNNER_CLASS" \
		-e REPOSE_PERF_FIXTURE_RUNTIME="$CLI" \
		-e REPOSE_PERF_RUNNER_IMAGE="$digest" \
		-e REPOSE_SSH_HOST="$ip" \
		-e REPOSE_SSH_PORT=22 \
		-e REPOSE_SSH_TARGET="repose@$ip:22" \
		-e REPOSE_SSH_KNOWN_HOSTS="$secrets/known_hosts" \
		"$runner_image" \
		bash scripts/run-performance-baseline.sh --out "$OUT" \
		--mock-reps "$MOCK_REPS" --mock-warmup "$MOCK_WARMUP" \
		--ssh-reps "$SSH_REPS" --ssh-warmup "$SSH_WARMUP" || rc=$?

	# Run cleanup here, before `$secrets`/`$fixture_name` (both `local` to
	# this function) go out of scope — a trap firing at the whole script's
	# EXIT would reference them after that scope is gone.
	trap - EXIT INT TERM
	cleanup

	[[ "$rc" -eq 0 ]] || FAILED=1
}

if [[ "$SKIP_SSH" -eq 1 ]]; then
	run_mock_workloads
	echo "== ssh-kind workloads: skipped (--skip-ssh) =="
elif [[ "${REPOSE_PERF_IN_CONTAINER:-0}" -eq 1 ]]; then
	run_mock_workloads
	echo "== ssh-kind workloads =="
	run_ssh_workloads
else
	run_in_container_pair
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
	dest="$SUMMARY_DIR/$runner_class.json"
	jq -s '{
        contract_version: 2,
        runner: (.[0].runner),
        generated_at: (map(.generated_at) | max),
        workloads: (
            map({
                workload_id, kind, repetitions, warmup_repetitions, host_count,
                latency_ns, throughput_ops_per_sec, peak_rss_bytes,
                command_count, probe_count, peak_concurrency, exit_code,
                stdout_digest, stderr_digest
            }) | sort_by(.workload_id)
        )
    }' "${ALL_REPORTS[@]}" >"$dest.tmp"
	jq -L "$ROOT/scripts" -e -f "$SUMMARY_VALIDATOR" "$dest.tmp" >/dev/null
	mv "$dest.tmp" "$dest"
	echo "wrote summary $dest"
fi
