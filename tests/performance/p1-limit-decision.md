# P1 limit decision table

Evidence and selected defaults for `plans/p1-bound-resources-and-prevent-stalls.md`'s
decision gate (steps 1–5). **Every numeric default in the P1 implementation
phases must trace to exactly one row below.** This document requires
explicit review before any of Phases A–H are implemented.

## Environment note (read first)

Docker is unavailable in the sandbox this evidence was gathered in (no
`docker`, `podman`, `colima`, or `nerdctl`; P0's own committed baseline has
zero `ssh`-kind entries for the same reason — `tests/performance/baselines/local-dev.json`
contains only `mock`-kind workloads). Real per-session SSH/SFTP transport
timing (decision-gate step 2) was instead gathered by driving the *same*
`tests/ssh/Dockerfile` fixture through Apple's `container` CLI (a
macOS-native container runtime, available on this machine) via a new,
clearly-scoped local-dev-only script
(`scripts/measure-ssh-concurrency-container.sh`); the committed,
CI/contributor-facing `tests/ssh/run.sh` remains Docker-only and
**unmodified**. This is real, measured transport data — not fabricated —
but it is a single lightweight Alpine `sshd` in one resource-constrained
local VM, not a real multi-host network. Where this matters, it is called
out explicitly below, and step 37 (re-measurement) should repeat the SSH
measurements against Docker/a real fleet once available.

A second, pre-existing, unrelated finding surfaced while building this
harness: `scripts/run-performance-baseline-ssh.sh` passes a `-i
"$REPOSE_SSH_IDENTITY"` flag that the current `repose` CLI does not accept
(no identity flag exists; identity resolution goes through
`~/.ssh/config`'s `IdentityFile`, which `tests/ssh/run.sh` already sets
up). Since `ssh`-kind workloads have never actually run end-to-end in any
environment this project has been evaluated in so far (no Docker), this
was never caught. It is flagged here, not fixed — out of scope for P1.

A third, pre-existing, unrelated limitation: `crates/repose-ssh/tests/ssh_integration.rs`
(the full live-fixture integration suite, as opposed to the short-lived
CLI invocations `scripts/measure-ssh-concurrency.sh` makes) fails with
`Connection reset by peer` against the Apple `container` substitute
starting on its second or third sequential connection within one test
process — reproduced identically against the unmodified pre-P1 code
(confirmed via `git stash`), so it is an artifact of this local
Docker-substitute environment, not a P1 regression. It could not be
locally validated end-to-end here; it does run in any CI/contributor
environment with real Docker (`tests/ssh/run.sh`), which is the supported
path for this suite.

**Update (mid-P1, changeset 7):** the Apple `container` substitute
degraded further during this session — even a single fresh connection
(via the plain OpenSSH client, not just `repose`) now gets
`kex_exchange_identification: read: Connection reset by peer` immediately
after TCP connect, while `container exec`ing into the same container
confirms `sshd` is running and listening correctly inside it (`0 of
10-100 startups`). This isolates the failure to the port-forwarding/NAT
layer of this specific local runtime (version 1.1.0), not `sshd`, the
Dockerfile, or any `repose`/P1 code — restarting `container system`
did not resolve it. From this point on, P1's SSH/SFTP-facing changes
(bounded reads, deadlines, output limits, host-key persistence) are
verified by: full compilation, the complete deterministic mock/unit test
suite, direct API-level review against `russh`/`russh-sftp` source, and
new live-gated tests committed for `tests/ssh/run.sh` + real Docker to
exercise in any environment where that remains reliable.

## 1. Host-operation concurrency cap (SSH group phases + mutation workers)

**Selected default: 32** (`NonZeroUsize`).

