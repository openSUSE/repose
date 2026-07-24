//! In-memory [`Host`] / [`HostGroup`] for L2 command tests (no SSH).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::Poll;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::future::join_all;

use crate::error::SshError;
use crate::traits::{Host, HostGroup, Probe};
use crate::types::{OutEntry, Repositories, Repository, System};

/// Scripted remote result for [`MockHost::run`].
#[derive(Debug, Clone)]
pub enum MockRunOutcome {
    /// Completed remote command (any exit code, including non-zero).
    Complete {
        stdout: String,
        stderr: String,
        exitcode: i32,
        runtime_secs: u64,
    },
    /// Hard timeout: appends `out` with `exitcode == -1` and **empty**
    /// streams (the real host logs the diagnostics instead), returns `Ok(())`.
    /// The `stderr` field is accepted for convenience but ignored.
    Timeout { stderr: String },
    /// Mid-command transport failure: appends a synthetic `out` entry with
    /// `exitcode == -1` and empty streams, returns `Ok(())` — matching the
    /// real host, where the message goes to the log (the field is ignored).
    TransportErr(String),
}

impl MockRunOutcome {
    #[must_use]
    pub fn ok_stdout(stdout: impl Into<String>) -> Self {
        Self::Complete {
            stdout: stdout.into(),
            stderr: String::new(),
            exitcode: 0,
            runtime_secs: 0,
        }
    }

    #[must_use]
    pub const fn exit(code: i32) -> Self {
        Self::Complete {
            stdout: String::new(),
            stderr: String::new(),
            exitcode: code,
            runtime_secs: 0,
        }
    }
}

/// Cooperative rendezvous for concurrency tests: [`RunBarrier::enter`]
/// completes only once `total` participants have entered.
///
/// Under truly concurrent fan-out every participant reaches the barrier
/// while the others are still inside it, so it releases immediately. Under
/// serial per-host execution the first participant can never be joined by
/// the second; it gives up after a bounded number of polls and sets
/// [`RunBarrier::timed_out`], which tests assert against (no hang, no
/// wall-clock timing).
#[derive(Debug)]
pub struct RunBarrier {
    total: usize,
    entered: AtomicUsize,
    timed_out: AtomicBool,
}

impl RunBarrier {
    #[must_use]
    pub fn new(total: usize) -> Arc<Self> {
        Arc::new(Self {
            total,
            entered: AtomicUsize::new(0),
            timed_out: AtomicBool::new(false),
        })
    }

    /// Enter the barrier and cooperatively wait for the other participants.
    pub async fn enter(&self) {
        const MAX_SPINS: usize = 100_000;
        self.entered.fetch_add(1, Ordering::SeqCst);
        let mut spins = 0usize;
        std::future::poll_fn(|cx| {
            if self.entered.load(Ordering::SeqCst) >= self.total {
                return Poll::Ready(());
            }
            spins += 1;
            if spins > MAX_SPINS {
                // Serial execution: nobody else can arrive while we wait.
                self.timed_out.store(true, Ordering::SeqCst);
                return Poll::Ready(());
            }
            cx.waker().wake_by_ref();
            Poll::Pending
        })
        .await;
    }

    /// Whether any participant gave up waiting (i.e. execution was serial).
    #[must_use]
    pub fn timed_out(&self) -> bool {
        self.timed_out.load(Ordering::SeqCst)
    }
}

/// Host-operation category for opt-in gating/metrics on mock test doubles.
///
/// Every [`Host`] method boundary maps to exactly one kind; `reboot`'s
/// nested `run` call is counted separately under [`Self::Run`] (see
/// [`MockMetricsSnapshot::operations_by_kind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MockOpKind {
    Connect,
    ReadProducts,
    ReadRepos,
    ParseRepos,
    Run,
    Reboot,
    Close,
}

/// Manually released gate an instrumented mock operation waits on.
///
/// Lets a test observe an operation as in-flight (via [`MockMetrics`])
/// before choosing to let it complete — a deterministic, non-sleeping
/// substitute for modelling remote latency.
#[derive(Debug, Default)]
pub struct MockGate {
    released: AtomicBool,
}

impl MockGate {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn release(&self) {
        self.released.store(true, Ordering::SeqCst);
    }

    async fn wait(&self) {
        const MAX_SPINS: usize = 1_000_000;
        let mut spins = 0usize;
        std::future::poll_fn(|cx| {
            if self.released.load(Ordering::SeqCst) {
                return Poll::Ready(());
            }
            spins += 1;
            assert!(
                spins <= MAX_SPINS,
                "MockGate never released (deadlock guard)"
            );
            cx.waker().wake_by_ref();
            Poll::Pending
        })
        .await;
    }
}

#[derive(Debug, Default)]
struct MockCounters {
    total_operations: AtomicUsize,
    current_operations: AtomicUsize,
    peak_operations: AtomicUsize,
    operations_by_kind: Mutex<BTreeMap<MockOpKind, usize>>,
    commands_attempted: AtomicUsize,
    commands_completed: AtomicUsize,
    probe_total: AtomicUsize,
    current_probes: AtomicUsize,
    peak_probes: AtomicUsize,
    probe_counts: Mutex<BTreeMap<String, usize>>,
}

