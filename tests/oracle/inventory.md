# Oracle inventory

Maps Python pytest specification sources to committed goldens consumed by Rust tests.
Regenerate with: `python3 scripts/gen-oracle.py` (when present) or the generator used for this tree.

| Golden | Python source | Rust consumer |
| --- | --- | --- |
| `repa/parse.json` | `tests/types/test_repa.py` | `repose_core::repa` |
| `shell/quote.json` | `shlex.quote` / `tests/command/test_shell_quoting.py` | `repose_core::shell` |
| `shell/join.json` | `shlex.join` | `repose_core::shell` |
| `shell/command_templates.json` | `tests/command/test_shell_quoting.py` tokens | `repose_core::shell::cmd` |
| `repoq/solve_repa.json` | `tests/template/test_resolver.py` | `repose_core::repoq` |
| `template/sample.yml` | products.yml schema | `repose_core::template` |
| `transform/version.json` | `tests/types/test_transformations.py` | `repose_core::transform` |
| `hostparse/hosts.json` | `repose/host.py` | `repose_core::host_parse` |
| `ndjson/events.jsonl` | `tests/test_console.py` shapes | `repose_core::console` |

Normative rule: goldens beat design doc; Python async path is oracle until cutover.
