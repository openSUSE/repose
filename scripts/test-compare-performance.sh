#!/usr/bin/env bash
#
# Comparator unit tests: runs scripts/compare-performance.sh against every
# fixture under tests/performance/comparator-fixtures/ and checks the exit
# code matches the fixture's declared `expect`. Also runs one real
# end-to-end guardrail using the baseline_report harness's controllable
# slowdown (REPOSE_PERF_INJECT_DELAY_MS) instead of a static fixture.
#
# Run from the repository root.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPARE="$ROOT/scripts/compare-performance.sh"
FIXTURES="$ROOT/tests/performance/comparator-fixtures"

expect_to_code() {
	case "$1" in
	pass) echo 0 ;;
	regression) echo 1 ;;
	contract-failure) echo 2 ;;
	incomparable-metadata) echo 3 ;;
	*)
		echo "unknown expect: $1" >&2
		exit 2
		;;
	esac
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

failures=0
for fixture in "$FIXTURES"/*.json; do
	name="$(basename "$fixture" .json)"
	expect="$(jq -r '.expect' "$fixture")"
	want_code="$(expect_to_code "$expect")"

	jq '.baseline' "$fixture" >"$tmp/baseline.json"
	jq '.candidate' "$fixture" >"$tmp/candidate.json"

	set +e
	bash "$COMPARE" "$tmp/baseline.json" "$tmp/candidate.json" >"$tmp/out.log" 2>&1
	got_code=$?
	set -e

	if [[ "$got_code" -eq "$want_code" ]]; then
		echo "ok: $name (expected exit $want_code, got $got_code)"
	else
		echo "FAIL: $name (expected exit $want_code, got $got_code)"
		sed 's/^/    /' "$tmp/out.log"
		failures=1
	fi
done

echo "-- end-to-end guardrail: a real injected slowdown must be caught (not a static fixture) --"
cargo build --release --locked -p repose-core --example baseline_report >/dev/null
BASELINE_REPORT="$ROOT/target/release/examples/baseline_report"

"$BASELINE_REPORT" mock-add-1h 15 3 >"$tmp/e2e-baseline.json"
"$BASELINE_REPORT" mock-add-1h 15 3 >"$tmp/e2e-unchanged.json"
REPOSE_PERF_INJECT_DELAY_MS=5 "$BASELINE_REPORT" mock-add-1h 15 3 >"$tmp/e2e-slowed.json"

set +e
bash "$COMPARE" "$tmp/e2e-baseline.json" "$tmp/e2e-unchanged.json" >"$tmp/e2e-unchanged.log" 2>&1
unchanged_code=$?
bash "$COMPARE" "$tmp/e2e-baseline.json" "$tmp/e2e-slowed.json" >"$tmp/e2e-slowed.log" 2>&1
slowed_code=$?
set -e

if [[ "$unchanged_code" -eq 0 ]]; then
	echo "ok: unchanged real run passes"
else
	echo "FAIL: unchanged real run should pass, got exit $unchanged_code"
	sed 's/^/    /' "$tmp/e2e-unchanged.log"
	failures=1
fi
if [[ "$slowed_code" -eq 1 ]]; then
	echo "ok: actually-slowed real run is caught as a regression"
else
	echo "FAIL: slowed real run should regress (exit 1), got exit $slowed_code"
	sed 's/^/    /' "$tmp/e2e-slowed.log"
	failures=1
fi

exit "$failures"
