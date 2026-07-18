//! Dev/packaging tool: regenerate the committed man pages and shell
//! completions from the `repose` CLI definition. CI runs this and diffs the
//! output so the assets never drift (mirrors the Python `repose-mangen` +
//! man-drift workflow). Built only with `--features gen`.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

fn main() -> std::process::ExitCode {
    let out = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("crates/repose-cli"), PathBuf::from);
    match generate(&out) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::from(2)
        }
    }
}

fn generate(out: &Path) -> std::io::Result<()> {
    let man_dir = out.join("man");
    let comp_dir = out.join("completions");
    std::fs::create_dir_all(&man_dir)?;
    std::fs::create_dir_all(&comp_dir)?;

    let mut cmd = repose_cli::command();
    cmd.build();
    render_man(&cmd, &man_dir)?;
    for shell in [
        clap_complete::Shell::Bash,
        clap_complete::Shell::Zsh,
        clap_complete::Shell::Fish,
    ] {
        clap_complete::generate_to(shell, &mut cmd, "repose", &comp_dir)?;
    }
    Ok(())
}

/// One man page for the top command plus one per visible subcommand
/// (`repose.1`, `repose-add.1`, …).
fn render_man(cmd: &clap::Command, dir: &Path) -> std::io::Result<()> {
    let mut buf = Vec::new();
    clap_mangen::Man::new(cmd.clone()).render(&mut buf)?;
    std::fs::write(dir.join(format!("{}.1", cmd.get_name())), buf)?;
    for sub in cmd.get_subcommands() {
        if sub.is_hide_set() {
            continue;
        }
        let sub_name = format!("{}-{}", cmd.get_name(), sub.get_name());
        let mut buf = Vec::new();
        // Title the page `repose-<sub>` so the .TH matches the file basename.
        // clap's `Command::name` wants `&'static str`; leak it (the generator is
        // a short-lived process).
        let titled: &'static str = Box::leak(sub_name.clone().into_boxed_str());
        clap_mangen::Man::new(sub.clone().name(titled)).render(&mut buf)?;
        std::fs::write(dir.join(format!("{sub_name}.1")), buf)?;
    }
    Ok(())
}
