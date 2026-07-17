//! Minimal domain types referenced by Host traits.
//!
//! Full parsers and REPA land in later PRs; these stubs keep traits
//! compilable and mockable without pulling XML/YAML deps into PR0.5.

use std::collections::BTreeMap;

/// Installed or template product identity (Python `Product`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Product {
    pub name: String,
    pub version: String,
    pub arch: String,
}

/// Host system model (Python `System`): base + addons + transactional flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct System {
    pub base: Product,
    pub addons: Vec<Product>,
    pub transactional: bool,
}

impl System {
    #[must_use]
    pub fn is_transactional(&self) -> bool {
        self.transactional
    }

    #[must_use]
    pub fn arch(&self) -> &str {
        &self.base.arch
    }

    #[must_use]
    pub fn get_base(&self) -> &Product {
        &self.base
    }
}

/// One zypper repository row (Python `Repository`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    pub alias: String,
    pub name: String,
    pub url: String,
    /// `enabled` from zypper XML (`"1"` → true).
    pub state: bool,
}

/// Alias → optional product parse (Python `Repositories`).
///
/// Values are `None` when the repo name is not a four-part product string.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Repositories {
    inner: BTreeMap<String, Option<Product>>,
}

impl Repositories {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, alias: String, product: Option<Product>) {
        self.inner.insert(alias, product);
    }

    #[must_use]
    pub fn get(&self, alias: &str) -> Option<&Option<Product>> {
        self.inner.get(alias)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.inner.keys()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// One remote command history entry (Python `Target.out` row).
///
/// Fields: `(command, stdout, stderr, exitcode, runtime_secs)`.
/// Timeout / missing status → `exitcode == -1`.
pub type OutEntry = (String, String, String, i32, u64);

/// Process-level aggregate exit codes (Python `ExitCode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    /// All hosts succeeded (or zero hosts).
    Ok = 0,
    /// Partial failure.
    Partial = 1,
    /// All hosts failed (including 1-of-1).
    AllFailed = 2,
}

impl ExitCode {
    /// Aggregate bool worker results: true = success.
    ///
    /// Empty → Ok. All true → Ok. All false → AllFailed. Mixed → Partial.
    #[must_use]
    pub fn aggregate(results: impl IntoIterator<Item = bool>) -> Self {
        let mut total = 0usize;
        let mut failed = 0usize;
        for ok in results {
            total += 1;
            if !ok {
                failed += 1;
            }
        }
        if total == 0 || failed == 0 {
            Self::Ok
        } else if failed == total {
            Self::AllFailed
        } else {
            Self::Partial
        }
    }

    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

/// zypper exit codes treated as success by `_report_target`
/// (`ZYPPER_SUCCESS_EXIT_CODES` in Python).
pub const ZYPPER_SUCCESS_EXIT_CODES: &[i32] = &[0, 100, 101, 102, 103, 106, 107];

/// Whether a zypper-style exit code is a successful host result.
#[must_use]
pub fn zypper_exit_ok(code: i32) -> bool {
    ZYPPER_SUCCESS_EXIT_CODES.contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_empty_is_ok() {
        assert_eq!(ExitCode::aggregate([]), ExitCode::Ok);
    }

    #[test]
    fn aggregate_all_ok() {
        assert_eq!(ExitCode::aggregate([true, true]), ExitCode::Ok);
    }

    #[test]
    fn aggregate_all_fail() {
        assert_eq!(ExitCode::aggregate([false]), ExitCode::AllFailed);
        assert_eq!(ExitCode::aggregate([false, false]), ExitCode::AllFailed);
    }

    #[test]
    fn aggregate_partial() {
        assert_eq!(ExitCode::aggregate([true, false]), ExitCode::Partial);
    }

    #[test]
    fn zypper_success_set() {
        assert!(zypper_exit_ok(0));
        assert!(zypper_exit_ok(102));
        assert!(!zypper_exit_ok(4));
        assert!(!zypper_exit_ok(-1));
    }
}
