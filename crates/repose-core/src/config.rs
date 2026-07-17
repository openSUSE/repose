//! Transport configuration (Python `ConnectionConfig` without `ssh_backend`).

use std::path::PathBuf;

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
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionConfig {
    pub host_key_policy: HostKeyPolicy,
    pub known_hosts: Option<PathBuf>,
    /// Per-command SSH timeout in seconds (Python default 120).
    pub timeout: f64,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            host_key_policy: HostKeyPolicy::AcceptNew,
            known_hosts: None,
            timeout: 120.0,
        }
    }
}
