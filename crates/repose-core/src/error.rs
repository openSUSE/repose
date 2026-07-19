//! Errors shared across traits (no russh types).

use thiserror::Error;

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
}
