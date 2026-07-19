#!/usr/bin/env bash
#
# CLI consistency self-check for the Rust `repose` binary: known-products must
# byte-match the committed expected output (tests/vectors/cli/known_products.txt),
# plus CLI-surface invariants (no --ssh-backend, version-line shape, all nine
# subcommands). Fully self-contained — no external `repose` is consulted.
#
# Env:
#   REPOSE_RS   command for the Rust binary (default: target/debug/repose).
#   FIXTURE     products.yml used for known-products (default: the committed
#               sample fixture).
#   GOLDEN      expected known-products output (default: the committed vector).
#
# Run from the repository root.
set -euo pipefail

read -ra RS <<<"${REPOSE_RS:-target/debug/repose}"
FIXTURE="${FIXTURE:-tests/vectors/template/sample.yml}"
GOLDEN="${GOLDEN:-tests/vectors/cli/known_products.txt}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
	echo "CLI CHECK FAIL: $*" >&2
	exit 1
}
ok() { echo "  ok: $*"; }

# Run `<binary> -c FIXTURE known-products` into $2, or fail with diagnostics.
# `-c` must precede the subcommand (it is a global option).
known_products() {
	local label="$1" out="$2"
	shift 2
	"$@" -c "$FIXTURE" known-products >"$out" 2>"$tmp/err" ||
		{
			cat "$tmp/err" >&2
			fail "$label known-products exited non-zero"
		}
}

[ -f "${RS[0]}" ] || fail "Rust binary not found: ${RS[*]} (build: cargo build -p repose-cli)"
[ -f "$FIXTURE" ] || fail "fixture not found: $FIXTURE"
[ -f "$GOLDEN" ] || fail "expected output not found: $GOLDEN"

# 1. known-products == committed expected output (byte-exact, trailing bytes included).
known_products "Rust" "$tmp/rs.txt" "${RS[@]}"
if ! cmp -s "$tmp/rs.txt" "$GOLDEN"; then
	diff "$GOLDEN" "$tmp/rs.txt" >&2 || true
	fail "known-products differs from committed expected output $GOLDEN"
fi
ok "known-products == committed expected output (byte-exact)"

# 2. Must NOT expose --ssh-backend (intentional single-backend CLI surface).
if "${RS[@]}" --help 2>&1 | grep -q -- '--ssh-backend'; then
	fail "help reintroduced --ssh-backend"
fi
ok "no --ssh-backend in help"

# 3. Version line shape: `repose version: X.Y.Z` (whole line).
if ! "${RS[@]}" --version 2>&1 | grep -Eq '^repose version: [0-9]+\.[0-9]+\.[0-9]+$'; then
	fail "--version line does not match 'repose version: X.Y.Z'"
fi
ok "version line shape"

# 4. All nine subcommands present in help, matched in the command column
#    (anchored so e.g. `uninstall` cannot stand in for a removed `install`).
rs_help="$("${RS[@]}" --help 2>&1)"
for sub in add remove reset install clear uninstall list-products list-repos known-products; do
	printf '%s\n' "$rs_help" | grep -qE "^[[:space:]]+${sub}([[:space:]]|\$)" ||
		fail "subcommand missing from help command column: $sub"
done
ok "all nine subcommands present"

echo "CLI CHECK OK"
