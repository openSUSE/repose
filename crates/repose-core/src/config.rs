//! Transport configuration (Python `ConnectionConfig` without `ssh_backend`).

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;

// P1 approved defaults — see `tests/performance/p1-limit-decision.md` for
// the measured evidence and rationale behind every value below. Each
// constant maps to exactly one row of that decision table; changing a
// value here without updating the table is a policy change, not a
// refactor.

/// Maximum host operations (connect/read/parse/run/close phases, and one
/// complete per-host mutation worker) admitted concurrently per CLI
/// invocation. Shared by SSH group phases and mutation command workers
/// (one validated limit, not duplicated per command).
const HOST_OPERATION_LIMIT_DEFAULT: NonZeroUsize = NonZeroUsize::new(32).unwrap();

/// Maximum repository-URL liveness probes (`Probe::is_live`) in flight at
/// once, fleet-wide, per CLI invocation.
const PROBE_CONCURRENCY_LIMIT_DEFAULT: NonZeroUsize = NonZeroUsize::new(64).unwrap();

/// Maximum concurrent SFTP reads per session (product/addon file discovery).
const SFTP_READ_CONCURRENCY_LIMIT_DEFAULT: NonZeroUsize = NonZeroUsize::new(16).unwrap();

/// Maximum plausible `/etc/products.d` directory entries before a listing
/// is rejected outright.
const MAX_PRODUCTS_D_ENTRIES_DEFAULT: NonZeroUsize = NonZeroUsize::new(256).unwrap();

/// Maximum bytes read from one remote file over SFTP (`.prod` /
/// transactional-conf / `os-release`).
const MAX_SFTP_FILE_BYTES_DEFAULT: NonZeroUsize = NonZeroUsize::new(65536).unwrap(); // 64 KiB

/// Maximum bytes retained from one command's stdout, independently of stderr.
const MAX_STDOUT_BYTES_DEFAULT: NonZeroUsize = NonZeroUsize::new(262_144).unwrap(); // 256 KiB

/// Maximum bytes retained from one command's stderr, independently of stdout.
const MAX_STDERR_BYTES_DEFAULT: NonZeroUsize = NonZeroUsize::new(262_144).unwrap(); // 256 KiB

/// OpenSSH-style strict host key checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostKeyPolicy {
    Yes,
    #[default]
    AcceptNew,
    No,
    Off,
}

impl HostKeyPolicy {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "yes" => Some(Self::Yes),
            "accept-new" => Some(Self::AcceptNew),
            "no" => Some(Self::No),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::AcceptNew => "accept-new",
            Self::No => "no",
            Self::Off => "off",
        }
    }
}

