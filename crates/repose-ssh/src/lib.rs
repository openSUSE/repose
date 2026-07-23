//! SSH transport and host implementations for `repose` (russh only).

#![forbid(unsafe_code)]

mod glob;
pub mod host;
mod hostkey;
mod openssh_config;
pub mod session;

pub use host::{RusshHost, RusshHostGroup};
pub use session::RusshSession;

pub use repose_core::VERSION as CORE_VERSION;
pub use repose_core::error::SshError;
pub use repose_core::traits::{Host, HostGroup, Probe, SshSession};

/// Confirms the workspace link to `repose-core` at compile time.
#[cfg(test)]
#[must_use]
const fn core_version() -> &'static str {
    CORE_VERSION
}

/// Chosen single backend name.
#[cfg(test)]
#[must_use]
const fn backend_name() -> &'static str {
    "russh"
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(super::core_version(), repose_core::VERSION);
    }

    #[test]
    fn single_backend_name() {
        assert_eq!(super::backend_name(), "russh");
    }
}
