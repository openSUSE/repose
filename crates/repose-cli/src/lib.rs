//! Rust `repose` CLI — wired to command algorithms + russh HostGroup.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use repose_core::commands::{
    default_probe, run_add, run_clear, run_install, run_known_products, run_list_products,
    run_list_repos, run_remove, run_reset, run_uninstall, CommandOptions,
};
use repose_core::console::{Console, OutputFormat as CoreFormat};
use repose_core::host_parse::parse_host;
use repose_core::repa::Repa;
use repose_core::{ConnectionConfig, HostKeyPolicy, VERSION};
use repose_ssh::RusshHostGroup;
use tracing::Level;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

impl From<OutputFormat> for CoreFormat {
    fn from(f: OutputFormat) -> Self {
        match f {
            OutputFormat::Text => CoreFormat::Text,
            OutputFormat::Json => CoreFormat::Json,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum HostKeyMode {
    Yes,
    #[value(name = "accept-new")]
    AcceptNew,
    No,
    Off,
}

impl From<HostKeyMode> for HostKeyPolicy {
    fn from(m: HostKeyMode) -> Self {
        match m {
            HostKeyMode::Yes => HostKeyPolicy::Yes,
            HostKeyMode::AcceptNew => HostKeyPolicy::AcceptNew,
            HostKeyMode::No => HostKeyPolicy::No,
            HostKeyMode::Off => HostKeyPolicy::Off,
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "repose",
    version = VERSION,
    about = "Repository manipulation tool for QAM",
    disable_version_flag = true,
    arg_required_else_help = false
)]
struct Cli {
    #[arg(short = 'n', long = "print", global = true)]
    dry: bool,
    #[arg(short = 'V', long = "version", global = true)]
    version: bool,
    #[arg(
        short = 'c',
        long = "config",
        global = true,
        default_value = "/etc/repose/products.yml"
    )]
    config: PathBuf,
    #[arg(short = 'd', long = "debug", global = true)]
    debug: bool,
    #[arg(short = 'q', long = "quiet", global = true)]
    quiet: bool,
    #[arg(long = "no-color", global = true)]
    no_color: bool,
    #[arg(long = "format", global = true, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
    #[arg(long = "strict-host-key-checking", global = true, value_enum, default_value_t = HostKeyMode::AcceptNew)]
    strict_host_key_checking: HostKeyMode,
    #[arg(long = "known-hosts", global = true)]
    known_hosts: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Add {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        #[arg(required = true)]
        repa: Vec<String>,
        #[arg(long = "probe-timeout", default_value_t = 5.0)]
        probe_timeout: f64,
        #[arg(long = "no-probe", default_value_t = false)]
        no_probe: bool,
    },
    Remove {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        #[arg(required = true)]
        repa: Vec<String>,
    },
    Reset {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        #[arg(long = "probe-timeout", default_value_t = 5.0)]
        probe_timeout: f64,
        #[arg(long = "no-probe", default_value_t = false)]
        no_probe: bool,
    },
    Install {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        #[arg(required = true)]
        repa: Vec<String>,
        #[arg(long = "probe-timeout", default_value_t = 5.0)]
        probe_timeout: f64,
        #[arg(long = "no-probe", default_value_t = false)]
        no_probe: bool,
        #[arg(long = "no-reboot", default_value_t = false)]
        no_reboot: bool,
    },
    Clear {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
    },
    Uninstall {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        #[arg(required = true)]
        repa: Vec<String>,
        #[arg(long = "no-reboot", default_value_t = false)]
        no_reboot: bool,
    },
    #[command(name = "list-products")]
    ListProducts {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        #[arg(long = "yaml", default_value_t = false)]
        yaml: bool,
    },
    #[command(name = "list-repos")]
    ListRepos {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
    },
    #[command(name = "known-products")]
    KnownProducts,
}

/// Run the `repose` CLI (entry point behind the thin `main.rs` shim).
pub fn run() -> ExitCode {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: tokio runtime: {e}");
            return ExitCode::from(2);
        }
    };
    rt.block_on(async_main())
}

/// The clap [`clap::Command`] for the CLI, used by the `repose-gen` binary to
/// render man pages and shell completions.
#[must_use]
pub fn command() -> clap::Command {
    Cli::command()
}

async fn async_main() -> ExitCode {
    let mut cli = Cli::parse();
    if cli.version {
        println!("repose version: {VERSION}");
        return ExitCode::SUCCESS;
    }
    if cli.debug && cli.quiet {
        eprintln!("error: --debug and --quiet are mutually exclusive");
        return ExitCode::from(2);
    }
    init_logging(cli.debug, cli.quiet, cli.no_color);

    let conn = ConnectionConfig {
        host_key_policy: cli.strict_host_key_checking.into(),
        known_hosts: cli.known_hosts.clone(),
        timeout: 120.0,
    };

    let cmd = match cli.command.take() {
        None => {
            let mut app = Cli::command();
            let _ = app.print_help();
            println!();
            return ExitCode::SUCCESS;
        }
        Some(Commands::KnownProducts) => {
            return known_products(&cli.config);
        }
        Some(other) => other,
    };

    // SIGINT → 130 (Python KeyboardInterrupt path). Racing the command future
    // means Ctrl-C drops the in-flight SSH work and returns immediately.
    tokio::select! {
        code = dispatch(cli, conn, cmd) => code,
        _ = tokio::signal::ctrl_c() => {
            tracing::error!("interrupted");
            ExitCode::from(130)
        }
    }
}

