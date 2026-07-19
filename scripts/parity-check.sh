#!/usr/bin/env bash
#
# Parity harness for the Rust `repose` binary. Since the Python cutover the
# checks are golden-first: Rust known-products must byte-match the committed
# golden (tests/oracle/parity/known_products.txt), plus CLI-surface invariants
# (no --ssh-backend, version-line shape, all nine subcommands). If a `repose`
# command still happens to be on PATH it is compared too, but the Python oracle
# is no longer part of the tree or CI.
#
# Env:
#   REPOSE_PY   optional reference `repose` to cross-check against (default:
#               repose). Compared only if present — the Python oracle is gone.
#   REPOSE_RS   command for the Rust binary (default: crates/target/debug/repose).
#   FIXTURE     products.yml used for known-products (default: the committed
#               sample fixture).
#   PARITY_REQUIRE_PY  when set to 1, fail if the Python oracle is unavailable
#               instead of skipping the cross-implementation comparison (CI).
#
# Run from the repository root.
set -euo pipefail

read -ra PY <<<"${REPOSE_PY:-repose}"
read -ra RS <<<"${REPOSE_RS:-crates/target/debug/repose}"
FIXTURE="${FIXTURE:-tests/oracle/template/sample.yml}"
GOLDEN="tests/oracle/parity/known_products.txt"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
  echo "PARITY FAIL: $*" >&2
  exit 1
}
ok() { echo "  ok: $*"; }

# Run `<binary> -c FIXTURE known-products` into $2, or fail with diagnostics.
# `-c` must precede the subcommand (Typer only accepts the global option there).
known_products() {
  local label="$1" out="$2"
  shift 2
  "$@" -c "$FIXTURE" known-products >"$out" 2>"$tmp/err" ||
    { cat "$tmp/err" >&2; fail "$label known-products exited non-zero"; }
}

[ -f "${RS[0]}" ] || fail "Rust binary not found: ${RS[*]} (build: cargo build -p repose-cli --manifest-path crates/Cargo.toml)"
[ -f "$FIXTURE" ] || fail "fixture not found: $FIXTURE"
[ -f "$GOLDEN" ] || fail "golden not found: $GOLDEN"

# 1. Rust known-products == committed golden (byte-exact, trailing bytes included).
known_products "Rust" "$tmp/rs.txt" "${RS[@]}"
if ! cmp -s "$tmp/rs.txt" "$GOLDEN"; then
  diff "$GOLDEN" "$tmp/rs.txt" >&2 || true
  fail "Rust known-products differs from committed golden $GOLDEN"
fi
ok "Rust known-products == committed golden (byte-exact)"

# 2. Rust must NOT expose --ssh-backend (intentional single-backend delta).
if "${RS[@]}" --help 2>&1 | grep -q -- '--ssh-backend'; then
  fail "Rust help reintroduced --ssh-backend"
fi
ok "no --ssh-backend in Rust help"

# 3. Version line shape: `repose version: X.Y.Z` (whole line).
if ! "${RS[@]}" --version 2>&1 | grep -Eq '^repose version: [0-9]+\.[0-9]+\.[0-9]+$'; then
  fail "Rust --version line does not match 'repose version: X.Y.Z'"
fi
ok "version line shape"

# 4. All nine subcommands present in Rust help, matched in the command column
#    (anchored so e.g. `uninstall` cannot stand in for a removed `install`).
rs_help="$("${RS[@]}" --help 2>&1)"
for sub in add remove reset install clear uninstall list-products list-repos known-products; do
  printf '%s\n' "$rs_help" | grep -qE "^[[:space:]]+${sub}([[:space:]]|\$)" ||
    fail "subcommand missing from Rust help command column: $sub"
done
ok "all nine subcommands present"

# 5. Cross-implementation: Python oracle == Rust on known-products (if available).
if "${PY[@]}" --version >/dev/null 2>&1; then
  known_products "Python" "$tmp/py.txt" "${PY[@]}"
  if ! cmp -s "$tmp/py.txt" "$tmp/rs.txt"; then
    diff "$tmp/py.txt" "$tmp/rs.txt" >&2 || true
    fail "Python vs Rust known-products differ"
  fi
  ok "Python oracle == Rust known-products (byte-exact)"
elif [ "${PARITY_REQUIRE_PY:-}" = "1" ]; then
  fail "Python oracle (${PY[*]}) unavailable but PARITY_REQUIRE_PY=1"
else
  echo "  note: Python oracle (${PY[*]}) unavailable — skipped cross-impl compare"
fi

echo "PARITY OK"