/// Shared, `Arc`-backed counters for [`MockHost`] / probe test doubles.
///
/// Cloning shares the same counters; instrumentation is opt-in so existing
/// mocks that never attach a tracker are unaffected. Tests read an immutable
/// [`MockMetricsSnapshot`] rather than mutating counters directly.
#[derive(Debug, Clone, Default)]
pub struct MockMetrics(Arc<MockCounters>);

impl MockMetrics {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn enter_op(&self, kind: MockOpKind) -> OpGuard {
        self.0.total_operations.fetch_add(1, Ordering::SeqCst);
        let current = self.0.current_operations.fetch_add(1, Ordering::SeqCst) + 1;
        self.0.peak_operations.fetch_max(current, Ordering::SeqCst);
        *self
            .0
            .operations_by_kind
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(kind)
            .or_insert(0) += 1;
        OpGuard {
            metrics: self.clone(),
        }
    }

    fn enter_probe(&self, url: &str) -> ProbeGuard {
        self.0.probe_total.fetch_add(1, Ordering::SeqCst);
        let current = self.0.current_probes.fetch_add(1, Ordering::SeqCst) + 1;
        self.0.peak_probes.fetch_max(current, Ordering::SeqCst);
        *self
            .0
            .probe_counts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(url.to_string())
            .or_insert(0) += 1;
        ProbeGuard {
            metrics: self.clone(),
        }
    }

    fn record_command_attempted(&self) {
        self.0.commands_attempted.fetch_add(1, Ordering::SeqCst);
    }

    fn record_command_completed(&self) {
        self.0.commands_completed.fetch_add(1, Ordering::SeqCst);
    }

    /// Immutable point-in-time read of every counter.
    #[must_use]
    pub fn snapshot(&self) -> MockMetricsSnapshot {
        MockMetricsSnapshot {
            total_operations: self.0.total_operations.load(Ordering::SeqCst),
            current_operations: self.0.current_operations.load(Ordering::SeqCst),
            peak_operations: self.0.peak_operations.load(Ordering::SeqCst),
            operations_by_kind: self
                .0
                .operations_by_kind
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
            commands_attempted: self.0.commands_attempted.load(Ordering::SeqCst),
            commands_completed: self.0.commands_completed.load(Ordering::SeqCst),
            probe_total: self.0.probe_total.load(Ordering::SeqCst),
            current_probes: self.0.current_probes.load(Ordering::SeqCst),
            peak_probes: self.0.peak_probes.load(Ordering::SeqCst),
            probe_counts: self
                .0
                .probe_counts
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
        }
    }
}

/// Immutable snapshot of [`MockMetrics`] at the time it was taken.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MockMetricsSnapshot {
    pub total_operations: usize,
    pub current_operations: usize,
    pub peak_operations: usize,
    pub operations_by_kind: BTreeMap<MockOpKind, usize>,
    /// `Host::run` calls that passed the connected-state check.
    pub commands_attempted: usize,
    /// Commands that appended an `out` entry after passing the
    /// connected-state check (including the timeout / transport-failure
    /// synthetic entries).
    pub commands_completed: usize,
    pub probe_total: usize,
    pub current_probes: usize,
    pub peak_probes: usize,
    pub probe_counts: BTreeMap<String, usize>,
}

/// RAII guard: decrements `current_operations` on drop (including early
/// return or panic unwind), so a failure path never leaks an in-flight count.
struct OpGuard {
    metrics: MockMetrics,
}

impl Drop for OpGuard {
    fn drop(&mut self) {
        self.metrics
            .0
            .current_operations
            .fetch_sub(1, Ordering::SeqCst);
    }
}

/// RAII guard: decrements `current_probes` on drop.
struct ProbeGuard {
    metrics: MockMetrics,
}