/// Immutable connection settings shared by all hosts.
///
/// **No `ssh_backend` field** — Rust has a single SSH stack (russh).
///
/// # P1 resource limits and phase deadlines
///
/// The fields below (added for `plans/p1-bound-resources-and-prevent-stalls.md`)
/// bound fleet-wide resource usage and SSH phase stalls. Every default is
/// reviewed evidence, not a guess — see
/// `tests/performance/p1-limit-decision.md`. Concurrency and count limits
/// use [`NonZeroUsize`] so a zero (permanently-deadlocked or
/// always-truncating) limit cannot be constructed. Deadlines are separate
/// phase budgets, not one end-to-end timeout: a phase completing before its
/// own deadline is unaffected by the others. `timeout` (the existing
/// per-command budget) is unchanged and is not one of these phases.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionConfig {
    pub host_key_policy: HostKeyPolicy,
    pub known_hosts: Option<PathBuf>,
    /// Per-command SSH timeout in seconds (Python default 120).
    pub timeout: f64,

    /// Maximum host operations (SSH group phases, per-host mutation
    /// workers) admitted concurrently per CLI invocation.
    pub host_operation_limit: NonZeroUsize,
    /// Maximum URL-liveness probes in flight at once, fleet-wide.
    pub probe_concurrency_limit: NonZeroUsize,
    /// Maximum concurrent SFTP reads per session.
    pub sftp_read_concurrency_limit: NonZeroUsize,
    /// Maximum plausible `/etc/products.d` entries before a listing is
    /// rejected.
    pub max_products_d_entries: NonZeroUsize,
    /// Maximum bytes read from one remote file over SFTP.
    pub max_sftp_file_bytes: NonZeroUsize,
    /// Maximum bytes retained from one command's stdout.
    pub max_stdout_bytes: NonZeroUsize,
    /// Maximum bytes retained from one command's stderr.
    pub max_stderr_bytes: NonZeroUsize,

    /// DNS/TCP/proxy connect + SSH handshake budget.
    pub connect_deadline: Duration,
    /// Network authentication attempt budget (agent/public-key/password
    /// round trips only; interactive passphrase/password entry is
    /// user-paced and excluded).
    pub auth_deadline: Duration,
    /// Channel-open budget.
    pub channel_open_deadline: Duration,
    /// Exec dispatch / SFTP subsystem request budget.
    pub dispatch_deadline: Duration,
    /// One SFTP read/listdir/readlink operation's budget.
    pub sftp_operation_deadline: Duration,
    /// Bounded cleanup/drain budget after a stdout/stderr/SFTP overflow,
    /// before the typed error is returned.
    pub overflow_cleanup_deadline: Duration,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            host_key_policy: HostKeyPolicy::AcceptNew,
            known_hosts: None,
            timeout: 120.0,

            host_operation_limit: HOST_OPERATION_LIMIT_DEFAULT,
            probe_concurrency_limit: PROBE_CONCURRENCY_LIMIT_DEFAULT,
            sftp_read_concurrency_limit: SFTP_READ_CONCURRENCY_LIMIT_DEFAULT,
            max_products_d_entries: MAX_PRODUCTS_D_ENTRIES_DEFAULT,
            max_sftp_file_bytes: MAX_SFTP_FILE_BYTES_DEFAULT,
            max_stdout_bytes: MAX_STDOUT_BYTES_DEFAULT,
            max_stderr_bytes: MAX_STDERR_BYTES_DEFAULT,

            connect_deadline: Duration::from_secs(30),
            auth_deadline: Duration::from_secs(30),
            channel_open_deadline: Duration::from_secs(15),
            dispatch_deadline: Duration::from_secs(15),
            sftp_operation_deadline: Duration::from_secs(30),
            overflow_cleanup_deadline: Duration::from_secs(5),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_approved_p1_decision_table() {
        let config = ConnectionConfig::default();
        assert_eq!(config.host_operation_limit.get(), 32);
        assert_eq!(config.probe_concurrency_limit.get(), 64);
        assert_eq!(config.sftp_read_concurrency_limit.get(), 16);
        assert_eq!(config.max_products_d_entries.get(), 256);
        assert_eq!(config.max_sftp_file_bytes.get(), 65536);
        assert_eq!(config.max_stdout_bytes.get(), 262_144);
        assert_eq!(config.max_stderr_bytes.get(), 262_144);
        assert_eq!(config.connect_deadline, Duration::from_secs(30));
        assert_eq!(config.auth_deadline, Duration::from_secs(30));
        assert_eq!(config.channel_open_deadline, Duration::from_secs(15));
        assert_eq!(config.dispatch_deadline, Duration::from_secs(15));
        assert_eq!(config.sftp_operation_deadline, Duration::from_secs(30));
        assert_eq!(config.overflow_cleanup_deadline, Duration::from_secs(5));
        // The existing command-completion budget is untouched by P1.
        assert_eq!(config.timeout, 120.0);
    }

    #[test]
    fn zero_concurrency_or_count_limits_cannot_be_constructed() {
        assert_eq!(NonZeroUsize::new(0), None);
    }

    #[test]
    fn explicit_valid_values_round_trip_through_clone() {
        let config = ConnectionConfig {
            host_operation_limit: NonZeroUsize::new(1).unwrap(),
            probe_concurrency_limit: NonZeroUsize::new(1).unwrap(),
            sftp_read_concurrency_limit: NonZeroUsize::new(1).unwrap(),
            max_products_d_entries: NonZeroUsize::new(1).unwrap(),
            max_sftp_file_bytes: NonZeroUsize::new(1).unwrap(),
            max_stdout_bytes: NonZeroUsize::new(1).unwrap(),
            max_stderr_bytes: NonZeroUsize::new(1).unwrap(),
            connect_deadline: Duration::ZERO,
            auth_deadline: Duration::ZERO,
            channel_open_deadline: Duration::ZERO,
            dispatch_deadline: Duration::ZERO,
            sftp_operation_deadline: Duration::ZERO,
            overflow_cleanup_deadline: Duration::ZERO,
            ..ConnectionConfig::default()
        };
        let cloned = config.clone();
        assert_eq!(config, cloned);
        assert_eq!(cloned.host_operation_limit.get(), 1);
        assert_eq!(cloned.connect_deadline, Duration::ZERO);
    }

    #[test]
    fn boundary_byte_and_duration_values_are_representable() {
        // usize::MAX and Duration::MAX are valid, if extreme, configuration
        // — no hidden narrowing/overflow in the type choice itself.
        let config = ConnectionConfig {
            max_stdout_bytes: NonZeroUsize::new(usize::MAX).unwrap(),
            sftp_operation_deadline: Duration::MAX,
            ..ConnectionConfig::default()
        };
        assert_eq!(config.max_stdout_bytes.get(), usize::MAX);
        assert_eq!(config.sftp_operation_deadline, Duration::MAX);
    }
}
