# repose — Rust workspace convenience targets; CI runs the
# same commands (see .github/workflows/ci.yml).
.PHONY: build release test fmt fmt-fix clippy deny layer cli assets check install clean \
	perf-smoke perf-baseline perf-compare-test

build:        ## debug build (locked)
	cargo build --locked
release:      ## optimized `repose` binary
	cargo build --release --locked -p repose-cli
test:         ## workspace tests
	cargo test --workspace --all-targets --locked
fmt:          ## check formatting
	cargo fmt --all -- --check
fmt-fix:      ## apply formatting
	cargo fmt --all
clippy:       ## lint (deny warnings), incl. the `gen` feature
	cargo clippy --workspace --all-targets --locked -- -D warnings
	cargo clippy --workspace --all-targets --features gen --locked -- -D warnings
deny:         ## dependency policy
	cargo deny check
layer:        ## enforce core -/-> ssh layering
	bash scripts/check-rust-layering.sh
cli:          ## CLI consistency self-check vs committed expected output
	bash scripts/check-cli.sh
assets:       ## regenerate committed man pages + shell completions
	cargo run --locked -p repose-cli --features gen --bin repose-gen -- crates/repose-cli
check: fmt clippy test deny layer cli  ## everything CI runs
install:      ## install `repose` into ~/.cargo/bin
	cargo install --path crates/repose-cli --locked
clean:
	cargo clean

# Performance (tests/performance/README.md). Not part of `check` — these
# are release builds and, for perf-baseline, real timing/RSS measurements
# that don't belong in the default fast feedback loop.
perf-smoke:    ## fleet benchmark structural smoke: every ID runs once, no statistics
	cargo bench -p repose-core --bench fleet --locked -- --test
perf-baseline: ## full release baseline report for every mock/ssh workload
	bash scripts/run-performance-baseline.sh
perf-compare-test: ## comparator fixtures + one real end-to-end regression guardrail
	bash scripts/test-compare-performance.sh
