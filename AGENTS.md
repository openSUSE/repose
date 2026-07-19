# Rust Workspace Guidelines

These instructions apply to this repository — a Rust workspace
(sources under `crates/`).

## Workspace Commands

Run commands from `crates/`:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets --locked
cargo deny check
```

Use the toolchain pinned in `rust-toolchain.toml` (repo root). The workspace MSRV is
declared once in `Cargo.toml` (`rust-version = "1.85"`); do not introduce APIs
or dependencies which require a newer compiler without deliberately updating
both the pin and the MSRV policy. CI and reproducible local checks use the
committed `Cargo.lock`, so use `--locked` for verification.

## Workspace Architecture

- `repose-core` is the portable domain layer. It must not depend on `russh`,
  `repose-ssh`, or transport-specific types.
- `repose-ssh` implements the `repose-core` traits and is the only SSH
  transport. Keep all russh/russh-sftp code here.
- `repose-cli` owns clap parsing, process exit mapping, and wiring. It must
  not duplicate command algorithms.
- Keep the dependency direction acyclic:
  `repose-cli -> repose-core`, `repose-cli -> repose-ssh`, and
  `repose-ssh -> repose-core`.

Run `../scripts/check-rust-layering.sh` after changing Cargo dependencies or
crate boundaries.

## Rust Style and APIs

- Follow `rustfmt`; use idiomatic ownership and borrowing rather than cloning
  to resolve a lifetime issue by default.
- Public items need rustdoc that explains purpose, relevant errors, and
  behavioral constraints. Keep crate documentation accurate.
- Return the project's typed errors with actionable context. Do not use
  `unwrap`, `expect`, or panics in recoverable production paths.
- Keep `unsafe` forbidden. Do not weaken workspace lint configuration merely
  to silence a new warning.
- Prefer small, focused functions and exhaustive `match` expressions for
  externally meaningful enums and protocol states.

## Async, SSH, and Security

- Do not block Tokio worker threads with synchronous I/O, sleeps, or process
  calls. Bound network operations with the configured timeout.
- Preserve per-host failure isolation: one target failure must not cancel the
  rest of a host group.
- Treat host-key policy as security-sensitive: `yes` rejects unknown/changed
  keys; `accept-new` persists only first-contact keys; `no`/`off` explicitly
  disable validation. Never log passwords, private keys, or secret material.
- Use the cached SFTP subsystem for remote filesystem operations. Do not
  replace file operations with remote shell commands.
- All user- or template-derived remote command arguments must use the shared
  `repose_core::shell` quoting/joining helpers and their golden tests.

## Tests and Dependencies

- Add focused unit tests alongside changed modules. Command algorithms test
  against `repose_core::traits::{Host, HostGroup, Probe}` using mocks; reserve
  live transport behavior for `repose-ssh` integration tests.
- Update committed oracle vectors in `tests/oracle/` only when Python-oracle
  behavior changes or a documented intentional delta is approved.
- A dependency change must update `Cargo.lock`, preserve the MSRV, and pass
  `cargo deny check`. Prefer the smallest compatible version change; do not
  run a broad `cargo update` as part of an unrelated change.
