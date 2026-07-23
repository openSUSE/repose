//! In-memory [`Host`] / [`HostGroup`] for L2 command tests (no SSH).

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::Poll;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{StreamExt, stream};

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
    /// Hard timeout: still appends `out` with `exitcode == -1`, returns `Ok(())`.
    Timeout { stderr: String },
    /// Pre-append transport failure: **no** `out` entry, returns `Err`.
    TransportErr(String),
}

impl MockRunOutcome {
    #[cfg(test)]
    #[must_use]
    pub(crate) fn ok_stdout(stdout: impl Into<String>) -> Self {
        Self::Complete {
            stdout: stdout.into(),
            stderr: String::new(),
            exitcode: 0,
            runtime_secs: 0,
        }
    }

    #[must_use]
    const fn exit(code: i32) -> Self {
        Self::Complete {
            stdout: String::new(),
            stderr: String::new(),
            exitcode: code,
            runtime_secs: 0,
        }
    }
}

/// Cooperative rendezvous for concurrency tests: `enter`
/// completes only once `total` participants have entered.
///
/// Under truly concurrent fan-out every participant reaches the barrier
/// while the others are still inside it, so it releases immediately. Under
/// serial per-host execution the first participant can never be joined by
/// the second; it gives up after a bounded number of polls and sets
/// `timed_out`, which tests assert against (no hang, no
/// wall-clock timing).
#[derive(Debug)]
pub struct RunBarrier {
    total: usize,
    entered: AtomicUsize,
    timed_out: AtomicBool,
}

impl RunBarrier {
    #[cfg(test)]
    #[must_use]
    pub(crate) fn new(total: usize) -> Arc<Self> {
        Arc::new(Self {
            total,
            entered: AtomicUsize::new(0),
            timed_out: AtomicBool::new(false),
        })
    }

    /// Enter the barrier and cooperatively wait for the other participants.
    async fn enter(&self) {
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
    #[cfg(test)]
    #[must_use]
    pub(crate) fn timed_out(&self) -> bool {
        self.timed_out.load(Ordering::SeqCst)
    }
}

/// Host-operation category for opt-in gating/metrics on mock test doubles.
///
/// Every [`Host`] method boundary maps to exactly one kind; `reboot`'s
/// nested `run` call is counted separately under [`Self::Run`] (see
/// `MockMetricsSnapshot::operations_by_kind`).
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
    total_operations: usize,
    pub current_operations: usize,
    pub peak_operations: usize,
    operations_by_kind: BTreeMap<MockOpKind, usize>,
    /// `Host::run` calls that passed the connected-state check.
    commands_attempted: usize,
    /// Commands that appended an `out` entry (excludes transport failures).
    pub commands_completed: usize,
    pub probe_total: usize,
    pub current_probes: usize,
    peak_probes: usize,
    probe_counts: BTreeMap<String, usize>,
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
    out: Vec<OutEntry>,
    /// FIFO outcomes for successive `run` calls. Empty → default exit 0.
    run_queue: Vec<MockRunOutcome>,
    /// Commands observed by `run` (in order).
    pub(crate) ran: Vec<String>,
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

    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_raw_repos(mut self, repos: Vec<Repository>) -> Self {
        self.raw_repos = Some(repos);
        self
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_repos(mut self, repos: Repositories) -> Self {
        self.repos = Some(repos);
        self
    }

    /// System that `reboot` swaps into `products`, modelling the post-reboot
    /// product re-read that transactional install/uninstall verify against.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_post_reboot_products(mut self, system: System) -> Self {
        self.post_reboot_products = Some(system);
        self
    }

    /// After `reboot`, clear `products` to `None` (post-reboot re-read
    /// succeeded but yielded no product state).
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn with_post_reboot_no_products(mut self) -> Self {
        self.post_reboot_clear_products = true;
        self
    }

    /// Make `read_products` fail (models a re-read failure after reboot).
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn with_read_products_err(mut self) -> Self {
        self.read_products_err = true;
        self
    }

