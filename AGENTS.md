# Rust Workspace Guidelines

These instructions apply to this repository — a root Rust workspace with
sources under `crates/`.

## Workspace Commands

Run commands from the repository root:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features gen -- -D warnings
cargo test --workspace --all-targets --locked
cargo deny check
```

The second clippy line is not optional: without `--features gen` the
`repose-gen` binary is skipped entirely (required-features off), so a
violation there passes the first line and fails only the gen leg.

Use the toolchain pinned in `rust-toolchain.toml` (repo root). The workspace MSRV is
declared once in `Cargo.toml` (`rust-version = "1.96"`); do not introduce APIs
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

Run `scripts/check-rust-layering.sh` after changing Cargo dependencies or
crate boundaries.

## The Python Implementation Is Not a Reference

repose was ported from a Python implementation, removed at the 3.0.0 cutover in
`eaef9ce`. The last Python release is the `2.1.0` tag, and the tree is readable
at `git show eaef9ce^:repose/...`. **Do not treat it as an authority.** Matching
its behavior is not a goal, and "Python did X" is not a justification. Read the
old tree to work out *why* a shape exists — then record what you learn as a
statement about the constraint, not as a citation.

- **Never** preserve a bug, a typo, or an awkward shape for parity with it.
  Fixing one is welcome — **in its own commit**, with the affected vector under
  `tests/vectors/` updated alongside it. Retiring a *rationale* is prose;
  changing the *bytes* is not, and the two must not ride in the same commit.
- **First check the shape is not a contract in disguise** (see "Output and
  Wire Contracts"). A stale citation is not evidence a shape is free to change:
  the `http://empty.url` sentinel (`repoq.rs`) was documented in-tree as what a
  Python `dict.get` default returned, yet it reaches a refhost inside a real
  `zypper ar` argument and is pinned in `tests/vectors/repoq/`. Replace the dead
  rationale; keep the constraint.
- A note recording a **deliberate departure** is a guard-rail, not a citation —
  it stays. State the choice positively ("parsed fields are whitespace-trimmed";
  "malformed XML yields an empty result rather than an error") instead of
  comparatively ("Python keeps the padding"; "Python raises `ParseError`").
  Where a line is a bare identifier citation ("Python `AsyncTarget`"), keep any
  behavioral description and drop the citation; if the line was *only* a
  citation, drop the line.
- **"Python" is also a domain word here**, and those uses are not citations: the
  `sle-module-python3` product under `tests/vectors/refhosts/` (its `.prod`
  file, CPE and `obsproduct://` URIs, and `:pool`/`:update` repo aliases), the
  prose of a captured SUSE product description ("PHP, Ruby on Rails, and
  Python."), and the live `python3` one-liners that pick a free port in
  `scripts/*-container.sh`. `tests/vectors/` is captured data, not prose — never
  edit it for wording.
- `typos.toml` is an allow-list of established spellings, not of citations.
  Retire an entry only once its last real occurrence is gone, and say so.
- Do not sweep on the word `upstream` either. It appears here as fixture data (a
  refhost repository whose recorded name ends in `- UPSTREAM`) and in its
  ordinary sense in `ci.yml`, but never as a reference to the removed tree.

Note `cargo doc` (the `docs` job, `-D warnings`) and `typos` (its own workflow)
both gate documentation changes, and neither is part of `make check`.

## Output and Wire Contracts

These shapes have consumers outside this repository. Changing one is a breaking
change, whatever a comment nearby says about where it came from.

- **Remote command strings** — the templates in `shell::cmd` and the quoting in
  `repose_core::shell` (both crate-private) produce the literal bytes a
  refhost's `/bin/sh` executes. All user- or template-derived arguments go
  through those helpers; `tests/vectors/shell/` pins the result.
- **NDJSON output** (`--format=json`) — key order, `", "` / `": "` separators,
  and `\uXXXX` escaping of non-ASCII are consumed by scripts. The field-level
  schemas are documented in `crates/README.md`.
- **`list-products --yaml`** — the host spec feeding the `refhosts.yml`
  generator.
- **`products.yml`** — the template schema is shared with the separate package
  that owns `/etc/repose/products.yml`, including its YAML merge keys.
- **Text output of the `list-*` and `known-products` commands**, ANSI color
  sequences included. The `list-*` goldens are asserted in
  `crates/repose-core/tests/refhost_vectors.rs`; `known-products` and the color
  sequences in `crates/repose-cli/tests/cli.rs`.
- **`repose version: X.Y.Z`** — the `--version` line's shape, asserted by CI.
- **Process exit codes** (`ExitCode`, and the zypper exit codes treated as
  success) — callers branch on them.
- **User-facing error text pinned by a vector**, such as `Unknow version: ...`.

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
- The committed vectors in `tests/vectors/` define the binary's expected
  output; update them only for an intentional, documented behavior change.
- Parser fuzz targets live in `fuzz/` (cargo-fuzz, nightly-only, detached
  from the workspace so they do not affect the MSRV, `Cargo.lock`, or
  `cargo deny`). CI runs them via ClusterFuzzLite (the `fuzz` workflow);
  locally use `cargo +nightly fuzz run <target>`.
- A dependency change must update `Cargo.lock`, preserve the MSRV, and pass
  `cargo deny check`. Prefer the smallest compatible version change; do not
  run a broad `cargo update` as part of an unrelated change.
