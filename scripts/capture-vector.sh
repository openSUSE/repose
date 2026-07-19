#!/usr/bin/env bash
# capture-vector.sh — record one refhost as a regression vector under
# tests/vectors/refhosts/<label>/.
#
# Usage: scripts/capture-vector.sh <host> <label>
#
# Connects to <host> as root (BatchMode: your key must already work) and
# fetches exactly the discovery inputs repose reads over SSH:
#
#   /etc/os-release                       -> os-release
#   readlink /etc/products.d/baseproduct  -> baseproduct-target (basename)
#   /etc/products.d/*.prod                -> products.d/*.prod
#   transactional-update.conf presence    -> transactional ("true"/"false")
#   zypper -x lr                          -> zypper-x-lr.xml
#
# It then SANITIZES the capture in place — this repository is PUBLIC — and
# verifies no internal URL/hostname survives. Sanitization rules (documented
# in tests/vectors/refhosts/README.md):
#
#   * every http(s) URL: scheme+host replaced by https://example.invalid, the
#     PATH is kept verbatim so distinct repos stay distinct
#   * repoid="obsrepository://..." -> obsrepository://example.invalid/scrubbed
#   * leftover internal hostnames (*.suse.cz, *.suse.de, qam.suse*) -> example.invalid
#
# Product-identity elements (name/version/arch/baseversion/patchlevel/
# codestream) and repo alias/enabled/autorefresh flags are never touched.
#
# Idempotent: re-running overwrites the capture and re-applies the same
# sanitization; sanitizing already-sanitized files is a no-op.
#
# Afterwards, generate the expected outputs with the UPDATE step it prints.

set -euo pipefail

usage() {
    echo "usage: $0 <host> <label>" >&2
    exit 2
}

[ $# -eq 2 ] || usage
host=$1
label=$2
case $label in
'' | . | .. | */* | -*)
    echo "error: <label> must be a plain directory name (got '$label')" >&2
    exit 2
    ;;
esac

repo_root=$(cd "$(dirname "$0")/.." && pwd)
dir="$repo_root/tests/vectors/refhosts/$label"

run() {
    ssh -o BatchMode=yes -n -l root "$host" "$@"
}

echo ">> capturing $host -> tests/vectors/refhosts/$label/"
mkdir -p "$dir/products.d"
rm -f "$dir/products.d"/*.prod # idempotency: drop stale .prod from earlier runs

run cat /etc/os-release >"$dir/os-release"

base_target=$(run readlink /etc/products.d/baseproduct)
basename "$base_target" >"$dir/baseproduct-target"

# One connection for all .prod files; extracted into products.d/.
run 'cd /etc/products.d && tar -cf - -- *.prod' | tar -xf - -C "$dir/products.d"

# Same probe order as repose (repose_core::product_parse::TRANSACTIONAL_CONF_PATHS).
if run 'test -e /usr/etc/transactional-update.conf || test -e /etc/transactional-update.conf'; then
    echo true >"$dir/transactional"
else
    echo false >"$dir/transactional"
fi

# zypper exit 6 (no repos) / 106 still emit valid XML; the live path accepts them.
rc=0
run 'zypper -x lr' >"$dir/zypper-x-lr.xml" || rc=$?
case $rc in
0 | 6 | 106) ;;
*)
    echo "error: 'zypper -x lr' on $host exited $rc" >&2
    exit 1
    ;;
esac

echo ">> sanitizing (public repo)"
sanitize() {
    # Rule order matters: obsrepository:// first (distinct scheme, its path IS
    # internal build-service data, so it is fully scrubbed), then http(s)
    # (host swapped, path preserved for uniqueness), then bare leftover
    # internal hostnames. Each rule maps its own output to itself: idempotent.
    sed -i -E \
        -e 's#obsrepository://[^"<[:space:]]*#obsrepository://example.invalid/scrubbed#g' \
        -e "s#https?://[^/\"'<>[:space:]]*#https://example.invalid#g" \
        -e 's#[A-Za-z0-9.-]*\.suse\.(cz|de)#example.invalid#g' \
        -e 's#qam\.suse[A-Za-z0-9.-]*#example.invalid#g' \
        -e 's#/ibs/#/scrubbed/#g' \
        "$1"
}

sanitize "$dir/os-release"
sanitize "$dir/zypper-x-lr.xml"
for prod in "$dir/products.d"/*.prod; do
    sanitize "$prod"
done

# Hard gate: nothing internal may survive under the vector directory.
leak_re='https?://[^[:space:]"<>'\'']*|obsrepository://[^[:space:]"<>]*|[A-Za-z0-9.-]+\.suse\.(cz|de)|qam\.suse'
if grep -rEoh "$leak_re" "$dir" | grep -Ev '^(https|obsrepository)://example\.invalid([/"<[:space:]]|$)' >/dev/null; then
    echo "error: sanitization left internal identifiers behind:" >&2
    grep -rEo "$leak_re" "$dir" | grep -Ev 'example\.invalid' >&2
    exit 1
fi

echo ">> done: $dir"
echo
echo "Next steps:"
echo "  1. UPDATE_VECTORS=1 cargo test -p repose-core --test refhost_vectors"
echo "     (writes list-products.{text,json,yaml} + list-repos.{text,json} goldens)"
echo "  2. Review: git status && git diff tests/vectors/refhosts/$label"
echo "  3. Commit the new vector."