    /// Enter `barrier` at the start of every `run` call (see [`RunBarrier`]).
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_run_barrier(mut self, barrier: Arc<RunBarrier>) -> Self {
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
    #[cfg(test)]
    pub(crate) fn push_run(&mut self, outcome: MockRunOutcome) {
        self.run_queue.push(outcome);
    }

    #[cfg(test)]
    const fn fail_connect(&mut self) {
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
            // Pre-append failure: not connected, no out entry, no command
            // counted (only attempts past the connected-state check count).
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
            MockRunOutcome::Timeout { stderr } => {
                // Contract: timeout still appends exitcode -1, returns Ok.
                self.out
                    .push((command.to_string(), String::new(), stderr, -1, 0));
                if let Some(m) = &self.metrics {
                    m.record_command_completed();
                }
                Ok(())
            }
            MockRunOutcome::TransportErr(msg) => {
                // Contract: no out entry when Err; command was attempted
                // but did not complete, so it stays distinguishable.
                Err(SshError::Transport(msg))
            }
        }
    }

    async fn read_products(&mut self) -> Result<(), SshError> {
        let _guard = self.enter_op(MockOpKind::ReadProducts);
        self.wait_gate(MockOpKind::ReadProducts).await;
        if !self.connected {
            return Err(SshError::NotConnected(self.key.clone()));
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
        if !self.connected {
            return Err(SshError::NotConnected(self.key.clone()));
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
            self.repos = Some(Repositories::new());
        }
        Ok(())
    }

    async fn reboot(&mut self, command: &str) -> Result<bool, SshError> {
        let _guard = self.enter_op(MockOpKind::Reboot);
        self.wait_gate(MockOpKind::Reboot).await;
        // Mock: record reboot command via run semantics (counted separately
        // as its own Run operation/command), then "reconnect".
        self.run(command).await?;
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
#[derive(Debug)]
pub struct MockHostGroup {
    hosts: BTreeMap<String, MockHost>,
    host_operation_limit: NonZeroUsize,
}

impl Default for MockHostGroup {
    fn default() -> Self {
        Self {
            hosts: BTreeMap::new(),
            // Single source of truth for the approved default (see
            // `tests/performance/p1-limit-decision.md`) rather than a
            // second copy of the magic number.
            host_operation_limit: crate::config::ConnectionConfig::default().host_operation_limit,
        }
    }
}

impl MockHostGroup {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the configured host-operation concurrency cap (default:
    /// the approved P1 value). Mainly for tests exercising limits of `1`
    /// and values greater than the host count.
    #[must_use]
    pub fn with_host_operation_limit(mut self, limit: NonZeroUsize) -> Self {
        self.host_operation_limit = limit;
        self
    }

    pub fn insert(&mut self, host: MockHost) {
        self.hosts.insert(host.key().to_string(), host);
    }

    #[cfg(test)]
    pub(crate) fn get_mock_mut(&mut self, key: &str) -> Option<&mut MockHost> {
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

    fn host_operation_limit(&self) -> NonZeroUsize {
        self.host_operation_limit
    }

    // Bounded, order-preserving (for connect_and_prune's removal set;
    // BTreeMap iteration is key-ordered), failure-isolated fan-out matching
    // `RusshHostGroup` (see `repose-ssh/src/host.rs`): `buffer_unordered`
    // admits at most `host_operation_limit` operations at once — unlike
    // `.buffered`, a slow early host cannot block admission of later ones —
    // and one host's error never stops or is counted against its siblings.
    // These phases mutate host state in place (no per-host result vector to
    // reorder); order restoration is a mutation-worker concern (P1 steps
    // 13–18), not this trait's.
    async fn connect_and_prune(&mut self) {
        let cap = self.host_operation_limit.get();
        let failed: Vec<String> = stream::iter(self.hosts.iter_mut())
            .map(connect_one)
            .buffer_unordered(cap)
            .filter_map(std::future::ready)
            .collect()
            .await;
        for key in failed {
            self.hosts.remove(&key);
        }
    }

    async fn read_products(&mut self) {
        let cap = self.host_operation_limit.get();
        stream::iter(self.hosts.values_mut())
            .map(read_products_one)
            .buffer_unordered(cap)
            .collect::<Vec<()>>()
            .await;
    }

    async fn read_repos(&mut self) {
        let cap = self.host_operation_limit.get();
        stream::iter(self.hosts.values_mut())
            .map(read_repos_one)
            .buffer_unordered(cap)
            .collect::<Vec<()>>()
            .await;
    }

    async fn parse_repos(&mut self) {
        let cap = self.host_operation_limit.get();
        stream::iter(self.hosts.values_mut())
            .map(parse_repos_one)
            .buffer_unordered(cap)
            .collect::<Vec<()>>()
            .await;
    }

    async fn run_all(&mut self, cmd: &str) {
        let cap = self.host_operation_limit.get();
        // `.zip(repeat(cmd))` instead of a capturing closure: `.map(run_one)`
        // then stays a bare function item (see the note below `close`).
        stream::iter(self.hosts.values_mut().zip(std::iter::repeat(cmd)))
            .map(run_one)
            .buffer_unordered(cap)
            .collect::<Vec<()>>()
            .await;
    }

    async fn close(&mut self) {
        let cap = self.host_operation_limit.get();
        stream::iter(self.hosts.values_mut())
            .map(close_one)
            .buffer_unordered(cap)
            .collect::<Vec<()>>()
            .await;
    }
}

// Named async fns (not closures) as `.map()` arguments below: a closure
// returning `async move { host.method().await }` over a `&mut MockHost`
// borrowed per stream item fails higher-ranked lifetime inference against
// `buffer_unordered` ("implementation of `FnOnce` is not general enough")
// because async_trait's boxed-future return type ties the future to the
// closure's argument lifetime. A plain `fn` item has a concrete
// `for<'a> fn(&'a mut MockHost) -> impl Future + 'a` signature the
// compiler resolves without that ambiguity.
async fn connect_one((key, host): (&String, &mut MockHost)) -> Option<String> {
    host.connect().await.is_err().then(|| key.clone())
}

async fn read_products_one(host: &mut MockHost) {
    let _ = host.read_products().await;
}

async fn read_repos_one(host: &mut MockHost) {
    let _ = host.read_repos().await;
}

async fn parse_repos_one(host: &mut MockHost) {
    let _ = host.parse_repos().await;
}

async fn run_one((host, cmd): (&mut MockHost, &str)) {
    let _ = host.run(cmd).await;
}

async fn close_one(host: &mut MockHost) {
    let _ = host.close().await;
}

/// Probe that always returns the configured answer (tests).
#[derive(Debug, Clone)]
pub struct ConstProbe {
    pub(crate) live: bool,
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
    #[cfg(test)]
    #[must_use]
    pub(crate) fn dead(urls: impl IntoIterator<Item = impl Into<String>>) -> Self {
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
    #[cfg(test)]
    #[must_use]
    fn set(mut self, url: impl Into<String>, live: bool) -> Self {
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
    async fn timeout_appends_minus_one_and_returns_ok() {
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h.push_run(MockRunOutcome::Timeout {
            stderr: "command timed out".into(),
        });
        h.run("sleep 999").await.unwrap();
        assert_eq!(h.out().len(), 1);
        assert_eq!(h.out()[0].3, -1);
        assert_eq!(last_out_succeeded(h.out()), Some(false));
    }

    #[tokio::test]
    async fn transport_err_does_not_append() {
        let mut h = MockHost::new("h1");
        h.connect().await.unwrap();
        h.push_run(MockRunOutcome::TransportErr("boom".into()));
        let err = h.run("x").await.unwrap_err();
        assert!(matches!(err, SshError::Transport(_)));
        assert!(h.out().is_empty());
    }

    #[tokio::test]
    async fn not_connected_is_err_without_out() {
        let mut h = MockHost::new("h1");
        let err = h.run("x").await.unwrap_err();
        assert!(matches!(err, SshError::NotConnected(_)));
        assert!(h.out().is_empty());
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

    #[test]
    fn host_operation_limit_defaults_and_overrides_through_the_trait_object() {
        // Compile-time proof (P1 step 9): the accessor is callable through
        // `&mut dyn HostGroup`, not only the concrete type.
        let mut default_group = MockHostGroup::new();
        let as_trait_object: &mut dyn HostGroup = &mut default_group;
        assert_eq!(as_trait_object.host_operation_limit().get(), 32);

        let mut overridden =
            MockHostGroup::new().with_host_operation_limit(NonZeroUsize::new(1).unwrap());
        let as_trait_object: &mut dyn HostGroup = &mut overridden;
        assert_eq!(as_trait_object.host_operation_limit().get(), 1);
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
        assert!(g.get_mock_mut("a").unwrap().out().is_empty());
        assert_eq!(g.get_mock_mut("b").unwrap().out().len(), 1);
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
    }

    #[tokio::test]
    async fn transport_failure_does_not_leak_in_flight_guard() {
        let metrics = MockMetrics::new();
        let mut h = MockHost::new("h1").with_metrics(metrics.clone());
        h.connect().await.unwrap();
        h.push_run(MockRunOutcome::TransportErr("boom".into()));
        assert!(h.run("cmd").await.is_err());

        let snap = metrics.snapshot();
        assert_eq!(snap.current_operations, 0);
        assert_eq!(snap.commands_attempted, 1, "attempted past connected check");
        assert_eq!(
            snap.commands_completed, 0,
            "no out entry on transport error"
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
        assert!(
            g.get_mock_mut("b").unwrap().out().is_empty(),
            "b's transport failure leaves no out entry"
        );
        assert!(
            g.get_mock_mut("c").unwrap().out().len() == 1,
            "sibling failure does not cancel c"
        );

        let snap = metrics.snapshot();
        assert_eq!(snap.peak_operations, 3);
        assert_eq!(snap.current_operations, 0);
        assert_eq!(snap.commands_completed, 2, "a and c completed; b did not");
    }

    /// P1 step 11: `run_all` never admits more than `host_operation_limit`
    /// operations concurrently, even with more hosts than the limit and a
    /// gate that stays closed for the whole observation window.
    #[tokio::test]
    async fn bounded_run_all_never_exceeds_the_configured_limit() {
        let metrics = MockMetrics::new();
        let gate = MockGate::new();
        const LIMIT: usize = 2;
        const HOSTS: usize = 5;
        let mut g =
            MockHostGroup::new().with_host_operation_limit(NonZeroUsize::new(LIMIT).unwrap());
        for i in 0..HOSTS {
            let mut h = MockHost::new(format!("h{i}"))
                .with_metrics(metrics.clone())
                .with_gate(MockOpKind::Run, gate.clone());
            h.connect().await.unwrap();
            g.insert(h);
        }

        let run = tokio::spawn(async move {
            g.run_all("true").await;
            g
        });

        // Observe admission staying pinned at the limit (not the host
        // count) across many polls, proving the cap actually bounds
        // concurrency rather than just eventually reaching it once. Bounded
        // well under `MockGate`'s own 1_000_000-spin deadlock guard, which
        // every still-blocked host future also consumes on each poll.
        let mut saw_limit = false;
        for _ in 0..2_000 {
            let current = metrics.snapshot().current_operations;
            assert!(
                current <= LIMIT,
                "admitted {current} operations, exceeding the configured limit {LIMIT}"
            );
            if current == LIMIT {
                saw_limit = true;
            }
            tokio::task::yield_now().await;
        }
        assert!(saw_limit, "never observed the fleet saturate the limit");

        gate.release();
        let g = run.await.unwrap();

        let snap = metrics.snapshot();
        assert_eq!(
            snap.peak_operations, LIMIT,
            "peak must equal, not exceed, the limit"
        );
        assert_eq!(snap.current_operations, 0, "no leaked in-flight guard");
        assert_eq!(
            snap.commands_completed, HOSTS,
            "every host ran exactly once"
        );
        for i in 0..HOSTS {
            assert_eq!(g.get(&format!("h{i}")).unwrap().out().len(), 1);
        }
    }

    /// P1 step 11: `connect_and_prune` at limit 1 (fully serial admission)
    /// still prunes exactly the failed host and keeps the rest key-ordered.
    #[tokio::test]
    async fn bounded_connect_and_prune_at_limit_one_still_prunes_correctly() {
        let mut g = MockHostGroup::new().with_host_operation_limit(NonZeroUsize::new(1).unwrap());
        for key in ["c", "a", "b"] {
            let mut h = MockHost::new(key);
            if key == "b" {
                h.fail_connect();
            }
            g.insert(h);
        }
        g.connect_and_prune().await;
        assert_eq!(
            g.keys(),
            vec!["a".to_string(), "c".to_string()],
            "BTreeMap keeps ascending key order after pruning"
        );
    }

    /// P1 step 11: a limit above the host count behaves exactly like the
    /// unbounded `join_all` fan-out it replaces.
    #[tokio::test]
    async fn limit_above_host_count_admits_the_whole_fleet_at_once() {
        let metrics = MockMetrics::new();
        let gate = MockGate::new();
        let mut g = MockHostGroup::new().with_host_operation_limit(NonZeroUsize::new(100).unwrap());
        for key in ["a", "b", "c"] {
            let mut h = MockHost::new(key)
                .with_metrics(metrics.clone())
                .with_gate(MockOpKind::Run, gate.clone());
            h.connect().await.unwrap();
            g.insert(h);
        }
        let run = tokio::spawn(async move {
            g.run_all("true").await;
        });
        for _ in 0..100_000 {
            if metrics.snapshot().current_operations >= 3 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(metrics.snapshot().current_operations, 3);
        gate.release();
        run.await.unwrap();
        assert_eq!(metrics.snapshot().peak_operations, 3);
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
