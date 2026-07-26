# Vector inventory

Committed regression vectors capturing the Rust `repose` binary's expected
output and parsing behavior. Rust tests consume them directly; the vectors
define expected behavior and are maintained by hand — update a vector only for
an intentional, documented behavior change.

| Vector | Rust consumer | Covers |
| --- | --- | --- |
| `repa/parse.json` | `repose_core::repa` | REPA parsing (`product:version:arch:repo`) |
| `shell/quote.json` | `repose_core::shell` | POSIX quoting of one argument |
| `shell/join.json` | `repose_core::shell` | quoting and joining an argument list |
| `shell/command_templates.json` | `repose_core::shell::cmd` | the built remote command templates (not the fixed `REFCMD`/`REFTCMD`/`REBOOT` constants) |
| `repoq/solve_repa.json` | `repose_core::repoq` | REPA to repository resolution, the fallback URL for an absent repo key, and the resolution errors |
| `template/sample.yml` | `repose_core::template` | the `products.yml` schema |
| `transform/version.json` | `repose_core::transform` | version normalization for the refhost YAML |
| `hostparse/hosts.json` | `repose_core::host_parse` | `[user@]host[:port]` target parsing |
| `ndjson/events.jsonl` | `repose_core::console` | that every emitted line parses as JSON carrying an `event` key (field-level shapes live in `crates/README.md`) |
| `product/parse_prod.json` | `repose_core::product_parse` | `/etc/products.d/*.prod` XML |
| `product/os_release.json` | `repose_core::product_parse` | the `/etc/os-release` fallback |
| `zypper_lr/parse.json` | `repose_core::repo_parse` | `zypper -x lr` XML |
| `remove_match/repolist.json` | `repose_core::commands::remove` | which aliases a `remove` pattern matches |
| `sequences/reset.json` | `repose_core::commands::reset` tests | the `reset` command sequence |
| `sequences/install.json` | `repose_core::commands::install` tests | the `install` command sequence |
| `sequences/uninstall.json` | `repose_core::commands::uninstall` tests | the `uninstall` command sequence |

L2 sequence vectors (`sequences/*.json`) are the per-scenario expected
remote-command sequences (`ran`), dry-run preview lines (`dry`), and aggregate
`exit` for the mutation commands. They are asserted by the `#[cfg(test)]`
modules in `repose-core::commands::{reset,install,uninstall}` via
`commands::seq`. Set-sourced alias/command order follows the Rust stable sort.

The CLI vector (`cli/known_products.txt`) is the committed `known-products`
output for `template/sample.yml`. `scripts/check-cli.sh` asserts the Rust
binary matches this expected output byte-for-byte and upholds the CLI-surface
invariants (no `--ssh-backend`, `repose version: X.Y.Z` shape, all nine
subcommands). Run in CI by the `rust-cli` job.

## OpenSSH integration and coverage

`tests/ssh/Dockerfile` provides an isolated OpenSSH server with SFTP, the
`sles-16-0` product and repository vectors, and a deterministic `zypper -x lr`
response. `tests/ssh/run.sh` generates fresh client and host keys, publishes
SSH on an ephemeral loopback port, waits for the container health check, runs
the supplied command, and removes the container and keys on exit.

The live tests cover public-key authentication, strict/accept-new/off host-key
policies, command streams/status/timeout, SFTP list/read/readlink and reuse,
close/reconnect, system/repository discovery, per-host failure isolation, CLI
`list-products`, CLI `list-repos`, and dry-run `clear` in text and NDJSON.

Run the same coverage check as CI from the repository root:

```sh
tests/ssh/run.sh scripts/check-coverage.sh
```

The command writes `coverage/coverage.txt`, `coverage/lcov.info`, and
`coverage/summary.json`. `scripts/check-coverage.sh` rejects workspace line
coverage below its committed baseline. After adding meaningful tests, raise
`LINE_BASELINE` in that script to the rounded-down observed percentage; never
lower it without a reviewed explanation for the coverage loss.

Normative rule: the committed vectors beat the design doc — they define the
binary's expected behavior and are updated deliberately, never regenerated
from another implementation.
