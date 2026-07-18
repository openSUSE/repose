//! `repose` binary — thin shim over the `repose_cli` library.

#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    repose_cli::run()
}
