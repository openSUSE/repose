//! In-memory [`Host`] / [`HostGroup`] for L2 command tests (no SSH).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::Poll;
use std::time::Duration;

use async_trait::async_trait;

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
    pub ran: Vec<String>,
    connect_fail: bool,
    /// When set, `run` enters this barrier before executing (concurrency
    /// proof in fan-out tests).
    run_barrier: Option<Arc<RunBarrier>>,
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

    /// Enter `barrier` at the start of every `run` call (see [`RunBarrier`]).
    #[must_use]
    pub fn with_run_barrier(mut self, barrier: Arc<RunBarrier>) -> Self {
        self.run_barrier = Some(barrier);
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
        self.connected = false;
        Ok(())
    }

    async fn run(&mut self, command: &str) -> Result<(), SshError> {
        if !self.connected {
            // Pre-append failure: not connected, no out entry.
            return Err(SshError::NotConnected(self.key.clone()));
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
                Ok(())
            }
            MockRunOutcome::Timeout { stderr } => {
                // Contract: timeout still appends exitcode -1, returns Ok.
                self.out
                    .push((command.to_string(), String::new(), stderr, -1, 0));
                Ok(())
            }
            MockRunOutcome::TransportErr(msg) => {
                // Contract: no out entry when Err.
                Err(SshError::Transport(msg))
            }
        }
    }

    async fn read_products(&mut self) -> Result<(), SshError> {
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
        if !self.connected {
            return Err(SshError::NotConnected(self.key.clone()));
        }
        Ok(())
    }

    async fn parse_repos(&mut self) -> Result<(), SshError> {
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
        // Mock: record reboot command via run semantics, then "reconnect".
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

    async fn connect_and_prune(&mut self) {
        let keys: Vec<String> = self.hosts.keys().cloned().collect();
        for key in keys {
            let fail = {
                let host = self.hosts.get_mut(&key).expect("key present");
                host.connect().await.is_err()
            };
            if fail {
                self.hosts.remove(&key);
            }
        }
    }

    async fn read_products(&mut self) {
        for host in self.hosts.values_mut() {
            let _ = host.read_products().await;
        }
    }

    async fn read_repos(&mut self) {
        for host in self.hosts.values_mut() {
            let _ = host.read_repos().await;
        }
    }

    async fn parse_repos(&mut self) {
        for host in self.hosts.values_mut() {
            let _ = host.parse_repos().await;
        }
    }

    async fn run_all(&mut self, cmd: &str) {
        // Isolated: collect keys first; errors do not stop siblings.
        let keys: Vec<String> = self.hosts.keys().cloned().collect();
        for key in keys {
            if let Some(host) = self.hosts.get_mut(&key) {
                let _ = host.run(cmd).await;
            }
        }
    }

    async fn close(&mut self) {
        for host in self.hosts.values_mut() {
            let _ = host.close().await;
        }
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
}
