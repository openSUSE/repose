# Vector inventory

Committed regression vectors capturing the Rust `repose` binary's expected
output and parsing behavior. Rust tests consume them directly; the vectors
define expected behavior and are maintained by hand — update a vector only for
an intentional, documented behavior change.

| Vector | Rust consumer | Historical source (Python 2.1.0) |
| --- | --- | --- |
| `repa/parse.json` | `repose_core::repa` | `tests/types/test_repa.py` |
| `shell/quote.json` | `repose_core::shell` | `shlex.quote` / `tests/command/test_shell_quoting.py` |
| `shell/join.json` | `repose_core::shell` | `shlex.join` |
| `shell/command_templates.json` | `repose_core::shell::cmd` | `tests/command/test_shell_quoting.py` tokens |
| `repoq/solve_repa.json` | `repose_core::repoq` | `tests/template/test_resolver.py` |
| `template/sample.yml` | `repose_core::template` | products.yml schema |
| `transform/version.json` | `repose_core::transform` | `tests/types/test_transformations.py` |
| `hostparse/hosts.json` | `repose_core::host_parse` | `repose/host.py` |
| `ndjson/events.jsonl` | `repose_core::console` | `tests/test_console.py` shapes |
| `product/parse_prod.json` | `repose_core::product_parse` | `repose/target/parsers/product.py` `__parse_product` |
| `product/os_release.json` | `repose_core::product_parse` | `repose/target/parsers/product.py` `__parse_os_release` |
| `zypper_lr/parse.json` | `repose_core::repo_parse` | `repose/target/parsers/repository.py` |
| `remove_match/repolist.json` | `repose_core::commands::remove` | `repose/command/remove.py` `_calculate_repolist` |
| `sequences/reset.json` | `repose_core::commands::reset` tests | `tests/command/test_reset.py` + `repose/command/reset.py` |
| `sequences/install.json` | `repose_core::commands::install` tests | `tests/command/test_install.py` + `repose/command/install.py` |
| `sequences/uninstall.json` | `repose_core::commands::uninstall` tests | `tests/command/test_uninstall.py` + `repose/command/uninstall.py` |

L2 sequence vectors (`sequences/*.json`) are the per-scenario expected
remote-command sequences (`ran`), dry-run preview lines (`dry`), and aggregate
`exit` for the mutation commands. They are asserted by the `#[cfg(test)]`
modules in `repose-core::commands::{reset,install,uninstall}` via
`commands::seq`. Set-sourced alias/command order follows the Rust stable sort.

The CLI vector (`cli/known_products.txt`) is the committed `known-products`
output for `template/sample.yml`. `scripts/check-cli.sh` asserts the Rust
binary matches this expected output byte-for-byte and upholds the CLI-surface
invariants (no `--ssh-backend`, `repose version: X.Y.Z` shape, all nine
subcommands). Run in CI by the `rust-cli` job. Dry-run mutation and
connect/accept-new coverage needs a containerized sshd and is tracked
separately.

Normative rule: the committed vectors beat the design doc — they define the
binary's expected behavior and are updated deliberately, never regenerated
from another implementation.