| Candidate cap | Mechanism | Measured 100-host latency | Source |
|---:|---|---:|---|
| 1 | bounded unordered, synthetic 5ms/op | 722–738 ms (mean 728 ms) | `cargo bench -p repose-core --bench fleet -- fleet_concurrency_cap_sweep_100h/1` |
| 4 | same | 180.8–182.2 ms | `.../4` |
| 8 | same | 93.5–94.9 ms | `.../8` |
| 16 (today's per-host probe precedent) | same | 50.4–50.8 ms | `.../16` |
| **32 (selected)** | same | **28.8–29.1 ms** | `.../32` |
| 64 | same | 14.5–14.6 ms | `.../64` |
| 100 (unbounded-equivalent) | same | 7.26–7.33 ms | `.../100` |

Each row is a real Criterion measurement (10 samples, `bounded_fanout_cost`
in `crates/repose-core/benches/fleet.rs`): 100 synthetic operations of
fixed 5 ms cost admitted through `futures_util::stream::buffer_unordered(cap)`.
The 5 ms per-op cost is a stand-in for one remote round trip, chosen only to
keep the sweep fast; the curve's *shape* (`~⌈100/cap⌉ × op_cost`) is what
transfers to real per-host costs of any magnitude — a fleet with a 500 ms
real per-host cost would see roughly 7 s at cap 32 vs. 50 s at cap 1.

Real-transport ceiling check (not a latency measurement, a *safety*
measurement): `scripts/measure-ssh-concurrency-container.sh` drove up to
**50 fully independent, concurrent, freshly-authenticated SSH sessions**
against one fixture `sshd` with **zero failures** (p50 46.1 ms, p95 70.1
ms, p99 84.9 ms, max 84.9 ms; see
`tests/performance/baselines/raw/ssh-concurrency-local-dev.json`). This
shows 32 concurrent host operations is comfortably inside demonstrated safe
transport-resource territory (sockets, SSH handshakes, SFTP channels) with
room to spare, before any client-side memory/FD concerns even become
relevant.

**Rationale for 32 over 16 or 64:** 32 keeps ~1.7x of 64's latency
improvement over today's 16-per-host precedent while roughly halving
peak concurrent SSH sessions, memory (see stdout/stderr limits below), and
authentication load relative to 64. This is a policy default, not a
hard resource ceiling breach point — 64 remains safe per the transport
check above but was not selected as default to keep first-cut resource
pressure conservative; nothing in Phase B prevents revisiting this with a
real multi-host measurement in step 37.

**Applies to:** `RusshHostGroup`/`MockHostGroup` connect/read/parse/run/close
phases (step 12/11) and each mutation command's per-host worker fan-out
(steps 13–18) — one shared limit type per the plan's decision ("use one
validated, nonzero host-operation limit for both SSH group phases and
complete per-host mutation workers").

## 2. Global probe concurrency cap

**Selected default: 64** (`NonZeroUsize`).

Probes are a single outbound HTTP HEAD/GET per candidate URL — no PTY, no
SFTP channel, no persistent per-host state — so they can safely sustain
higher concurrency than full host operations for the same resource budget.
Today's per-*host* cap of 16 (`crates/repose-core/src/commands/mod.rs:294`,
inherited from the Python `asyncio.Semaphore(min(16, n))`) means a 100-host
fleet with several repository candidates per host can already reach
multiples of 16 in flight simultaneously with no fleet-wide ceiling at all.
64 is 4x today's per-host value yet remains an explicit, single, fleet-wide
number instead of an unbounded multiple of host count — the core ask of
decision item 19–23. No dedicated network-only load test was run (probes
go over `reqwest`/TLS to arbitrary external mirrors, not the local fixture);
64 is a conservative, reviewable starting point given the mock sweep above
already demonstrates the queueing shape at this order of magnitude.

## 3. Per-session SFTP read concurrency cap

**Selected default: 16.**

| Refhost fixture | `/etc/products.d` entries |
|---|---:|
| sl-micro-6-1, sles-16-0 | 2 |
| sle-hpc-15-sp5, sle-rt-15-sp4 | 8 |
| sled-15-sp6 | 9 |
| sles-12-sp5 | 11 |
| sles-teradata-15-sp4 | 12 |
| sles-15-sp6 | 17 |
| sles-sap-12-sp5 | 17 |
| **sles-15-sp5 (max observed)** | **18** |

