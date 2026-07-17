//! SSH transport and host implementations for `repose`.
//!
//! Depends on [`repose_core`] only. Must not be depended upon by `repose-core`.

#![forbid(unsafe_code)]

use repose_core::VERSION as CORE_VERSION;

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
}