impl Drop for ProbeGuard {
    fn drop(&mut self) {
        self.metrics.0.current_probes.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Controllable host for command unit tests.
#[derive(Debug, Default)]
pub struct MockHost {
    key: String,
    connected: bool,
    products: Option<System>,
    raw_repos: Option<Vec<Repository>>,
    repos: Option<Repositories>,
    /// System swapped into `products` by `reboot` (models the post-reboot
    /// re-read used by transactional install/uninstall verify).
    post_reboot_products: Option<System>,
    /// `reboot` clears `products` to `None` (models a post-reboot re-read
    /// that returned nothing parseable).
    post_reboot_clear_products: bool,
    /// `read_products` returns `Err` (models a re-read failure).
    read_products_err: bool,
    /// `read_repos` returns `Err` (models a failed `zypper -x lr`, leaving
    /// `raw_repos` unset — distinct from a successful read with zero repos).
    read_repos_err: bool,
    out: Vec<OutEntry>,
    /// FIFO outcomes for successive `run` calls. Empty → default exit 0.
    run_queue: Vec<MockRunOutcome>,
    /// Commands observed by `run` (in order).
    pub ran: Vec<String>,
    connect_fail: bool,
    /// When set, `run` enters this barrier before executing (concurrency
    /// proof in fan-out tests).
    run_barrier: Option<Arc<RunBarrier>>,
    /// Opt-in shared counters; `None` preserves plain default behavior.
    metrics: Option<MockMetrics>,
    /// Per-operation-kind gates a call waits on before proceeding.
    gates: BTreeMap<MockOpKind, Arc<MockGate>>,
}

impl MockHost {
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            connected: false,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_products(mut self, system: System) -> Self {
        self.products = Some(system);
        self
    }

    #[must_use]
    pub fn with_raw_repos(mut self, repos: Vec<Repository>) -> Self {
        self.raw_repos = Some(repos);
        self
    }

    #[must_use]
    pub fn with_repos(mut self, repos: Repositories) -> Self {
        self.repos = Some(repos);
        self
    }

    /// System that `reboot` swaps into `products`, modelling the post-reboot
    /// product re-read that transactional install/uninstall verify against.
    #[must_use]
    pub fn with_post_reboot_products(mut self, system: System) -> Self {
        self.post_reboot_products = Some(system);
        self
    }

    /// After `reboot`, clear `products` to `None` (post-reboot re-read
    /// succeeded but yielded no product state).
    #[must_use]
    pub const fn with_post_reboot_no_products(mut self) -> Self {
        self.post_reboot_clear_products = true;
        self
    }

    /// Make `read_products` fail (models a re-read failure after reboot).
    #[must_use]
    pub const fn with_read_products_err(mut self) -> Self {
        self.read_products_err = true;
        self
    }

    /// Make `read_repos` fail, leaving `raw_repos` unset (models a failed
    /// `zypper -x lr`; a *successful* read with zero repos is the default
    /// for unconfigured hosts).
    #[must_use]
    pub const fn with_read_repos_err(mut self) -> Self {
        self.read_repos_err = true;
        self
    }

    /// Enter `barrier` at the start of every `run` call (see [`RunBarrier`]).
    #[must_use]
    pub fn with_run_barrier(mut self, barrier: Arc<RunBarrier>) -> Self {
        self.run_barrier = Some(barrier);
        self
    }

    /// Attach shared counters; every instrumented method boundary reports
    /// through them. Opt-in only — omitting this call preserves prior
    /// zero-overhead default behavior.
    #[must_use]
    pub fn with_metrics(mut self, metrics: MockMetrics) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Make every call to the `kind` method wait on `gate` before proceeding
    /// (after metrics have already recorded it as in-flight).
    #[must_use]
    pub fn with_gate(mut self, kind: MockOpKind, gate: Arc<MockGate>) -> Self {
        self.gates.insert(kind, gate);
        self
    }

    /// Queue scripted outcomes for the next `run` calls.
    pub fn push_run(&mut self, outcome: MockRunOutcome) {
        self.run_queue.push(outcome);
    }

    pub const fn fail_connect(&mut self) {
        self.connect_fail = true;
    }

    fn take_outcome(&mut self) -> MockRunOutcome {
        if self.run_queue.is_empty() {
            MockRunOutcome::exit(0)
        } else {
            self.run_queue.remove(0)
        }
    }

    /// Record `kind` as entered (if metrics attached) and return the guard
    /// that marks it complete on drop.
    fn enter_op(&self, kind: MockOpKind) -> Option<OpGuard> {
        self.metrics.as_ref().map(|m| m.enter_op(kind))
    }

    /// Wait on the gate registered for `kind`, if any.
    async fn wait_gate(&self, kind: MockOpKind) {
        if let Some(gate) = self.gates.get(&kind) {
            gate.wait().await;
        }
    }
}

#[async_trait]
impl Host for MockHost {
    fn key(&self) -> &str {
        &self.key
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn products(&self) -> Option<&System> {
        self.products.as_ref()
    }

    fn raw_repos(&self) -> Option<&[Repository]> {
        self.raw_repos.as_deref()
    }

    fn repos(&self) -> Option<&Repositories> {
        self.repos.as_ref()
    }

    fn out(&self) -> &[OutEntry] {
        &self.out
    }

    async fn connect(&mut self) -> Result<(), SshError> {
        let _guard = self.enter_op(MockOpKind::Connect);
        self.wait_gate(MockOpKind::Connect).await;
        if self.connect_fail {
            self.connected = false;
            return Err(SshError::Transport(format!(
                "mock connect failed for {}",
                self.key
            )));
        }
        self.connected = true;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), SshError> {
        let _guard = self.enter_op(MockOpKind::Close);
        self.wait_gate(MockOpKind::Close).await;
        self.connected = false;
        Ok(())
    }

    async fn run(&mut self, command: &str) -> Result<(), SshError> {
        let _guard = self.enter_op(MockOpKind::Run);
        if !self.connected {
            // Real-host parity: a failed dispatch still records a synthetic
            // out entry (rc -1, empty streams) so the report cannot desync.
            self.out
                .push((command.to_string(), String::new(), String::new(), -1, 0));
            return Err(SshError::NotConnected(self.key.clone()));
        }
        self.wait_gate(MockOpKind::Run).await;
        if let Some(m) = &self.metrics {
            m.record_command_attempted();
        }
        if let Some(barrier) = self.run_barrier.clone() {
            barrier.enter().await;
        }
        self.ran.push(command.to_string());
        match self.take_outcome() {
            MockRunOutcome::Complete {
                stdout,
                stderr,
                exitcode,
                runtime_secs,
            } => {
                self.out
                    .push((command.to_string(), stdout, stderr, exitcode, runtime_secs));
                if let Some(m) = &self.metrics {
                    m.record_command_completed();
                }
                Ok(())
            }
            MockRunOutcome::Timeout { .. } => {
                // Real-host parity: timeout appends (command, "", "", -1) —
                // the diagnostics go to the log, not the entry's stderr.
                self.out
                    .push((command.to_string(), String::new(), String::new(), -1, 0));
                if let Some(m) = &self.metrics {
                    m.record_command_completed();
                }
                Ok(())
            }
            MockRunOutcome::TransportErr(_) => {
                // Real-host parity: a mid-command transport failure appends
                // a synthetic (command, "", "", -1) entry and returns Ok.
                self.out
                    .push((command.to_string(), String::new(), String::new(), -1, 0));
                if let Some(m) = &self.metrics {
                    m.record_command_completed();
                }
                Ok(())
            }
        }
    }

    async fn read_products(&mut self) -> Result<(), SshError> {
        let _guard = self.enter_op(MockOpKind::ReadProducts);
        self.wait_gate(MockOpKind::ReadProducts).await;
        // Real-host parity: connect lazily instead of failing outright.
        if !self.connected {
            self.connect().await?;
        }
        if self.read_products_err {
            return Err(SshError::Transport(format!(
                "mock read_products failed for {}",
                self.key
            )));
        }
        // Products are injected by the test; nothing to fetch.
        Ok(())
    }

    async fn read_repos(&mut self) -> Result<(), SshError> {
        let _guard = self.enter_op(MockOpKind::ReadRepos);
        self.wait_gate(MockOpKind::ReadRepos).await;
        // Real-host parity: a disconnected host is a silent no-op.
        if !self.connected {
            return Ok(());
        }
        if self.read_repos_err {
            // Real-host parity: the failed zypper call was still recorded
            // as an out entry, and the error is `Other`, not `Transport`.
            self.out.push((
                "zypper -x lr".to_string(),
                String::new(),
                String::new(),
                4,
                0,
            ));
            return Err(SshError::Other(format!(
                "mock read_repos failed for {}",
                self.key
            )));
        }
        // Real-host parity: the zypper call is recorded as an out entry
        // like any other command (appended directly, NOT via `run`, so the
        // `ran` instrumentation and command counters stay stable). The mock
        // has no XML to store, so the streams stay empty.
        self.out.push((
            "zypper -x lr".to_string(),
            String::new(),
            String::new(),
            0,
            0,
        ));
        // A successful read: preserve injected repos; an unconfigured host
        // read successfully and found zero repositories — `Some(empty)`,
        // which is NOT the same state as a failed read (`None`).
        if self.raw_repos.is_none() {
            self.raw_repos = Some(Vec::new());
        }
        Ok(())
    }

    async fn parse_repos(&mut self) -> Result<(), SshError> {
        let _guard = self.enter_op(MockOpKind::ParseRepos);
        self.wait_gate(MockOpKind::ParseRepos).await;
        if self.products.is_none() {
            self.read_products().await?;
        }
        if self.raw_repos.is_none() {
            self.read_repos().await?;
        }
        if self.repos.is_none() {
            // Derived from the raw repos like `RusshHost::parse_repos`.
            // Deliberate delta: a `with_repos` fixture is kept as injected
            // (the real host rebuilds unconditionally) — that is what the
            // uninstall/remove command tests configure.
            let arch = self
                .products
                .as_ref()
                .map(|p| p.arch().to_string())
                .unwrap_or_else(|| "unknown".into());
            let raw = self.raw_repos.clone().unwrap_or_default();
            self.repos = Some(crate::types::repositories_from_raw(&raw, &arch));
        }
        Ok(())
    }

    async fn reboot(&mut self, command: &str) -> Result<bool, SshError> {
        let _guard = self.enter_op(MockOpKind::Reboot);
        self.wait_gate(MockOpKind::Reboot).await;
        // Mock: record reboot command via run semantics (counted separately
        // as its own Run operation/command), then "reconnect".
        self.run(command).await?;
        // The real host dispatches via `fire_and_forget`, which FAILS when
        // the exec never left the client. A scripted TransportErr/Timeout
        // produces a synthetic rc -1 dispatch entry — model the dispatch
        // failure instead of reporting a successful reboot.
        if self.out.last().is_some_and(|entry| entry.3 == -1) {
            return Err(SshError::Transport(format!(
                "mock reboot dispatch failed for {}",
                self.key
            )));
        }
        self.connected = true;
        // Model the post-reboot product change (e.g. product now removed).
        if let Some(sys) = self.post_reboot_products.take() {
            self.products = Some(sys);
        }
        if self.post_reboot_clear_products {
            self.products = None;
        }
        Ok(true)
    }
}

/// Map of mock hosts with isolated `run_all`.
#[derive(Debug, Default)]
pub struct MockHostGroup {
    hosts: BTreeMap<String, MockHost>,
}

impl MockHostGroup {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, host: MockHost) {
        self.hosts.insert(host.key().to_string(), host);
    }

    pub fn get_mock_mut(&mut self, key: &str) -> Option<&mut MockHost> {
        self.hosts.get_mut(key)
    }
}

#[async_trait]
impl HostGroup for MockHostGroup {
    fn keys(&self) -> Vec<String> {
        self.hosts.keys().cloned().collect()
    }

    fn get(&self, key: &str) -> Option<&dyn Host> {
        self.hosts.get(key).map(|h| h as &dyn Host)
    }

    fn get_mut(&mut self, key: &str) -> Option<&mut dyn Host> {
        self.hosts.get_mut(key).map(|h| h as &mut dyn Host)
    }

    fn hosts_mut(&mut self) -> Vec<&mut dyn Host> {
        // BTreeMap::values_mut iterates in ascending key order.
        self.hosts
            .values_mut()
            .map(|h| h as &mut dyn Host)
            .collect()
    }

    // Concurrent, order-preserving, failure-isolated fan-out matching
    // `RusshHostGroup` (see `repose-ssh/src/host.rs`): `join_all` preserves
    // input order and the map is a `BTreeMap`, so results stay key-ordered;
    // one host's error never stops or is counted against its siblings.
    async fn connect_and_prune(&mut self) {
        let failed: Vec<String> =
            join_all(self.hosts.iter_mut().map(|(key, host)| async move {
                host.connect().await.is_err().then(|| key.clone())
            }))
            .await
            .into_iter()
            .flatten()
            .collect();
        for key in failed {
            self.hosts.remove(&key);
        }
    }

    async fn read_products(&mut self) {
        join_all(self.hosts.values_mut().map(|host| async move {
            let _ = host.read_products().await;
        }))
        .await;
    }

    async fn read_repos(&mut self) {
        join_all(self.hosts.values_mut().map(|host| async move {
            let _ = host.read_repos().await;
        }))
        .await;
    }

    async fn parse_repos(&mut self) {
        join_all(self.hosts.values_mut().map(|host| async move {
            let _ = host.parse_repos().await;
        }))
        .await;
    }

    async fn run_all(&mut self, cmd: &str) {
        join_all(self.hosts.values_mut().map(|host| async move {
            let _ = host.run(cmd).await;
        }))
        .await;
    }

    async fn close(&mut self) {
        join_all(self.hosts.values_mut().map(|host| async move {
            let _ = host.close().await;
        }))
        .await;
    }
}

/// Probe that always returns the configured answer (tests).
#[derive(Debug, Clone)]
pub struct ConstProbe {
    pub live: bool,
}

#[async_trait]
impl Probe for ConstProbe {
    async fn is_live(&self, _url: &str, _timeout: Duration) -> bool {
        self.live
    }
}

/// Per-URL probe: every listed URL is dead, everything else is live.
///
/// Needed to exercise reset's safety-critical *partial-drop* guard, which
/// [`ConstProbe`] (all-or-nothing) cannot reach.
#[derive(Debug, Clone, Default)]
pub struct MapProbe {
    dead_urls: std::collections::HashSet<String>,
}

impl MapProbe {
    /// Mark the given exact URLs as dead (all others live).
    #[must_use]
    pub fn dead(urls: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            dead_urls: urls.into_iter().map(Into::into).collect(),
        }
    }
}

#[async_trait]
impl Probe for MapProbe {
    async fn is_live(&self, url: &str, _timeout: Duration) -> bool {
        !self.dead_urls.contains(url)
    }
}

/// Counting/gated [`Probe`] test double: per-URL outcome overrides on top of
/// a default, optional shared [`MockMetrics`], and an optional [`MockGate`]
/// for deterministic overlap proofs. `ConstProbe`/`MapProbe` remain the
/// simple choice when metrics/gating are not needed.
#[derive(Debug, Clone, Default)]
pub struct MetricProbe {
    default_live: bool,
    overrides: std::collections::HashMap<String, bool>,
    metrics: Option<MockMetrics>,
    gate: Option<Arc<MockGate>>,
}

impl MetricProbe {
    #[must_use]
    pub fn new(default_live: bool) -> Self {
        Self {
            default_live,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_metrics(mut self, metrics: MockMetrics) -> Self {
        self.metrics = Some(metrics);
        self
    }

    #[must_use]
    pub fn with_gate(mut self, gate: Arc<MockGate>) -> Self {
        self.gate = Some(gate);
        self
    }

    /// Override the outcome for one exact URL (input-independent lookup).
    #[must_use]
    pub fn set(mut self, url: impl Into<String>, live: bool) -> Self {
        self.overrides.insert(url.into(), live);
        self
    }
}

#[async_trait]
impl Probe for MetricProbe {
    async fn is_live(&self, url: &str, _timeout: Duration) -> bool {
        let _guard = self.metrics.as_ref().map(|m| m.enter_probe(url));
        if let Some(gate) = &self.gate {
            gate.wait().await;
        }
        self.overrides
            .get(url)
            .copied()
            .unwrap_or(self.default_live)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::last_out_succeeded;
    use crate::types::{Product, zypper_exit_ok};

    #[tokio::test]
    async fn parse_repos_derives_from_raw_repos_like_the_real_host() {
        // Production-shaped host: products + raw `zypper -x lr` rows, no
        // prebuilt `Repositories`. The mock must derive the alias→product
        // map exactly like `RusshHost::parse_repos`, so command tests
        // exercise the same path the transport takes.
        let mut h = MockHost::new("h1")
            .with_products(System {
                base: Product {
                    name: "SLES".into(),
                    version: "15-SP6".into(),
                    arch: "x86_64".into(),
                },
                addons: vec![],
                transactional: false,
            })
            .with_raw_repos(vec![
                Repository {
                    alias: "SLES:15-SP6::pool".into(),
                    name: "SLES:15-SP6:pool:x86_64".into(),
                    url: "http://x/".into(),
                    state: true,
                },
                Repository {
                    alias: "weird".into(),
                    name: "not-a-product-string".into(),
                    url: "http://y/".into(),
                    state: true,
                },
            ]);
        h.connect().await.unwrap();
        h.parse_repos().await.unwrap();

        let repos = h.repos().expect("repos built from raw");
        assert_eq!(repos.len(), 2);
        let product = repos
            .get("SLES:15-SP6::pool")
            .and_then(|p| p.as_ref())
            .expect("4-part repo name parses to a product");
        assert_eq!(product.name, "SLES");
        assert_eq!(product.version, "15-SP6");
        assert_eq!(product.arch, "x86_64");
        assert!(
            repos.get("weird").expect("alias present").is_none(),
            "non-product repo name maps to the None sentinel"
        );
    }

    #[tokio::test]
    async fn read_repos_unconfigured_models_a_successful_empty_read() {
        // `None` is reserved for "read failed/never ran" (e.g. zypper
        // failure); a plain unconfigured host read successfully and found
        // zero repositories. Real-host parity: the zypper call is recorded
        // as an out entry, so `last_out_succeeded` answers like production.
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h.read_repos().await.unwrap();
        assert_eq!(h.raw_repos(), Some(&[][..]));
        assert_eq!(h.out().len(), 1);
        assert_eq!(h.out()[0].0, "zypper -x lr");
        assert_eq!(last_out_succeeded(h.out()), Some(true));
    }

    #[tokio::test]
    async fn read_repos_err_keeps_raw_repos_unset() {
        // Real-host parity for a failed `zypper -x lr`: the failed call is
        // still recorded (non-zero rc) and the error is `Other`.
        let mut h = MockHost::new("h1").with_read_repos_err();
        h.connect().await.unwrap();
        let err = h.read_repos().await.unwrap_err();
        assert!(matches!(err, SshError::Other(_)));
        assert!(
            h.raw_repos().is_none(),
            "failed read must not masquerade as empty"
        );
        assert_eq!(h.out().len(), 1);
        assert_eq!(h.out()[0].0, "zypper -x lr");
        assert_eq!(h.out()[0].3, 4);
        // parse_repos propagates the failure and builds nothing.
        assert!(h.parse_repos().await.is_err());
        assert!(h.repos().is_none());
    }

    #[tokio::test]
    async fn reboot_fails_on_a_scripted_dispatch_failure() {
        // The real host's fire_and_forget fails when the exec never left
        // the client; a scripted TransportErr must fail the reboot, not
        // report a successful reconnect.
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h.push_run(MockRunOutcome::TransportErr("link down".into()));
        let err = h.reboot("systemctl reboot").await.unwrap_err();
        assert!(matches!(err, SshError::Transport(_)));
        // Real-host parity: a dispatch failure is NOT a disconnect — the
        // host only goes disconnected after a dispatched reboot.
        assert!(h.is_connected());
    }

    #[tokio::test]
    async fn read_products_connects_lazily_like_the_real_host() {
        let mut h = MockHost::new("h1");
        assert!(!h.is_connected());
        h.read_products().await.unwrap();
        assert!(h.is_connected());
    }

    #[tokio::test]
    async fn read_repos_on_a_disconnected_host_is_a_no_op() {
        let mut h = MockHost::new("h1");
        assert!(h.read_repos().await.is_ok());
        assert!(h.raw_repos().is_none());
        assert!(h.out().is_empty(), "no zypper call was dispatched");
    }

    #[tokio::test]
    async fn run_appends_success_exit() {
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h.push_run(MockRunOutcome::exit(0));
        h.run("zypper -n lr").await.unwrap();
        assert_eq!(h.out().len(), 1);
        assert_eq!(h.out()[0].0, "zypper -n lr");
        assert_eq!(h.out()[0].3, 0);
        assert_eq!(last_out_succeeded(h.out()), Some(true));
    }

    #[tokio::test]
    async fn run_nonzero_zypper_is_ok_not_err() {
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        // exit 4 is zypper failure but still Ok(()) + out entry
        h.push_run(MockRunOutcome::exit(4));
        h.run("zypper -n ar x").await.unwrap();
        assert_eq!(h.out().len(), 1);
        assert_eq!(h.out()[0].3, 4);
        assert!(!zypper_exit_ok(4));
        assert_eq!(last_out_succeeded(h.out()), Some(false));
    }

    #[tokio::test]
    async fn timeout_appends_minus_one_with_empty_streams_and_returns_ok() {
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h.push_run(MockRunOutcome::Timeout {
            stderr: "command timed out".into(),
        });
        h.run("sleep 999").await.unwrap();
        assert_eq!(h.out().len(), 1);
        assert_eq!(h.out()[0].3, -1);
        // Real-host parity: the diagnostics go to the log, NOT the entry.
        assert_eq!(h.out()[0].1, "");
        assert_eq!(h.out()[0].2, "");
        assert_eq!(last_out_succeeded(h.out()), Some(false));
    }

    #[tokio::test]
    async fn transport_err_appends_synthetic_and_returns_ok() {
        // Real-host parity: a mid-command transport failure is recorded as a
        // synthetic (cmd, "", "", -1) entry and reported via `Ok` + rc -1,
        // exactly like `RusshHost::run` (the message goes to the log).
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h.push_run(MockRunOutcome::TransportErr("boom".into()));
        h.run("x").await.unwrap();
        assert_eq!(
            h.out(),
            &[("x".to_string(), String::new(), String::new(), -1, 0)]
        );
        assert_eq!(last_out_succeeded(h.out()), Some(false));
    }

    #[tokio::test]
    async fn not_connected_is_err_with_a_synthetic_out_entry() {
        // Real-host parity: a failed dispatch still records the attempt so
        // the report cannot desync (same shape as the `RusshHost` test).
        let mut h = MockHost::new("h1");
        let err = h.run("x").await.unwrap_err();
        assert!(matches!(err, SshError::NotConnected(_)));
        assert_eq!(
            h.out(),
            &[("x".to_string(), String::new(), String::new(), -1, 0)]
        );
    }

    #[test]
    fn hosts_mut_matches_keys_order() {
        // Exit aggregation relies on hosts_mut() lining up with keys().
        let mut g = MockHostGroup::new();
        g.insert(MockHost::new("b"));
        g.insert(MockHost::new("a"));
        g.insert(MockHost::new("c"));
        let order: Vec<String> = g.hosts_mut().iter().map(|h| h.key().to_string()).collect();
        assert_eq!(order, vec!["a", "b", "c"]);
        assert_eq!(order, g.keys());
    }

    #[tokio::test]
    async fn connect_and_prune_drops_failures() {
        let mut g = MockHostGroup::new();
        let mut bad = MockHost::new("bad");
        bad.fail_connect();
        g.insert(MockHost::new("good"));
        g.insert(bad);
        g.connect_and_prune().await;
        assert_eq!(g.keys(), vec!["good".to_string()]);
    }

    #[tokio::test]
    async fn run_all_isolates_failures() {
        let mut g = MockHostGroup::new();
        let mut a = MockHost::new("a");
        a.connect().await.unwrap();
        a.push_run(MockRunOutcome::TransportErr("fail".into()));
        let mut b = MockHost::new("b");
        b.connect().await.unwrap();
        b.push_run(MockRunOutcome::exit(0));
        g.insert(a);
        g.insert(b);
        g.run_all("true").await;
        // a's transport failure is a synthetic rc -1 entry, not a cancellation.
        assert_eq!(g.get_mock_mut("a").unwrap().out().len(), 1);
        assert_eq!(g.get_mock_mut("a").unwrap().out()[0].3, -1);
        assert_eq!(g.get_mock_mut("b").unwrap().out().len(), 1);
        assert_eq!(g.get_mock_mut("b").unwrap().out()[0].3, 0);
    }

    #[tokio::test]
    async fn zypper_success_codes_report_true() {
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        for code in [0, 100, 101, 102, 103, 106, 107] {
            h.push_run(MockRunOutcome::exit(code));
            h.run("cmd").await.unwrap();
            assert_eq!(last_out_succeeded(h.out()), Some(true), "code {code}");
        }
    }

    #[test]
    fn system_helpers() {
        let s = System {
            base: Product {
                name: "SLES".into(),
                version: "15-SP6".into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional: true,
        };
        assert!(s.is_transactional());
        assert_eq!(s.arch(), "x86_64");
    }

    #[tokio::test]
    async fn serial_operations_peak_at_one_and_leave_zero_in_flight() {
        let metrics = MockMetrics::new();
        let mut h = MockHost::new("h1").with_metrics(metrics.clone());
        h.connect().await.unwrap();
        h.push_run(MockRunOutcome::exit(0));
        h.run("cmd").await.unwrap();
        h.close().await.unwrap();

        let snap = metrics.snapshot();
        assert_eq!(snap.peak_operations, 1, "no overlap under serial calls");
        assert_eq!(snap.current_operations, 0);
        assert_eq!(snap.total_operations, 3, "connect + run + close");
        assert_eq!(snap.commands_attempted, 1);
        assert_eq!(snap.commands_completed, 1);
        assert_eq!(snap.operations_by_kind[&MockOpKind::Run], 1);
    }

    #[tokio::test]
    async fn not_connected_run_counts_operation_but_no_command() {
        let metrics = MockMetrics::new();
        let mut h = MockHost::new("h1").with_metrics(metrics.clone());
        let err = h.run("cmd").await.unwrap_err();
        assert!(matches!(err, SshError::NotConnected(_)));

        let snap = metrics.snapshot();
        assert_eq!(snap.total_operations, 1);
        assert_eq!(snap.current_operations, 0, "guard released on early return");
        assert_eq!(
            snap.commands_attempted, 0,
            "counted only past the connected check"
        );
        assert_eq!(
            snap.commands_completed, 0,
            "nothing past the check completes"
        );
    }

    #[tokio::test]
    async fn transport_failure_does_not_leak_in_flight_guard() {
        let metrics = MockMetrics::new();
        let mut h = MockHost::new("h1").with_metrics(metrics.clone());
        h.connect().await.unwrap();
        h.push_run(MockRunOutcome::TransportErr("boom".into()));
        h.run("cmd").await.unwrap();

        let snap = metrics.snapshot();
        assert_eq!(snap.current_operations, 0);
        assert_eq!(snap.commands_attempted, 1, "attempted past connected check");
        assert_eq!(
            snap.commands_completed, 1,
            "transport failure appends a synthetic out entry"
        );
    }

    #[tokio::test]
    async fn reboot_counts_separately_from_its_nested_run_command() {
        let metrics = MockMetrics::new();
        let mut h = MockHost::new("h1").with_metrics(metrics.clone());
        h.connect().await.unwrap();
        h.reboot("systemctl reboot").await.unwrap();

        let snap = metrics.snapshot();
        assert_eq!(snap.operations_by_kind[&MockOpKind::Reboot], 1);
        assert_eq!(snap.operations_by_kind[&MockOpKind::Run], 1);
        assert_eq!(snap.commands_attempted, 1);
        assert_eq!(snap.current_operations, 0);
    }

    #[tokio::test]
    async fn gated_hosts_overlap_reaches_expected_peak_operations() {
        let metrics = MockMetrics::new();
        let gate = MockGate::new();
        let mut h1 = MockHost::new("h1")
            .with_metrics(metrics.clone())
            .with_gate(MockOpKind::Run, gate.clone());
        h1.connect().await.unwrap();
        let mut h2 = MockHost::new("h2")
            .with_metrics(metrics.clone())
            .with_gate(MockOpKind::Run, gate.clone());
        h2.connect().await.unwrap();

        let t1 = tokio::spawn(async move {
            h1.run("cmd").await.unwrap();
            h1
        });
        let t2 = tokio::spawn(async move {
            h2.run("cmd").await.unwrap();
            h2
        });

        // Spin (bounded) until both operations are observably in-flight;
        // no real sleeps, no fixed wall-clock wait.
        for _ in 0..100_000 {
            if metrics.snapshot().current_operations >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(
            metrics.snapshot().current_operations,
            2,
            "both hosts must be in-flight before release"
        );
        gate.release();
        let (h1, h2) = tokio::join!(t1, t2);
        drop((h1.unwrap(), h2.unwrap()));

        let snap = metrics.snapshot();
        assert_eq!(snap.peak_operations, 2);
        assert_eq!(snap.current_operations, 0);
        assert_eq!(snap.commands_completed, 2);
    }

    #[tokio::test]
    async fn group_fan_out_is_concurrent_ordered_and_failure_isolated() {
        let metrics = MockMetrics::new();
        let gate = MockGate::new();
        let mut g = MockHostGroup::new();
        for key in ["a", "b", "c"] {
            let mut h = MockHost::new(key)
                .with_metrics(metrics.clone())
                .with_gate(MockOpKind::Run, gate.clone());
            h.connect().await.unwrap();
            if key == "b" {
                h.push_run(MockRunOutcome::TransportErr("fail".into()));
            }
            g.insert(h);
        }

        let run = tokio::spawn(async move {
            g.run_all("true").await;
            g
        });
        for _ in 0..100_000 {
            if metrics.snapshot().current_operations >= 3 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(
            metrics.snapshot().current_operations,
            3,
            "all three hosts must run concurrently"
        );
        gate.release();
        let mut g = run.await.unwrap();

        assert_eq!(g.keys(), vec!["a", "b", "c"], "BTreeMap keeps key order");
        assert!(g.get_mock_mut("a").unwrap().out().len() == 1);
        let b_out = g.get_mock_mut("b").unwrap().out().to_vec();
        assert_eq!(
            b_out.len(),
            1,
            "b's transport failure is one synthetic entry"
        );
        assert_eq!(b_out[0].3, -1);
        assert!(
            g.get_mock_mut("c").unwrap().out().len() == 1,
            "sibling failure does not cancel c"
        );

        let snap = metrics.snapshot();
        assert_eq!(snap.peak_operations, 3);
        assert_eq!(snap.current_operations, 0);
        assert_eq!(
            snap.commands_completed, 3,
            "a, b (synthetic), and c all appended an entry"
        );
    }

    #[tokio::test]
    async fn probe_metrics_count_per_url_and_reset_current_after_completion() {
        let metrics = MockMetrics::new();
        let probe = MetricProbe::new(true)
            .with_metrics(metrics.clone())
            .set("http://dead/", false);

        assert!(probe.is_live("http://a/", Duration::from_secs(1)).await);
        assert!(probe.is_live("http://a/", Duration::from_secs(1)).await);
        assert!(!probe.is_live("http://dead/", Duration::from_secs(1)).await);

        let snap = metrics.snapshot();
        assert_eq!(snap.probe_total, 3);
        assert_eq!(snap.current_probes, 0);
        assert_eq!(snap.probe_counts.get("http://a/"), Some(&2));
        assert_eq!(snap.probe_counts.get("http://dead/"), Some(&1));
    }

    #[tokio::test]
    async fn gated_probes_reach_expected_peak_overlap() {
        let metrics = MockMetrics::new();
        let gate = MockGate::new();
        let p1 = MetricProbe::new(true)
            .with_metrics(metrics.clone())
            .with_gate(gate.clone());
        let p2 = p1.clone();

        let t1 = tokio::spawn(async move { p1.is_live("http://a/", Duration::from_secs(1)).await });
        let t2 = tokio::spawn(async move { p2.is_live("http://b/", Duration::from_secs(1)).await });

        for _ in 0..100_000 {
            if metrics.snapshot().current_probes >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(metrics.snapshot().current_probes, 2);
        gate.release();
        let (r1, r2) = tokio::join!(t1, t2);
        assert!(r1.unwrap());
        assert!(r2.unwrap());

        let snap = metrics.snapshot();
        assert_eq!(snap.peak_probes, 2);
        assert_eq!(snap.current_probes, 0);
    }
}