(Counted directly from `tests/vectors/refhosts/*/products.d/`, the
project's own committed real-product reference fixtures covering SLES,
SLED, SAP, HPC, RT, Teradata, and SL Micro variants.)

16 lets every fixture above complete its addon-`.prod` + transactional-conf
SFTP fan-out in a single batch (at most one extra addon over 16, requiring
a second small batch only for the largest fixture) while still bounding
worst-case fan-out if a host's directory is unexpectedly large. Reuses the
existing 16-value precedent already in the codebase (probe fan-out) rather
than introducing a new distinct magic number, per the plan's preference for
minimal new configuration surface.

## 4. Product-directory listing cap (`/etc/products.d` entries)

**Selected default: 256.**

Largest real observed entry count is 18 (above). 256 is a ~14x margin over
the largest known real configuration (including SAP/HPC/Teradata variants),
while remaining far below a count that could plausibly represent a
misconfigured or corrupted filesystem returning implausible directory
listings — the failure mode step 30 defends against
(`DirectoryTooLarge`).

## 5. SFTP file byte cap (`.prod` / transactional-conf / `os-release` reads)

**Selected default: 65536 bytes (64 KiB) per file.**

| Fixture file | Bytes |
|---|---:|
| largest `.prod` (`sl-micro-6-1/SL-Micro-Extras.prod`) | 4426 |
| all other `.prod` files across all 10 refhosts | ≤ 4085 |

64 KiB is ~15x the largest real `.prod` file this project has on file.
Worst case memory impact at the approved per-session SFTP cap: 16 concurrent
reads × 64 KiB = 1 MiB per host; at the approved host-operation cap of 32
concurrent hosts, 32 MiB fleet-wide ceiling — negligible on any machine
capable of running `repose` today.

## 6. Command stdout/stderr byte caps

**Selected default: 262144 bytes (256 KiB) per stream, independently.**

| Real remote command output sample | Bytes |
|---|---:|
| largest `zypper -x lr` XML across all 10 refhosts (`sles-15-sp5`) | 11607 |
| largest `list-repos.json`/`.text` rendering | 9774 |

256 KiB is >20x the largest real single-command output this project has on
file. Each stream (stdout/stderr) gets its own independent cap — not a
combined budget — because zypper is documented elsewhere in this codebase
(`commands/mod.rs::report_target`) to write diagnostics to *either* stream
depending on exit code. Worst-case fleet memory at the approved
host-operation cap: 32 concurrent hosts × (256 KiB + 256 KiB) = 16 MiB.

## 7. SSH phase deadlines

**Selected defaults:**

| Phase | Deadline | Command-completion deadline unchanged |
|---|---:|---|
| Connection/handshake (DNS+TCP+proxy+KEX) | 30 s | — |
| Network authentication (agent + pubkey attempts; excludes user-paced password entry) | 30 s | — |
| Channel open | 15 s | — |
| Exec dispatch / SFTP subsystem initialization | 15 s | — |
| SFTP operation (one read/listdir/readlink call) | 30 s | — |
| Bounded cleanup/drain after an output/SFTP overflow | 5 s | — |
| Command completion | — | unchanged: existing configurable `ConnectionConfig.timeout` (default 120 s) |

**Evidence:** real, measured, uncontended single-session latency for the
*entire* connect+auth+SFTP-product-discovery+one-command chain
(`list-products` against the fixture) is **10.8–11.6 ms**; under
concurrent load it degrades sub-linearly with zero failures:

| Concurrent sessions | p50 | p95 | p99 | max | failures |
|---:|---:|---:|---:|---:|---:|
| 1 | 10.79 ms | 10.79 ms | 10.79 ms | 10.79 ms | 0 |
| 5 | 10.85 ms | 10.95 ms | 10.95 ms | 10.95 ms | 0 |
| 10 | 14.66 ms | 17.33 ms | 17.33 ms | 17.33 ms | 0 |
| 20 | 26.45 ms | 32.29 ms | 33.47 ms | 33.47 ms | 0 |
| 50 | 46.07 ms | 70.15 ms | 84.89 ms | 84.89 ms | 0 |

(`tests/performance/baselines/raw/ssh-concurrency-local-dev.json`, gathered
via `scripts/measure-ssh-concurrency-container.sh` — see the environment
note above for why this is a local single-container measurement, not a
real fleet/WAN one.)

This means every phase these deadlines separately bound — combined —
completes in under 100 ms even under 50-way contention on a
resource-constrained local fixture. No real WAN/bastion/ProxyCommand
measurement was available. Per widely-documented OpenSSH operational
experience, real connect+auth latency over real networks, VPNs, or
multi-hop `ProxyCommand`s commonly falls in the 100 ms–5 s range, and
occasionally higher on saturated or high-latency links. The proposed 15
s/30 s budgets therefore carry roughly 300×–1000× headroom over the
measured local baseline — deliberately generous specifically to absorb
that gap, while remaining short enough that a genuinely stalled phase
(network black hole, hung server, wedged subsystem) is detected and
released within one to two minutes rather than hanging indefinitely (today
these phases have **no** deadline at all and can hang forever). This is
also consistent with the existing reconnect schedule's own use of a 10 s
base timeout (`reconnect_sleep_secs`, `wait_reconnect(10, 10, true)` in
`crates/repose-ssh/src/host.rs`).

**Known limitation, carried into step 37:** these five deadlines are
evidence-based but not validated against real per-phase (as opposed to
whole-operation-aggregate) percentiles, nor against a real multi-host
WAN fleet. They should be revisited with real Docker/fleet measurements
in step 37 once available; until then they are a strict improvement over
today's *no* deadline on these phases.

## Summary table (for Phase A implementation)

| Config field | Default |
|---|---:|
| `host_operation_limit` | `NonZeroUsize::new(32)` |
| `probe_concurrency_limit` | `NonZeroUsize::new(64)` |
| `sftp_read_concurrency_limit` (per session) | `NonZeroUsize::new(16)` |
| `max_products_d_entries` | `256` |
| `max_sftp_file_bytes` | `65536` |
| `max_stdout_bytes` / `max_stderr_bytes` | `262144` each |
| `connect_deadline` | `30s` |
| `auth_deadline` | `30s` |
| `channel_open_deadline` | `15s` |
| `dispatch_deadline` | `15s` |
| `sftp_operation_deadline` | `30s` |
| `overflow_cleanup_deadline` | `5s` |
| `command_deadline` | unchanged (`ConnectionConfig.timeout`, default 120s) |

## Reproducing this evidence

```sh
# Mock concurrency-cap sweep + gated fleet-width proof (no docker needed):
cargo bench -p repose-core --bench fleet --locked -- fleet_concurrency_cap_sweep
cargo bench -p repose-core --bench fleet --locked -- fleet_gated_admission

# Real SSH multi-session measurement (needs Docker OR Apple's `container` CLI):
cargo build --release --locked -p repose-cli
REPOSE_BIN="$PWD/target/release/repose" \
  ./scripts/measure-ssh-concurrency-container.sh \
  bash ./scripts/measure-ssh-concurrency.sh /tmp/ssh-perf-out
# or, with Docker available:
REPOSE_BIN="$PWD/target/release/repose" \
  tests/ssh/run.sh bash scripts/measure-ssh-concurrency.sh /tmp/ssh-perf-out
```

## Sign-off

- [ ] Reviewed and approved: the numeric defaults above may be implemented
      in Phases A–H of `plans/p1-bound-resources-and-prevent-stalls.md`.
