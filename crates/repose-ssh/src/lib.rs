//! SSH transport and host implementations for `repose`.
//!
//! Depends on [`repose_core`] only. Must not be depended upon by `repose-core`.
//!
//! # PR0.5 status
//!
//! Trait surface and layering are established. Full `russh` session / host
//! implementations land in later PRs (PR8–PR10). This crate currently
//! re-exports core traits for ergonomic `repose_ssh::Host` use and exposes
//! a placeholder module for the future backend.

#![forbid(unsafe_code)]

pub use repose_core::error::SshError;
pub use repose_core::traits::{Host, HostGroup, Probe, SshSession};
pub use repose_core::types::{OutEntry, Product, Repositories, Repository, System};
pub use repose_core::VERSION as CORE_VERSION;

/// Future russh-backed types (PR8+). Kept as a module so the crate graph
/// and public layout are stable while the spike has no network I/O.
///
/// Placeholder for `RusshSession` / `RusshHost` / `RusshHostGroup`.
pub mod russh_backend {
    /// Marker proving this module compiles without pulling `russh` yet.
    #[must_use]
    pub const fn backend_name() -> &'static str {
        "russh" // chosen single backend; not linked until PR8
    }
}

/// Confirms the workspace link to `repose-core` at compile time.
#[must_use]
pub fn core_version() -> &'static str {
    CORE_VERSION
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_core() {
        assert_eq!(super::core_version(), repose_core::VERSION);
    }

    #[test]
    fn single_backend_name() {
        assert_eq!(super::russh_backend::backend_name(), "russh");
    }

    /// Document the intended dependency direction for reviewers/CI.
    #[test]
    fn layering_doc() {
        // repose-ssh → repose-core is the only allowed edge.
        // The inverse is forbidden and checked by CI (`scripts/check-rust-layering.sh`).
        let _ = super::core_version();
    }
}
