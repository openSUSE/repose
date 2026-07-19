# Improve test coverage

## Current state

CI does not check coverage; it only runs tests at `.github/workflows/ci.yml:43-44`. Current line coverage is 81.47%, with major gaps in CLI wiring (21.66%) and SSH session/host behavior (about 22–26%).

Assumptions: use a ratcheted workspace line baseline, run containerized OpenSSH tests on pull requests, generate test keys dynamically, and avoid production refactoring unless required for testability.

## Plan

1. [small] Add process-level CLI tests in `crates/repose-cli/tests/cli.rs` covering help/version, conflicting flags, missing command, invalid host/REPA, known-products text/JSON, exit codes, and color behavior — verify: CLI integration tests pass and `lib.rs` coverage increases.
2. [medium] Add an OpenSSH fixture image in `tests/ssh/Dockerfile`, pinned to an immutable base, with SFTP and representative product/repository files — verify: the container passes a health check and accepts only the generated test key.
3. [small] Add `tests/ssh/run.sh` to generate temporary host/client keys, start the container on an ephemeral port, export connection details, and always clean up — verify: no credentials are committed and repeated runs leave no containers or files behind.
4. [large] Add `crates/repose-ssh/tests/ssh_integration.rs` covering public-key connection, host-key policies, stdout/stderr/exit status, timeout, SFTP list/read/readlink, cached SFTP reuse, close/reconnect, system discovery, and per-host failure isolation — verify: tests fail when transport behavior is deliberately broken and pass against the fixture.
5. [medium] Extend CLI integration coverage through the SSH fixture for `list-products`, `list-repos`, and one dry-run mutation command, checking text/NDJSON and process exit codes — verify: binary behavior succeeds end-to-end without external network access.
6. [small] Add `scripts/check-coverage.sh` running `cargo llvm-cov --workspace --all-targets --locked`, producing text/LCOV reports and enforcing the committed line-coverage baseline — verify: lowering observed coverage below the baseline makes the script fail.
7. [small] Add a coverage job to `.github/workflows/ci.yml`: install pinned `cargo-llvm-cov`, start the SSH fixture, run the coverage script, and upload the LCOV artifact — verify: pull-request CI displays a coverage job and rejects regressions.
8. [trivial] Update `tests/vectors/inventory.md` to document the SSH fixture, covered scenarios, local command, and ratchet procedure — verify: the instructions reproduce the CI check.
9. [small] Update `crates/Cargo.lock` only if test tooling introduces dependencies — verify: `cargo test --workspace --all-targets --locked` succeeds.

## Files

- Modify:
  - `.github/workflows/ci.yml`
  - `tests/vectors/inventory.md`
  - `crates/Cargo.lock` if needed
- Create:
  - `crates/repose-cli/tests/cli.rs`
  - `crates/repose-ssh/tests/ssh_integration.rs`
  - `tests/ssh/Dockerfile`
  - `tests/ssh/run.sh`
  - `scripts/check-coverage.sh`
- Delete: none

## Risks

- Container startup can make CI slower or flaky; use health checks, pinned images, ephemeral ports, and deterministic retries.
- Aggregate coverage can mask regressions in low-covered crates; record workspace and per-crate results, with SSH/CLI floors once stable.
- SIGINT and timeout tests can become timing-sensitive; use bounded synchronization rather than fixed sleeps.

## Alternatives considered

- Mocking russh internals — rejected because it would not validate authentication, protocol, SFTP, or host-key behavior.
- Report-only coverage — rejected because it permits silent regressions.

## Success criteria

- [ ] All existing 172 tests still pass.
- [ ] New CLI and SSH integration tests pass locally and in CI.
- [ ] CI publishes an LCOV artifact and fails below the ratcheted baseline.
- [ ] CLI and SSH coverage materially rises from the current 21–26%.
- [ ] `cargo fmt`, Clippy, layering, locked tests, and `cargo deny check` pass.
- [ ] No private keys, passwords, or persistent containers remain.
