//! Host / session / probe traits — **no russh types**.
//!
//! Implementations live in `repose-ssh`. Command algorithms depend only
//! on these traits (see design: acyclic `ssh → core`).

use std::time::Duration;

use async_trait::async_trait;

use crate::error::SshError;
use crate::types::{OutEntry, Repositories, Repository, System};

/// Low-level SSH + SFTP session (Python `AsyncConnection` surface).
///
/// Command tests mock [`Host`], not this trait. Session mocks belong in
/// `repose-ssh` unit tests.
#[async_trait]
pub trait SshSession: Send {
    async fn connect(&mut self) -> Result<(), SshError>;

    /// Run a remote command. Returns `(stdout, stderr, exitcode)`.
    /// Timeout should surface as [`SshError`] at this layer; Host adapters
    /// translate timeout into an `out` entry with `exitcode == -1`.
    async fn run(&mut self, command: &str) -> Result<(String, String, i32), SshError>;

    async fn listdir(&mut self, path: &str) -> Result<Vec<String>, SshError>;

    async fn readlink(&mut self, path: &str) -> Result<Option<String>, SshError>;

    async fn read_file(&mut self, path: &str) -> Result<Vec<u8>, SshError>;

    async fn close(&mut self) -> Result<(), SshError>;

    fn is_active(&self) -> bool;

    /// Dispatch command without waiting for completion (reboot).
    /// Must fail if the exec never left the client.
    async fn fire_and_forget(&mut self, command: &str) -> Result<(), SshError>;

    /// `cat /proc/sys/kernel/random/boot_id` or empty string if unreadable.
    async fn boot_id(&mut self) -> String;

    /// Sleep-first reconnect. Python defaults: `retry=10`, `timeout=10`, `backoff=true`.
    async fn wait_reconnect(&mut self, retry: u32, timeout_secs: u64, backoff: bool) -> bool;
}

/// Per-host surface used by command workers (Python `AsyncTarget`).
///
/// # `run` / `out` contract
///
/// 1. **Always append** an [`OutEntry`] when a remote attempt completed or
///    timed out. Timeout / missing status → `exitcode = -1`.
/// 2. **`Ok(())` means session I/O finished and `out` was updated.** Remote
///    non-zero exit codes are **not** `Err`.
/// 3. **`Err` only when no `out` entry was written** (pre-append transport
///    failure).
/// 4. Callers that report via last `out` entry require a non-empty history
///    after a successful `Ok` return from `run`.
#[async_trait]
pub trait Host: Send {
    /// Map key: hostname or `host:port` when non-default port.
    fn key(&self) -> &str;

    fn is_connected(&self) -> bool;

    fn products(&self) -> Option<&System>;

    fn raw_repos(&self) -> Option<&[Repository]>;

    fn repos(&self) -> Option<&Repositories>;

    /// Command history; last entry drives success reporting.
    fn out(&self) -> &[OutEntry];

    async fn connect(&mut self) -> Result<(), SshError>;

    async fn close(&mut self) -> Result<(), SshError>;

    /// Execute remote command under the `run` / `out` contract above.
    async fn run(&mut self, command: &str) -> Result<(), SshError>;

    async fn read_products(&mut self) -> Result<(), SshError>;

    async fn read_repos(&mut self) -> Result<(), SshError>;

    /// Ensure products + raw_repos, then build [`Repositories`].
    async fn parse_repos(&mut self) -> Result<(), SshError>;

    /// fire-and-forget reboot command, wait_reconnect, re-read products.
    /// Returns whether reconnect appeared successful.
    async fn reboot(&mut self, command: &str) -> Result<bool, SshError>;
}

/// Multi-host fan-out (Python `AsyncHostGroup`).
///
/// Object-safe: hosts are accessed by key rather than returning
/// `impl Iterator<Item = &mut dyn Host>` (not object-safe).
#[async_trait]
pub trait HostGroup: Send {
    fn keys(&self) -> Vec<String>;

    fn get(&self, key: &str) -> Option<&dyn Host>;

    fn get_mut(&mut self, key: &str) -> Option<&mut dyn Host>;

    /// All hosts mutably at once, in ascending key order (same order as
    /// [`Self::keys`]).
    ///
    /// Mutation commands fan their per-host work out concurrently over this
    /// list (`join_all`, which preserves input order), so per-host results
    /// line up with `keys()` for exit aggregation.
    fn hosts_mut(&mut self) -> Vec<&mut dyn Host>;

    /// Connect all; drop hosts that fail to connect.
    async fn connect_and_prune(&mut self);

    async fn read_products(&mut self);

    async fn read_repos(&mut self);

    async fn parse_repos(&mut self);

    /// Isolated fan-out: one host failure must not cancel siblings.
    async fn run_all(&mut self, cmd: &str);

    async fn close(&mut self);
}

/// Repository URL liveness probe (Python `check_repo_url_async`).
#[async_trait]
pub trait Probe: Send + Sync {
    async fn is_live(&self, url: &str, timeout: Duration) -> bool;
}

/// Classify last `out` entry like Python `_report_target` (success codes only).
///
/// Returns `None` if `out` is empty (caller should treat as failure / bug).
#[must_use]
pub fn last_out_succeeded(out: &[OutEntry]) -> Option<bool> {
    let entry = out.last()?;
    Some(crate::types::zypper_exit_ok(entry.3))
}