/// Map `-d`/`-q` to a tracing level (default INFO), matching Python
/// `create_logger` (INFO) + `-d`→DEBUG / `-q`→WARNING.
fn log_level(debug: bool, quiet: bool) -> Level {
    if debug {
        Level::DEBUG
    } else if quiet {
        Level::WARN
    } else {
        Level::INFO
    }
}

/// Install the stderr tracing subscriber (also captures repose-ssh `log`
/// events via the tracing-log bridge). ANSI is disabled for `--no-color`
/// or a non-empty `NO_COLOR`, matching Python's `log_no_color`.
fn init_logging(debug: bool, quiet: bool, no_color: bool) {
    let ansi = !no_color && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty());
    let _ = tracing_subscriber::fmt()
        .with_max_level(log_level(debug, quiet))
        .with_writer(std::io::stderr)
        .with_target(false)
        .without_time()
        .with_ansi(ansi)
        .try_init();
}

fn known_products(config: &Path) -> ExitCode {
    match run_known_products(config, &mut std::io::stdout()) {
        Ok(c) => exit_from(c),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

async fn dispatch(cli: Cli, conn: ConnectionConfig, cmd: Commands) -> ExitCode {
    let (targets, repa, probe_timeout, no_probe, no_reboot) = match &cmd {
        Commands::Add {
            targets,
            repa,
            probe_timeout,
            no_probe,
        } => (
            targets.clone(),
            repa.clone(),
            *probe_timeout,
            *no_probe,
            false,
        ),
        Commands::Remove { targets, repa } => (targets.clone(), repa.clone(), 5.0, false, false),
        Commands::Reset {
            targets,
            probe_timeout,
            no_probe,
        } => (targets.clone(), vec![], *probe_timeout, *no_probe, false),
        Commands::Install {
            targets,
            repa,
            probe_timeout,
            no_probe,
            no_reboot,
        } => (
            targets.clone(),
            repa.clone(),
            *probe_timeout,
            *no_probe,
            *no_reboot,
        ),
        Commands::Clear { targets } => (targets.clone(), vec![], 5.0, false, false),
        Commands::Uninstall {
            targets,
            repa,
            no_reboot,
        } => (targets.clone(), repa.clone(), 5.0, false, *no_reboot),
        Commands::ListProducts { targets, .. } => (targets.clone(), vec![], 5.0, false, false),
        Commands::ListRepos { targets } => (targets.clone(), vec![], 5.0, false, false),
        Commands::KnownProducts => unreachable!(),
    };

    let mut specs = Vec::new();
    for t in &targets {
        match parse_host(t) {
            Ok(s) => specs.push(s),
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        }
    }
    let mut repas = Vec::new();
    for r in &repa {
        match Repa::parse(r) {
            Ok(p) => repas.push(p),
            Err(e) => {
                eprintln!("error: invalid REPA {r:?}: {e}");
                return ExitCode::from(2);
            }
        }
    }

    let opts = CommandOptions {
        dry: cli.dry,
        config: cli.config.clone(),
        repa: repas,
        probe_timeout: Duration::from_secs_f64(probe_timeout),
        no_probe,
        no_reboot,
        format: cli.format.into(),
    };

    let mut group = RusshHostGroup::from_targets(specs, conn);
    let probe = default_probe();
    let mut console = Console::new(std::io::stdout());
    console.format = opts.format;
    if cli.no_color {
        console.color = repose_core::console::ColorMode::Never;
    }

    let code = match cmd {
        Commands::Add { .. } => match run_add(&opts, &mut group, &probe, &mut console).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        },
        Commands::Remove { .. } => run_remove(&opts, &mut group, &mut console).await,
        Commands::Reset { .. } => match run_reset(&opts, &mut group, &probe, &mut console).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        },
        Commands::Install { .. } => {
            match run_install(&opts, &mut group, &probe, &mut console).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            }
        }
        Commands::Clear { .. } => run_clear(&opts, &mut group, &mut console).await,
        Commands::Uninstall { .. } => run_uninstall(&opts, &mut group, &mut console).await,
        Commands::ListProducts { .. } => {
            run_list_products(&opts, &mut group, &mut std::io::stdout()).await
        }
        Commands::ListRepos { .. } => {
            run_list_repos(&opts, &mut group, &mut std::io::stdout()).await
        }
        Commands::KnownProducts => unreachable!(),
    };
    exit_from(code)
}

fn exit_from(c: repose_core::ExitCode) -> ExitCode {
    ExitCode::from(c.as_i32() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_maps_flags() {
        assert_eq!(log_level(false, false), Level::INFO);
        assert_eq!(log_level(true, false), Level::DEBUG);
        assert_eq!(log_level(false, true), Level::WARN);
        // -d wins if both slip through (mutex is enforced separately).
        assert_eq!(log_level(true, true), Level::DEBUG);
    }
}
