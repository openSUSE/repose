# repose Rust workspace

In-progress rewrite of openSUSE/repose (see `docs/design/rust-rewrite.md`).

## Layout

| Crate | Role |
| --- | --- |
| `repose-core` | Pure logic, traits, command algorithms (**no russh**) |
| `repose-ssh` | Single SSH backend implementing core traits |
| `repose-cli` | Binary `repose` |

**Dependency direction:** `repose-cli` → `{repose-core, repose-ssh}`; `repose-ssh` → `repose-core`; `repose-core` must never depend on `repose-ssh` or `russh`.

## Commands

```bash
# from repository root
cargo test --manifest-path crates/Cargo.toml
cargo fmt --manifest-path crates/Cargo.toml --all -- --check
cargo clippy --manifest-path crates/Cargo.toml --workspace --all-targets -- -D warnings
cargo deny --manifest-path crates/Cargo.toml check   # needs cargo-deny >= 0.18.4 (0.20.x preferred)
cargo run --manifest-path crates/Cargo.toml -p repose-cli
```

Or `cd crates` and run the same commands without `--manifest-path`.

## Policy notes

- **Layering:** `repose-core` must never depend on `repose-ssh` or `russh` (enforced more strictly in PR0.5).
- **Single SSH backend:** `crates/deny.toml` bans `ssh2` / `libssh2-sys` / async-ssh2-* crates.
- **Path deps** pin `version = "0.1.0"` so `cargo-deny` `wildcards = "deny"` accepts them.
- **Binary name:** `repose` (replace strategy; no `repose-rs`).

Python sources under `repose/` remain the behavioral oracle until cutover.
