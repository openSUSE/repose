#!/usr/bin/env bash
# Fail if repose-core depends on repose-ssh or russh (design layering).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="${ROOT}/crates/Cargo.toml"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found" >&2
  exit 1
fi

tree="$(cargo tree --manifest-path "$MANIFEST" -p repose-core --edges normal --prefix none --format '{p}')"

if echo "$tree" | grep -E '(^| )repose-ssh( |$)|(^| )russh( |$)|russh-' >/dev/null; then
  echo "LAYERING VIOLATION: repose-core must not depend on repose-ssh or russh" >&2
  echo "$tree" >&2
  exit 1
fi

# Positive check: core package appears.
if ! echo "$tree" | grep -q 'repose-core'; then
  echo "unexpected cargo tree output for repose-core" >&2
  echo "$tree" >&2
  exit 1
fi

echo "layering ok: repose-core has no repose-ssh/russh dependency"
