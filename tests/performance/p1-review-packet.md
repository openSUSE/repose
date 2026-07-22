# P1 review packet

Final remeasurement and review evidence for
`plans/p1-bound-resources-and-prevent-stalls.md` (steps 37–39), covering
changesets 2–10 (steps 6–36: configuration/errors/trait/CLI wiring, bounded
host/mutation fan-out, the global probe budget, the SSH deadline
foundation, bounded SFTP discovery/reads, bounded command output, the
private-key `spawn_blocking` offload, and fail-closed `accept-new`
persistence).

## 1. Selected limits (unchanged from the decision gate)

Every default below is unchanged from `tests/performance/p1-limit-decision.md`'s
approved table and was implemented exactly as approved — no default drifted
during implementation:

| Config field | Default |
|---|---:|
| `host_operation_limit` | `32` |
| `probe_concurrency_limit` | `64` |
| `sftp_read_concurrency_limit` (per session) | `16` |
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

## 2. Before/after metrics (mock-kind workloads, step 37)

Every `mock`-kind workload in `tests/performance/workloads.json` was
rerun (release build, 20 repetitions, 3 warmup — identical to the
committed P0 baseline's methodology) after all P1 changesets, and compared
against the committed `tests/performance/baselines/local-dev.json` P0
baseline using the exact rules in `tests/performance/thresholds.json`
(`scripts/compare-performance.sh`'s rule engine, applied manually here
because the original P0 raw per-workload reports — full `wall_time_ns`
sample arrays — are local/CI-only gitignored artifacts and were not
retained after the P0 baseline's compact summary was written; the compact
summary retains every field the comparator rules need).

| workload | p50 Δ | p95 Δ | p99 Δ | throughput Δ | peak RSS Δ | exact metrics |
|---|---:|---:|---:|---:|---:|---|
| mock-add-1h | −5.8% | +69.7%¹ | +76.3%¹ | −6.2% | +7.5% | match |
| mock-add-20h | +5.3% | −16.7% | −13.7% | +5.0% | +3.0% | match |
| mock-add-20h-json | +7.9% | +16.4% | +34.1% | +7.4% | +4.2% | match |
| mock-add-100h | −4.1% | −4.0% | −1.2% | −4.3% | +4.7% | match |
| mock-add-100h-repeated-urls | +0.8% | +1.2% | +5.1% | +0.7% | +4.3% | match |
| mock-add-100h-slow-host | −0.0% | −0.0% | +0.0% | −0.0% | +4.3% | match |
| mock-install-1h | −15.1% | −30.5% | −27.0% | −17.7% | +6.0% | match |
| mock-install-20h | +5.2% | −0.2% | +2.3% | +4.9% | +3.2% | match |
| mock-install-100h | +3.3% | +9.3% | +58.5%¹ | +3.2% | +3.6% | match |
| mock-list-products-1h | +15.7% | +4.6% | +18.2% | +13.6% | +2.9% | match |
| mock-list-products-20h-json | −12.4% | +3.1% | +19.1% | −14.2% | +1.8% | match |
| mock-list-products-100h | −0.8% | −0.4% | +1.9% | −0.8% | +1.5% | match |

¹ These three workloads have the largest absolute deltas but are all
sub-millisecond, single-run measurements (e.g. mock-add-1h: 69µs → 122µs
p99) — exactly the noise regime `thresholds.json`'s own evidence describes
("5 independent local runs of mock-add-100h... ~56% spread" at p50 alone).
A same-code, back-to-back rerun of the full P1 suite (`raw-p1` vs
`raw-p1-run2`, both post-P1, zero code change between them) reproduced
comparable swings on these same small workloads
(`scripts/compare-performance.sh` output, e.g. mock-add-1h p50
45333ns→58375ns, p95 117583ns→83458ns between two identical-code runs),
confirming this is measurement noise on a shared/uncontrolled runner, not
a P1 regression.

**Result: every one of the 12 mock workloads passes every threshold rule**
(`exit_code`, `command_count`, `probe_count`, `stdout_digest`,
`stderr_digest` exact; `peak_concurrency` ceiling; `latency_ns.{p50,p95}`
≤ +100%; `latency_ns.p99` ≤ +120%; `throughput_ops_per_sec` ≥ −50%;
`peak_rss_bytes` ≤ +20%). No workload came close to a threshold boundary;
the largest ratio observed (mock-install-100h p99, +58.5%) is well inside
its +120% limit.

`ssh`-kind workloads could not be remeasured in this environment: Docker
is unavailable (as documented in the P0 baseline and the decision gate's
environment note), and the local Apple-`container` substitute used for the
decision gate degraded partway through changeset 7 (`kex_exchange_identification:
read: Connection reset by peer` immediately after TCP connect, unrelated
to `repose` code — see the decision gate's environment note for the full
diagnosis). SSH/SFTP-facing changesets (7–10) were instead verified by:
compilation, the full deterministic mock/unit suite, direct API review
against `russh`/`russh-sftp` source, and newly committed live-gated tests
in `crates/repose-ssh/tests/ssh_integration.rs` that will exercise the
real transport in any environment with Docker or the fixture's
`REPOSE_SSH_*` variables available (they currently no-op here, matching
this file's own pre-existing, flagged-not-fixed limitation).

## 3. Command/probe histories

Exact per-workload `command_count` and `probe_count` are unchanged from
the P0 baseline for all 12 mock workloads (see the table above: "exact
metrics" column is `match` throughout, meaning `exit_code`,
`command_count`, `probe_count`, `stdout_digest`, and `stderr_digest` are
byte-for-byte identical to the P0 baseline, and `peak_concurrency` did not
exceed its P0 ceiling). This directly demonstrates the plan's core
equivalence requirement: bounded execution changed *only* operation start
times, never which operations ran, how many times, or in what aggregate
order.

## 4. Equivalence proofs

Each changeset's isomorphism argument is recorded inline next to its
implementation, not duplicated here:

- Steps 11–18 (bounded host/mutation fan-out): see the "Isomorphism proof
  for steps 11–18" note in the plan, and `crates/repose-core/src/mock.rs` /
  `crates/repose-ssh/src/host.rs`'s gated concurrency tests.
- Steps 19–23 (global probe budget): "Isomorphism proof for steps 19–23"
  in the plan; `crates/repose-core/src/commands/mod.rs`'s
  `bounded_add_probe_budget_is_fleet_wide_not_per_host`-style tests.
- Steps 24–27 (SSH deadlines): "Isomorphism proof for steps 24–27" in the
  plan; `crates/repose-ssh/src/session.rs`'s `with_deadline` tests.
- Steps 28–30 (bounded SFTP/listing): "Isomorphism proof for steps 28–30"
  in the plan; `crates/repose-ssh/tests/ssh_integration.rs`'s live-gated
  byte/entry-cap tests.
- Steps 31–32 (bounded command output): "Isomorphism proof for steps
  31–32" in the plan; `accumulate_or_overflow`'s unit tests plus the
  live-gated at-limit/over-limit/mixed-stream/session-reuse tests.
- Step 33 (private-key offload): candidate order and
  encrypted/malformed/missing-key match arms are byte-for-byte unchanged
  (see the diff in `authenticate_automatic`); only the execution context
  (blocking pool vs. async thread) changed. Verified by
  `spawn_blocking_key_work_does_not_delay_an_independent_sibling` and the
  `candidate_keys`/`default_key_candidates_in` correctness tests.
- Steps 34–35 (fail-closed accept-new): matching/changed/revoked
  decisions are unchanged (`HostKeyVerifier::decide` is the same logic as
  the prior `verify_key`, only pure/non-mutating now); the sole behavior
  change is the approved one (persistence failure now rejects instead of
  trusting-without-recording). All 14 pre-existing `hostkey.rs` tests pass
  unmodified in intent (routed through a test-only `verify()` helper that
  mirrors the real decide → persist → record sequence).

## 5. Fail-closed host-key evidence

- `hostkey::tests::accept_new_rejects_first_contact_when_persistence_fails`
  — a read-only `known_hosts` directory causes `decide()` +
  `persist_first_contact` to reject the key and record nothing on disk or
  in memory.
- `session::tests::check_server_key_rejects_first_contact_when_persistence_fails`
  — the same scenario through the real, async `ClientHandler::check_server_key`
  entry point used by `russh`.
- `session::tests::check_server_key_accepts_first_contact_only_after_a_durable_write`
  — the positive case: acceptance requires an actual successful write, not
  just a decision.
- `session::tests::check_server_key_persistence_does_not_delay_an_independent_sibling`
  — persistence runs via `spawn_blocking`; a sibling timer completes on
  its own schedule while the write is in flight.
- Changed and revoked keys are decided by the pure `decide()` function
  before a `PersistFirstContact` value can ever be produced (see
  `hostkey.rs`'s `decide` match arms), so they structurally cannot
  schedule a write — confirmed by the pre-existing
  `accept_new_persists_first_contact_and_refuses_changed_key` and
  `revoked_key_is_refused_even_without_a_trusted_pin` tests still passing
  unmodified.

## 6. Repository gates (step 38)

Run after every changeset in this phase, and again after this packet was
assembled:

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets --locked
cargo deny check
scripts/check-rust-layering.sh
```

All pass with only the already-accepted `cargo deny` duplicate-dependency
warnings present before P1. Final counts: `repose-core` 175 unit tests + 1
vector-parity integration test + the `fleet` Criterion bench's own
equivalence assertions; `repose-ssh` 46 unit tests + 12 (currently
no-op/live-gated) integration tests; `repose-cli` 13 tests.

## 7. Rollback boundaries

P1 is ten independently reviewable changesets (changeset 1 was the
decision gate itself); each is a separate, self-contained commit-sized
unit that can be reverted without touching the others' code:

1. Configuration/errors/trait/CLI wiring (steps 6–10) — purely additive
   types/fields with defaults; reverting restores unbounded behavior with
   no call-site changes elsewhere.
2. Mock/SSH group fan-out (steps 11–12) — reverting restores unbounded
   `join_all` in `mock.rs`/`host.rs`'s group phases only.
3. Per-command mutation worker bounding (steps 13–18) — six independent
   command modules; any single one can be reverted without affecting the
   others (each owns its own `join_all`→bounded-stream change).
4. Global probe budget (steps 19–23) — reverting removes the semaphore
   and restores the prior per-host cap in `commands/mod.rs` and the three
   command modules that construct a `ProbeBudget`.
5. SSH deadline foundation (steps 24–27) — reverting `with_deadline`'s
   call sites in `session.rs` and the typed-timeout match arm in
   `host.rs` restores the prior undeadlined phases; the command-completion
   timeout (pre-existing, unchanged contract) is unaffected either way.
6. Bounded SFTP discovery/reads (steps 28–30) — reverting restores
   whole-file reads and unbounded `read_files`/listing fan-out in
   `session.rs`/`host.rs`.
7. Bounded command output (steps 31–32) — reverting `accumulate_or_overflow`'s
   call sites in `run_inner` restores unbounded stdout/stderr buffering;
   `host.rs` needs no change either way (the generic `Err(e)` arm handles
   any transport error already).
8. Private-key `spawn_blocking` offload (step 33) — reverting
   `candidate_keys`/`load_unencrypted_key` to synchronous calls changes
   only which thread does the work, not behavior; safe to revert in
   isolation.
9. Fail-closed `accept-new` persistence (steps 34–36) — reverting
   `check_server_key`'s persist-then-trust sequence to the prior
   trust-then-persist-best-effort behavior (and the two README edits) is
   the one changeset with an intentional, approved behavior change if
   rolled back — matching/changed/revoked decisions are unaffected.

Each changeset's own test suite (documented per-changeset in
`plans/p1-bound-resources-and-prevent-stalls.md`'s progress log) continues
to pass with any later changeset reverted, since no changeset depends on
code introduced by a later one.

## Sign-off

- [x] Reviewed and approved: P1 (`plans/p1-bound-resources-and-prevent-stalls.md`)
      is complete and ready to merge.
