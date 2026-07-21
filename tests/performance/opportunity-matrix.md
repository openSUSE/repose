# P0.4 opportunity matrix

Ranked local-CPU/allocation opportunities from release profiles of the
`mock`-kind fleet workloads. Every entry traces to a captured manifest
under `tests/performance/profiles/`; no percentage here is inferred from
source reading alone (see `tests/performance/README.md` for the contract
and cross-platform prerequisites).

## Evidence captured

| Manifest | Workload | Platform | Classes |
|---|---|---|---|
| `mock-add-100h.darwin.json` | `add`, 100 hosts, 200k reps | macOS arm64, `sample`+`heap` | cpu, alloc |
| `mock-list-products-100h.darwin.json` | `list-products`, 100 hosts, 200k reps | macOS arm64, `sample`+`heap` | cpu, alloc |

**I/O class**: attempted (`fs_usage`) and recorded as `"status": "skipped"` —
this sandbox has no passwordless `sudo` and `fs_usage` requires root. No I/O
opportunity is claimed below; rerun `scripts/profile-performance.sh <id>
--classes io` with root to fill this in.

**Linux**: not captured in this session (no Linux host available); the
`perf`/`heaptrack`/`strace` adapters in `scripts/profile-performance.sh` are
implemented per each tool's documented CLI but unverified end-to-end. Treat
Linux-specific claims as pending until a Linux capture lands.

**Scope caveat**: these are `mock`-kind (in-process, no SSH/SFTP/zypper)
profiles — they measure `repose-core`'s *local* command-algorithm cost, not
remote/network cost. This is consistent with, not a contradiction of, the
audit's conclusion that remote round trips dominate production wall time
(`plans/performance-action-plan.md`); local hotspots matter most for P5
("profile-gated local optimizations"), which explicitly gates on evidence
like this.

**Excluded as harness artifact**: `mock-list-products-100h`'s CPU profile's
#2 symbol is `baseline_report::check_expectation` (185/2215 ≈ 8.4% of
top-of-stack samples) — the harness's own per-repetition equivalence check
(hashing the full 100-host stdout every rep). This is
`crates/repose-core/examples/baseline_report.rs` overhead, not `repose`
production behavior, and is excluded from ranking below.

**Not independently attributable**: generic allocator/copy symbols
(`_xzm_malloc_tc`, `_xzm_free_tc`, `_platform_memmove`, `_malloc_zone_malloc`,
`_free`, `_platform_memcmp`) together account for ~41% of `mock-add-100h`'s
top-of-stack samples. `sample`'s flat top-of-stack view cannot attribute
*which* call site allocated without a full call-tree capture (a known
macOS-tooling limit — `tests/performance/README.md`), so this is reported as
aggregate evidence that the workload is allocation/copy-bound in general
(motivating P5's existing clone-reduction items), not as a single ranked
candidate below.

## Ranked candidates

| Rank | Candidate | Evidence | Impact | Confidence | Effort | Score | Equivalence constraint |
|---:|---|---|---|---|---|---|---|
| 1 | `repose_core::shell::quote` allocates a new `String` even on the all-safe-characters fast path | `mock-add-100h.darwin.json` cpu: `repose_core::shell::quote` is the top *repose*-owned symbol at 109/2711 ≈ 4.0% of top-of-stack samples, called once per shell token in every generated `zypper`/`transactional-update` command across 100 hosts | Medium (called on every command token, every host) | High (function is on the hot path in every mutation command) | Small (return `Cow<str>` or write into a caller-supplied buffer; `is_safe_char` scan already computed) | (0.6×0.9)/0.2 ≈ 2.7 | Output bytes for every existing `tests/vectors/shell/` vector must stay byte-identical; only the allocation strategy changes |
| 2 | `Repoq::substitute` / `Repoq::solve_repa` template-variable substitution cost | `mock-add-100h` cpu: `repoq::substitute` and `repoq::solve_repa` symbols present in the sampled call stacks (per-REPA, per-host); scales with `host_count × repa_count` | Medium at fleet scale | Medium (present but smaller than rank 1 in this capture; needs a repeated/controlled capture to rank precisely) | Small–Medium | (0.5×0.6)/0.3 ≈ 1.0 | Resolved repository name/URL/refresh values must stay byte-identical for every `tests/vectors/repoq/` case |
| 3 | `products.yml` (YAML) template is re-parsed from scratch inside `load_repoq` on every `run_add`/`run_install` call | `mock-add-100h` cpu: `unsafe_libyaml::scanner::yaml_parser_fetch_plain_scalar` / `yaml_parser_update_buffer` present in top-of-stack samples; `mock-install-1h`/`mock-add-1h` baselines show ~36–50µs p50 for a single host, where template parsing is a non-negligible fraction | Low in production (one parse per CLI invocation regardless of host count, so cost is amortized across the fleet) — **not worth optimizing at current scale**; recorded for completeness only | Medium | N/A | Below threshold | N/A — do not implement without a production workload showing per-invocation parse cost actually matters (e.g., many small, single-host invocations) |
| 4 | `core::fmt::write` / `alloc::str::join_generic_copy` string formatting and joining | `mock-add-100h` cpu: both present in top-of-stack samples (17 and 15 of 2711 respectively ≈ 0.6% each) — likely `shell::join`/`cmd::*` command-string assembly | Low (each individually under 1% of samples) | Medium | Small | (0.3×0.6)/0.2 ≈ 0.9 | Generated command strings must stay byte-identical |
| 5 | Aggregate allocation/copy pressure (~41% of `mock-add-100h` top-of-stack samples across `malloc`/`free`/`memmove`/`memcmp`) | `mock-add-100h.darwin.json` cpu (see "not independently attributable" above) | Unknown until attributed | Low until a call-tree-attributed capture exists | N/A | Profile-gated | Capture a call-tree (not flat top-of-stack) profile — e.g. `perf record --call-graph` on Linux, or Instruments Allocations on macOS — before deriving a specific P5 candidate from this line |

## Next steps

1. Capture a Linux `perf`/`heaptrack` run to validate the darwin-only
   evidence above and fill in the I/O class.
2. Capture a call-tree (not flat) CPU profile to attribute rank 5's
   aggregate allocation cost to specific call sites before proposing a P5
   change for it.
3. Re-profile after any P1–P3 change that touches `add`/`install`'s hot
   path (bounded fan-out, batched refreshes) — ranks 1–2 may shift once
   remote-round-trip reductions land, since they change how many times
   `shell::quote`/`Repoq::solve_repa` run per invocation.
