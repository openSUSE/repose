#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COVERAGE_DIR="${COVERAGE_DIR:-$ROOT/coverage}"
LINE_BASELINE="${REPOSE_COVERAGE_LINE_BASELINE:-83.09}"

command -v cargo-llvm-cov >/dev/null || {
	echo "cargo-llvm-cov is required: cargo install cargo-llvm-cov --locked" >&2
	exit 2
}
command -v jq >/dev/null || {
	echo "jq is required" >&2
	exit 2
}

mkdir -p "$COVERAGE_DIR"
cd "$ROOT"

cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --all-targets --locked --no-report
cargo llvm-cov report --text --show-missing-lines --output-path "$COVERAGE_DIR/coverage.txt"
cargo llvm-cov report --lcov --output-path "$COVERAGE_DIR/lcov.info"
cargo llvm-cov report --summary-only --json --output-path "$COVERAGE_DIR/summary.json"

line_percent="$(jq -r '.data[0].totals.lines.percent' "$COVERAGE_DIR/summary.json")"
printf 'Workspace line coverage: %.2f%% (baseline %.2f%%)\n' "$line_percent" "$LINE_BASELINE"

if ! awk -v actual="$line_percent" -v baseline="$LINE_BASELINE" 'BEGIN { exit !(actual + 0.000001 >= baseline) }'; then
	echo "line coverage ${line_percent}% is below the committed baseline ${LINE_BASELINE}%" >&2
	exit 1
fi
