//! Entry point for the Rust `repose` binary.
//!
//! PR0 scaffold only: prints the version line and exits. Full CLI lands later.

#![forbid(unsafe_code)]

fn main() {
    // Match Python shape: `repose version: {version}` (see design Key Decisions).
    // Full clap surface is PR15a+; this keeps the binary name and version contract.
    println!("repose version: {}", repose_core::VERSION);
    // Touch repose-ssh so the workspace edge is not dead-code eliminated from the graph
    // in a way that would let us forget the dependency (layering: cli → ssh → core).
    let _ = repose_ssh::core_version();
}
