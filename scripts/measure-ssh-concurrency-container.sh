#!/usr/bin/env bash
#
# LOCAL-DEV-ONLY P1 decision-gate evidence gatherer. Docker is unavailable
# on this machine; this drives the *same* tests/ssh/Dockerfile fixture
# through Apple's `container` CLI (macOS-native container runtime) instead,
# then reuses scripts/measure-ssh-concurrency.sh for the actual
# measurement. Not part of the committed CI/contributor path — that
# remains tests/ssh/run.sh + Docker (see tests/performance/p1-limit-decision.md
# for why this script exists and its scope).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="repose-ssh-test-container-local"
tmp="$(mktemp -d)"
container_name="repose-ssh-perf-$$"

cleanup() {
	container rm -f "$container_name" >/dev/null 2>&1 || true
	rm -rf "$tmp"
}
trap cleanup EXIT INT TERM

command -v container >/dev/null || {
	echo "Apple container CLI is required" >&2
	exit 2
}
command -v ssh-keygen >/dev/null || {
	echo "ssh-keygen is required" >&2
	exit 2
}

ssh-keygen -q -t ed25519 -N '' -C repose-integration -f "$tmp/client_key"
ssh-keygen -q -t ed25519 -N '' -C repose-fixture-host -f "$tmp/host_key"
cp "$tmp/client_key.pub" "$tmp/authorized_keys"
chmod 0600 "$tmp/authorized_keys" "$tmp/client_key" "$tmp/host_key"

echo "== building fixture image via Apple container runtime ==" >&2
container build --tag "$IMAGE" --file "$ROOT/tests/ssh/Dockerfile" "$ROOT" >&2

port="$(python3 -c "import socket; s=socket.socket(); s.bind(('127.0.0.1',0)); print(s.getsockname()[1]); s.close()")"

container run --detach --name "$container_name" \
	--publish "127.0.0.1:${port}:22" \
	--mount "type=bind,source=$tmp,target=/fixture,readonly" \
	"$IMAGE" >&2

echo "== waiting for sshd on 127.0.0.1:$port ==" >&2
ready=0
for _ in $(seq 1 60); do
	if (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then
		exec 3<&- 3>&-
		ready=1
		break
	fi
	sleep 1
done
[[ "$ready" -eq 1 ]] || {
	echo "sshd did not become reachable" >&2
	container logs "$container_name" >&2 || true
	exit 1
}
# TCP accept can race sshd's own listen(); give the handshake a moment.
sleep 1

mkdir -p "$tmp/home/.ssh"
cat >"$tmp/home/.ssh/config" <<EOF
Host 127.0.0.1
  IdentityFile $tmp/client_key
  IdentitiesOnly yes
EOF
chmod 0700 "$tmp/home/.ssh"
chmod 0600 "$tmp/home/.ssh/config"

host_public="$(cut -d' ' -f1,2 "$tmp/host_key.pub")"
printf '[127.0.0.1]:%s %s\n' "$port" "$host_public" >"$tmp/known_hosts"

export HOME="$tmp/home"
export SSH_AUTH_SOCK="$tmp/no-agent"
export REPOSE_SSH_TARGET="repose@127.0.0.1:$port"
export REPOSE_SSH_IDENTITY="$tmp/client_key"
export REPOSE_SSH_KNOWN_HOSTS="$tmp/known_hosts"

"$@"
