//! Pure logic, traits, and command algorithms for `repose`.
//!
//! This crate must **not** depend on `russh` or `repose-ssh` (acyclic layering).

#![forbid(unsafe_code)]

/// Crate version string (workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semverish() {
        assert!(!super::VERSION.is_empty());
        assert!(super::VERSION.contains('.'));
    }
}
