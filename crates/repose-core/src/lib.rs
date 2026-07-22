//! Pure logic, traits, and command algorithms for `repose`.
//!
//! This crate must **not** depend on `russh` or `repose-ssh` (acyclic layering).

#![forbid(unsafe_code)]

pub mod commands;
pub mod config;
pub mod console;
pub mod display;
pub mod error;
pub mod host_parse;
pub mod mock;
pub mod probe;
pub mod product_parse;
pub mod repa;
pub mod repo_parse;
pub mod repoq;
pub mod shell;
pub mod template;
pub mod traits;
pub mod transform;
pub mod types;

/// Crate version string (workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use config::{ConnectionConfig, HostKeyPolicy};
pub use error::{OutputStream, SshError, TimeoutPhase};
pub use host_parse::{HostParseError, HostSpec, parse_host};
pub use repa::{Repa, RepaError};
pub use shell::{join as shell_join, quote as shell_quote};
pub use traits::{Host, HostGroup, Probe, SshSession, last_out_succeeded};
pub use types::{
    ExitCode, OutEntry, Product, Repositories, Repository, System, ZYPPER_SUCCESS_EXIT_CODES,
    zypper_exit_ok,
};

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semverish() {
        assert!(!super::VERSION.is_empty());
        assert!(super::VERSION.contains('.'));
    }
}
