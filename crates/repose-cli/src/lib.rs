//! Rust `repose` CLI — wired to command algorithms + russh HostGroup.

#![forbid(unsafe_code)]

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use repose_core::commands::{
    CommandOptions, default_probe, run_add, run_clear, run_install, run_known_products,
    run_list_products, run_list_repos, run_remove, run_reset, run_uninstall,
};
use repose_core::console::{ColorMode as CoreColorMode, Console, OutputFormat as CoreFormat};
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
enum Color {
    Auto,
    Always,
    Never,
}

impl From<Color> for CoreColorMode {
    fn from(c: Color) -> Self {
        match c {
            Color::Auto => CoreColorMode::Auto,
            Color::Always => CoreColorMode::Always,
            Color::Never => CoreColorMode::Never,
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
    /// print commands for host and exit
    #[arg(short = 'n', long = "print", global = true)]
    dry: bool,
    /// show program's version number and exit
    #[arg(short = 'V', long = "version", global = true)]
    version: bool,
    /// path to repose configuration
    #[arg(
        short = 'c',
        long = "config",
        global = true,
        default_value = "/etc/repose/products.yml"
    )]
    config: PathBuf,
    /// enable debug logging
    #[arg(short = 'd', long = "debug", global = true)]
    debug: bool,
    /// suppress messages from repose
    #[arg(short = 'q', long = "quiet", global = true)]
    quiet: bool,
    /// disable ANSI color in console output (alias for --color=never; honors NO_COLOR)
    #[arg(long = "no-color", global = true)]
    no_color: bool,
    /// console color mode: 'auto' (default; color on a TTY unless NO_COLOR), 'always', or 'never'
    #[arg(long = "color", global = true, value_enum, default_value_t = Color::Auto)]
    color: Color,
    /// console output format: 'text' (default) or 'json' (one event per line)
    #[arg(long = "format", global = true, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
    /// SSH host-key policy (OpenSSH semantics): 'yes' refuses unknown hosts; 'accept-new' (default) accepts unknown hosts on first contact but rejects changed keys; 'no'/'off' accepts both unknown and changed keys (pre-1.12 behaviour)
    #[arg(long = "strict-host-key-checking", global = true, value_enum, default_value_t = HostKeyMode::AcceptNew)]
    strict_host_key_checking: HostKeyMode,
    /// path to a custom known_hosts file (overrides ~/.ssh/known_hosts)
    #[arg(long = "known-hosts", global = true)]
    known_hosts: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// add specified repository to target
    Add {
        /// target to operate on
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        /// REPA pattern specification for needed repository
        #[arg(required = true)]
        repa: Vec<String>,
        /// seconds to wait per repository URL probe (default: 5)
        #[arg(long = "probe-timeout", default_value_t = 5.0, value_parser = parse_probe_timeout)]
        probe_timeout: f64,
        /// skip repository URL liveness probes
        #[arg(long = "no-probe", default_value_t = false)]
        no_probe: bool,
    },
    /// remove repository from target
    Remove {
        /// target to operate on
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        /// REPA pattern specification for needed repository
        #[arg(required = true)]
        repa: Vec<String>,
    },
    /// reset target repositories to only installed products repositories
    Reset {
        /// target to operate on
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        /// seconds to wait per repository URL probe (default: 5)
        #[arg(long = "probe-timeout", default_value_t = 5.0, value_parser = parse_probe_timeout)]
        probe_timeout: f64,
        /// skip repository URL liveness probes
        #[arg(long = "no-probe", default_value_t = false)]
        no_probe: bool,
    },
    /// add specified repository to target and install product
    Install {
        /// target to operate on
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        /// REPA pattern specification for needed repository
        #[arg(required = true)]
        repa: Vec<String>,
        /// seconds to wait per repository URL probe (default: 5)
        #[arg(long = "probe-timeout", default_value_t = 5.0, value_parser = parse_probe_timeout)]
        probe_timeout: f64,
        /// skip repository URL liveness probes
        #[arg(long = "no-probe", default_value_t = false)]
        no_probe: bool,
        /// on transactional hosts (SL Micro), stage the package change but do not reboot/reconnect/verify (default: reboot)
        #[arg(long = "no-reboot", default_value_t = false)]
        no_reboot: bool,
    },
    /// clear all repositories from target
    Clear {
        /// target to operate on
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
    },
    /// remove specified repository from target and uninstall product
    Uninstall {
        /// target to operate on
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        /// REPA pattern specification for needed repository
        #[arg(required = true)]
        repa: Vec<String>,
        /// on transactional hosts (SL Micro), stage the package change but do not reboot/reconnect/verify (default: reboot)
        #[arg(long = "no-reboot", default_value_t = false)]
        no_reboot: bool,
    },
    /// list products on target
    #[command(name = "list-products")]
    ListProducts {
        /// target to operate on
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        /// Generate YAML host spec for refhosts.yml generator without normalization. Default for SLE 12-SP5 and SLE 15-SP3+ products
        #[arg(long = "yaml", default_value_t = false)]
        yaml: bool,
    },
    /// list repositories on target
    #[command(name = "list-repos")]
    ListRepos {
        /// target to operate on
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
    },
    /// list known products by 'repose'
    #[command(name = "known-products")]
    KnownProducts,
}

