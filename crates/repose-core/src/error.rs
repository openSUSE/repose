//! Errors shared across traits (no russh types).

use std::time::Duration;

use thiserror::Error;

/// Which SSH lifecycle phase a [`SshError::Timeout`] exceeded its
/// configured deadline in (see `ConnectionConfig`'s `*_deadline` fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutPhase {
    /// DNS/TCP/proxy connect + SSH handshake.
    Connect,
    /// Agent/public-key/password authentication network round trip
    /// (excludes user-paced interactive passphrase/password entry).
    Authentication,
    /// Channel open.
    ChannelOpen,
    /// Exec dispatch or SFTP subsystem request.
    Dispatch,
    /// Remote command completion.
    Command,
    /// One SFTP read/listdir/readlink operation.
    SftpOperation,
}

impl TimeoutPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Connect => "connect",
            Self::Authentication => "authentication",
            Self::ChannelOpen => "channel open",
            Self::Dispatch => "dispatch",
            Self::Command => "command",
            Self::SftpOperation => "SFTP operation",
        }
    }
}

impl std::fmt::Display for TimeoutPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which command output stream a [`SshError::OutputTooLarge`] overflowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

impl OutputStream {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

impl std::fmt::Display for OutputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Transport / session errors for [`crate::traits::SshSession`] and
/// [`crate::traits::Host`].
///
/// Remote command **exit codes** are not errors — they live in the host
/// `out` history (see `Host::run` contract). Use this type only for
/// pre-append transport failures or unrecoverable session problems.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SshError {
    /// Host is not connected and no attempt produced an `out` entry.
    #[error("not connected: {0}")]
    NotConnected(String),

    /// Channel / session setup failed before any command history entry.
    #[error("transport error: {0}")]
    Transport(String),

    /// Generic failure without an `out` entry.
    #[error("{0}")]
    Other(String),

    /// A configured phase deadline (`ConnectionConfig`'s `*_deadline`
    /// fields) elapsed before the operation completed.
    #[error("{phase} timed out after {deadline:?}")]
    Timeout {
        phase: TimeoutPhase,
        deadline: Duration,
    },

    /// Command output on `stream` exceeded the configured byte limit.
    /// Never carries the oversized payload itself.
    #[error("{stream} exceeded the {limit}-byte limit")]
    OutputTooLarge { stream: OutputStream, limit: usize },

    /// A remote file exceeded the configured SFTP read byte limit. Never
    /// carries the oversized payload itself.
    #[error("remote file {path} exceeded the {limit}-byte limit")]
    SftpFileTooLarge { path: String, limit: usize },

    /// A directory listing exceeded the configured plausible-entry cap.
    #[error("directory {path} has {observed} entries, exceeding the {limit}-entry limit")]
    DirectoryTooLarge {
        path: String,
        limit: usize,
        observed: usize,
    },

    /// `accept-new` could not durably persist a first-contact host key;
    /// the session must be rejected (fail-closed) rather than trusted
    /// without a recorded pin.
    #[error("could not persist host key for {host} to {path}: {reason}")]
    KnownHostsPersistFailed {
        host: String,
        path: String,
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_formatting_identifies_phase_and_deadline() {
        let err = SshError::Timeout {
            phase: TimeoutPhase::Connect,
            deadline: Duration::from_secs(30),
        };
        assert_eq!(err.to_string(), "connect timed out after 30s");
    }

    #[test]
    fn stdout_overflow_formatting_identifies_stream_and_limit() {
        let err = SshError::OutputTooLarge {
            stream: OutputStream::Stdout,
            limit: 262_144,
        };
        assert_eq!(err.to_string(), "stdout exceeded the 262144-byte limit");
    }

    #[test]
    fn stderr_overflow_formatting_identifies_stream_and_limit() {
        let err = SshError::OutputTooLarge {
            stream: OutputStream::Stderr,
            limit: 262_144,
        };
        assert_eq!(err.to_string(), "stderr exceeded the 262144-byte limit");
    }

    #[test]
    fn sftp_file_overflow_formatting_identifies_path_and_limit() {
        let err = SshError::SftpFileTooLarge {
            path: "/etc/products.d/SLES.prod".into(),
            limit: 65536,
        };
        assert_eq!(
            err.to_string(),
            "remote file /etc/products.d/SLES.prod exceeded the 65536-byte limit"
        );
    }

    #[test]
    fn directory_overflow_formatting_identifies_path_limit_and_observed_count() {
        let err = SshError::DirectoryTooLarge {
            path: "/etc/products.d".into(),
            limit: 256,
            observed: 257,
        };
        assert_eq!(
            err.to_string(),
            "directory /etc/products.d has 257 entries, exceeding the 256-entry limit"
        );
    }

    #[test]
    fn known_hosts_persist_failure_formatting_identifies_host_path_and_reason() {
        let err = SshError::KnownHostsPersistFailed {
            host: "example.com".into(),
            path: "/home/user/.ssh/known_hosts".into(),
            reason: "permission denied".into(),
        };
        assert_eq!(
            err.to_string(),
            "could not persist host key for example.com to /home/user/.ssh/known_hosts: permission denied"
        );
    }
}
