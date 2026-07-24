//! Errors shared across traits (no russh types).

use thiserror::Error;

/// Transport / session errors for [`crate::traits::SshSession`] and
/// [`crate::traits::Host`].
///
/// Remote command **exit codes** are not errors — they live in the host
/// `out` history (see the `Host::run` contract). For command execution,
/// session-layer failures (timeouts, transport, setup) are recorded as
/// `out` entries with `exitcode == -1`, and only the never-dispatched case
/// ([`SshError::NotConnected`]) is propagated.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SshError {
    /// Host is not connected, so the command was never dispatched; a
    /// synthetic `out` entry with `exitcode == -1` is still recorded.
    #[error("not connected: {0}")]
    NotConnected(String),

    /// Channel / session setup or teardown failed.
    #[error("transport error: {0}")]
    Transport(String),

    /// Generic failure (e.g. discovery/read failures); may follow recorded
    /// `out` entries.
    #[error("{0}")]
    Other(String),
}
