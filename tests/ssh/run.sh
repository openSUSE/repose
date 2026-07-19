#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE="repose-ssh-test:${REPOSE_SSH_IMAGE_TAG:-local}"
tmp="$(mktemp -d)"
container=""

cleanup() {
	if [[ -n "$container" ]]; then
		docker rm -f "$container" >/dev/null 2>&1 || true
	fi
	rm -rf "$tmp"
}
trap cleanup EXIT INT TERM

if [[ $# -eq 0 ]]; then
	echo "usage: tests/ssh/run.sh COMMAND [ARG ...]" >&2
	exit 2
fi
command -v docker >/dev/null || {
	echo "docker is required" >&2
	exit 2
}
command -v ssh-keygen >/dev/null || {
	echo "ssh-keygen is required" >&2
	exit 2
}

ssh-keygen -q -t ed25519 -N '' -C repose-integration -f "$tmp/client_key"
ssh-keygen -q -t ed25519 -N '' -C repose-fixture-host -f "$tmp/host_key"
ssh-keygen -q -t ed25519 -N '' -C repose-wrong-host -f "$tmp/wrong_host_key"
cp "$tmp/client_key.pub" "$tmp/authorized_keys"
chmod 0600 "$tmp/authorized_keys" "$tmp/client_key" "$tmp/host_key"

docker build --tag "$IMAGE" --file "$ROOT/tests/ssh/Dockerfile" "$ROOT"
container="$(docker run --detach --rm \
	--publish 127.0.0.1::22 \
	--mount "type=bind,src=$tmp,dst=/fixture,readonly" \
	"$IMAGE")"

for _ in $(seq 1 40); do
	health="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$container")"
	[[ "$health" == healthy ]] && break
	[[ "$health" == unhealthy ]] && {
		docker logs "$container" >&2
		exit 1
	}
	sleep 0.25
done
[[ "${health:-}" == healthy ]] || {
	docker logs "$container" >&2
	echo "sshd did not become healthy" >&2
	exit 1
}

port="$(docker port "$container" 22/tcp | awk -F: 'NR == 1 { print $NF }')"
[[ "$port" =~ ^[0-9]+$ ]] || {
	echo "could not determine fixture port" >&2
	exit 1
}

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
export REPOSE_SSH_HOST="127.0.0.1"
export REPOSE_SSH_PORT="$port"
export REPOSE_SSH_USER="repose"
export REPOSE_SSH_TARGET="repose@127.0.0.1:$port"
export REPOSE_SSH_IDENTITY="$tmp/client_key"
export REPOSE_SSH_WRONG_IDENTITY="$tmp/wrong_host_key"
export REPOSE_SSH_KNOWN_HOSTS="$tmp/known_hosts"
export REPOSE_SSH_WRONG_HOST_KEY="$tmp/wrong_host_key.pub"
export REPOSE_SSH_REQUIRED=1

"$@"
