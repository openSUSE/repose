//! Pure logic, traits, and command algorithms for `repose`.
//!
//! This crate must **not** depend on `russh` or `repose-ssh` (acyclic layering).
//!
//! # Module map (PR0.5)
//!
//! | Module | Role |
//! | --- | --- |
//! | [`error`] | [`error::SshError`] |
//! | [`types`] | Product/System/repos, exit aggregate, zypper codes |
//! | [`traits`] | `SshSession`, `Host`, `HostGroup`, `Probe` |
//! | [`mock`] | L2 `MockHost` / `MockHostGroup` |

#![forbid(unsafe_code)]

pub mod error;
pub mod mock;
pub mod traits;
pub mod types;

/// Crate version string (workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use error::SshError;
pub use traits::{last_out_succeeded, Host, HostGroup, Probe, SshSession};
pub use types::{
    zypper_exit_ok, ExitCode, OutEntry, Product, Repositories, Repository, System,
    ZYPPER_SUCCESS_EXIT_CODES,
};

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semverish() {
        assert!(!super::VERSION.is_empty());
        assert!(super::VERSION.contains('.'));
    }
}
