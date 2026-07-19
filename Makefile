# repose — Rust workspace under crates/. Convenience targets; CI runs the
# same commands (see .github/workflows/ci.yml).
.PHONY: build release test fmt fmt-fix clippy deny layer cli assets check install clean

build:        ## debug build (locked)
	cd crates && cargo build --locked
release:      ## optimized `repose` binary
	cd crates && cargo build --release --locked -p repose-cli
test:         ## workspace tests
	cd crates && cargo test --workspace --all-targets --locked
fmt:          ## check formatting
	cd crates && cargo fmt --all -- --check
fmt-fix:      ## apply formatting
	cd crates && cargo fmt --all
clippy:       ## lint (deny warnings), incl. the `gen` feature
	cd crates && cargo clippy --workspace --all-targets --locked -- -D warnings
	cd crates && cargo clippy --workspace --all-targets --features gen --locked -- -D warnings
deny:         ## dependency policy
	cd crates && cargo deny check
layer:        ## enforce core -/-> ssh layering
	bash scripts/check-rust-layering.sh
cli:          ## CLI consistency self-check vs committed expected output
	bash scripts/check-cli.sh
assets:       ## regenerate committed man pages + shell completions
	cd crates && cargo run --locked -p repose-cli --features gen --bin repose-gen -- repose-cli
check: fmt clippy test deny layer cli  ## everything CI runs
install:      ## install `repose` into ~/.cargo/bin
	cd crates && cargo install --path repose-cli --locked
clean:
	cd crates && cargo clean