/// Validate `--probe-timeout`: a finite, non-negative number of seconds with a
/// sane upper bound (one day). Rejects `-1`, `NaN`, `inf`, and absurd values
/// that would otherwise panic in [`Duration::from_secs_f64`].
fn parse_probe_timeout(s: &str) -> Result<f64, String> {
    let secs: f64 = s
        .parse()
        .map_err(|_| format!("'{s}' is not a number of seconds"))?;
    if !secs.is_finite() {
        return Err(format!("'{s}' is not a finite number of seconds"));
    }
    if secs < 0.0 {
        return Err(format!("'{s}' is negative; timeout must be >= 0 seconds"));
    }
    if secs > 86400.0 {
        return Err(format!("'{s}' exceeds the maximum of 86400 seconds"));
    }
    Ok(secs)
}

/// Run the `repose` CLI (entry point behind the thin `main.rs` shim).
pub fn run() -> ExitCode {
    // I/O-bound concurrency (join_all fan-out) needs no thread pool; blocking
    // work (password prompts, known_hosts parsing) runs via spawn_blocking.
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
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

/// Build the [`ConnectionConfig`] for one CLI invocation.
///
/// P1 resource limits/deadlines are internal policy, not CLI flags (see
/// `tests/performance/p1-limit-decision.md`) — every field left unset here
/// takes the reviewed `ConnectionConfig::default()` value.
fn connection_config(cli: &Cli) -> ConnectionConfig {
    ConnectionConfig {
        host_key_policy: cli.strict_host_key_checking.into(),
        known_hosts: cli.known_hosts.clone(),
        timeout: 120.0,
        ..ConnectionConfig::default()
    }
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

    let conn = connection_config(&cli);

    let cmd = match cli.command.take() {
        None => {
            let mut app = Cli::command();
            let _ = app.print_help();
            println!();
            return ExitCode::SUCCESS;
        }
        Some(Commands::KnownProducts) => {
            let color = resolve_color(cli.no_color, cli.color, std::io::stdout().is_terminal());
            return known_products(&cli.config, cli.format.into(), color);
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
const fn log_level(debug: bool, quiet: bool) -> Level {
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

/// Resolve the effective color decision for stdout `list-*` output, mirroring
/// `repose_core::console::Console::use_color`: `--no-color`/`--color=never`
/// force off, `--color=always` forces on, and `auto` honors `NO_COLOR`, then
/// `COLOR`, then `is_tty`.
fn resolve_color(no_color: bool, color: Color, is_tty: bool) -> bool {
    if no_color {
        return false;
    }
    match color {
        Color::Always => true,
        Color::Never => false,
        Color::Auto => {
            if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
                return false;
            }
            if let Ok(c) = std::env::var("COLOR") {
                if c.eq_ignore_ascii_case("never") {
                    return false;
                }
                if c.eq_ignore_ascii_case("always") {
                    return true;
                }
            }
            is_tty
        }
    }
}

fn known_products(config: &Path, format: CoreFormat, color: bool) -> ExitCode {
    match run_known_products(config, format, color, &mut std::io::stdout()) {
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
        // Intercepted before dispatch (it needs no SSH targets); reaching it
        // here is a wiring bug — fail like an empty target list, not a panic.
        Commands::KnownProducts => (vec![], vec![], 5.0, false, false),
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
        yaml: matches!(&cmd, Commands::ListProducts { yaml: true, .. }),
        color: resolve_color(cli.no_color, cli.color, std::io::stdout().is_terminal()),
        probe_concurrency_limit: conn.probe_concurrency_limit,
    };

    let mut group = RusshHostGroup::from_targets(specs, conn);
    let probe = default_probe();
    let is_tty = std::io::stdout().is_terminal();
    let mut console = Console::new(std::io::stdout());
    console.format = opts.format;
    // Auto mode reflects the real terminal; force_color is the injected TTY bit.
    console.force_color = Some(is_tty);
    console.color = if cli.no_color {
        // --no-color is an alias for --color=never and wins over --color.
        CoreColorMode::Never
    } else {
        cli.color.into()
    };

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
        // Intercepted before dispatch; treated as a no-op failure rather than
        // a panic if a refactor ever routes it here.
        Commands::KnownProducts => {
            eprintln!("error: known-products does not take targets");
            repose_core::types::ExitCode::AllFailed
        }
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

    #[test]
    fn probe_timeout_accepts_sane_values() {
        assert_eq!(parse_probe_timeout("5"), Ok(5.0));
        assert_eq!(parse_probe_timeout("0"), Ok(0.0));
        assert_eq!(parse_probe_timeout("0.5"), Ok(0.5));
        assert_eq!(parse_probe_timeout("86400"), Ok(86400.0));
    }

    #[test]
    fn probe_timeout_rejects_panicking_values() {
        // Each of these would panic (or hang forever) in Duration::from_secs_f64.
        assert!(parse_probe_timeout("-1").is_err());
        assert!(parse_probe_timeout("NaN").is_err());
        assert!(parse_probe_timeout("inf").is_err());
        assert!(parse_probe_timeout("1e300").is_err());
        assert!(parse_probe_timeout("bogus").is_err());
    }

    #[test]
    fn probe_timeout_parse_error_exits_with_usage_code() {
        let err = Cli::try_parse_from(["repose", "add", "-t", "h", "--probe-timeout=-1", "R"])
            .unwrap_err();
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn connection_config_carries_the_approved_p1_defaults() {
        // No CLI flags exist for the P1 resource limits/deadlines (they
        // are internal policy — see tests/performance/p1-limit-decision.md);
        // every real invocation must get exactly the reviewed defaults.
        let cli = Cli::try_parse_from(["repose", "add", "-t", "h", "R"]).unwrap();
        let conn = connection_config(&cli);
        let defaults = ConnectionConfig::default();
        assert_eq!(conn.host_operation_limit, defaults.host_operation_limit);
        assert_eq!(
            conn.probe_concurrency_limit,
            defaults.probe_concurrency_limit
        );
        assert_eq!(
            conn.sftp_read_concurrency_limit,
            defaults.sftp_read_concurrency_limit
        );
        assert_eq!(conn.max_products_d_entries, defaults.max_products_d_entries);
        assert_eq!(conn.max_sftp_file_bytes, defaults.max_sftp_file_bytes);
        assert_eq!(conn.max_stdout_bytes, defaults.max_stdout_bytes);
        assert_eq!(conn.max_stderr_bytes, defaults.max_stderr_bytes);
        assert_eq!(conn.connect_deadline, defaults.connect_deadline);
        assert_eq!(conn.auth_deadline, defaults.auth_deadline);
        assert_eq!(conn.channel_open_deadline, defaults.channel_open_deadline);
        assert_eq!(conn.dispatch_deadline, defaults.dispatch_deadline);
        assert_eq!(
            conn.sftp_operation_deadline,
            defaults.sftp_operation_deadline
        );
        assert_eq!(
            conn.overflow_cleanup_deadline,
            defaults.overflow_cleanup_deadline
        );
        // CLI-set fields still come from flags/defaults as before.
        assert_eq!(conn.host_key_policy, HostKeyPolicy::AcceptNew);
        assert_eq!(conn.timeout, 120.0);
    }
}
