//! Rust `repose` CLI (PR15a: surface stubs; full wire-up later).
//!
//! Binary name is `repose`. No `--ssh-backend` (single SSH stack).

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use repose_core::{parse_host, template, ConnectionConfig, HostKeyPolicy, VERSION};

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, ValueEnum)]
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
    long_about = None,
    disable_version_flag = true,
    arg_required_else_help = false,
)]
struct Cli {
    /// Print commands for host and exit
    #[arg(short = 'n', long = "print", global = true)]
    dry: bool,

    /// Show program version
    #[arg(short = 'V', long = "version", global = true)]
    version: bool,

    /// Path to repose configuration
    #[arg(
        short = 'c',
        long = "config",
        global = true,
        default_value = "/etc/repose/products.yml"
    )]
    config: PathBuf,

    /// Enable debug logging
    #[arg(short = 'd', long = "debug", global = true)]
    debug: bool,

    /// Suppress messages from repose
    #[arg(short = 'q', long = "quiet", global = true)]
    quiet: bool,

    /// Disable ANSI color
    #[arg(long = "no-color", global = true)]
    no_color: bool,

    /// Console output format
    #[arg(long = "format", global = true, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,

    /// Strict host key checking
    #[arg(
        long = "strict-host-key-checking",
        global = true,
        value_enum,
        default_value_t = HostKeyMode::AcceptNew
    )]
    strict_host_key_checking: HostKeyMode,

    /// Custom known_hosts path
    #[arg(long = "known-hosts", global = true)]
    known_hosts: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Add repository to target
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
    /// Remove repository from target
    Remove {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        #[arg(required = true)]
        repa: Vec<String>,
    },
    /// Reset repos to installed products' repos
    Reset {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        #[arg(long = "probe-timeout", default_value_t = 5.0)]
        probe_timeout: f64,
        #[arg(long = "no-probe", default_value_t = false)]
        no_probe: bool,
    },
    /// Add repos and install product
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
    /// Clear all repos
    Clear {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
    },
    /// Remove repos and uninstall product
    Uninstall {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        #[arg(required = true)]
        repa: Vec<String>,
        #[arg(long = "no-reboot", default_value_t = false)]
        no_reboot: bool,
    },
    /// List products on target
    #[command(name = "list-products")]
    ListProducts {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
        #[arg(long = "yaml", default_value_t = false)]
        yaml: bool,
    },
    /// List repositories on target
    #[command(name = "list-repos")]
    ListRepos {
        #[arg(short = 't', long = "target", required = true)]
        targets: Vec<String>,
    },
    /// List products known to config (no SSH)
    #[command(name = "known-products")]
    KnownProducts,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.version {
        println!("repose version: {VERSION}");
        return ExitCode::SUCCESS;
    }

    if cli.debug && cli.quiet {
        eprintln!("error: --debug and --quiet are mutually exclusive");
        return ExitCode::from(2);
    }

    // Build connection config early (validates host-key mode).
    let _conn = ConnectionConfig {
        host_key_policy: cli.strict_host_key_checking.into(),
        known_hosts: cli.known_hosts.clone(),
        timeout: 120.0,
    };

    let Some(cmd) = cli.command else {
        // Bare `repose` → help, exit 0 (Python parity).
        let mut app = Cli::command();
        let _ = app.print_help();
        println!();
        return ExitCode::SUCCESS;
    };

    match cmd {
        Commands::KnownProducts => known_products(&cli.config),
        Commands::Add { targets, repa, .. } => stub("add", &targets, Some(&repa), cli.dry),
        Commands::Remove { targets, repa } => stub("remove", &targets, Some(&repa), cli.dry),
        Commands::Reset { targets, .. } => stub("reset", &targets, None, cli.dry),
        Commands::Install { targets, repa, .. } => stub("install", &targets, Some(&repa), cli.dry),
        Commands::Clear { targets } => stub("clear", &targets, None, cli.dry),
        Commands::Uninstall { targets, repa, .. } => {
            stub("uninstall", &targets, Some(&repa), cli.dry)
        }
        Commands::ListProducts { targets, .. } => stub("list-products", &targets, None, cli.dry),
        Commands::ListRepos { targets } => stub("list-repos", &targets, None, cli.dry),
    }
}

fn known_products(config: &Path) -> ExitCode {
    match template::load_template(config) {
        Ok(tpl) => {
            let mut names: Vec<String> = tpl
                .as_object()
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            names.sort();
            println!("Products known by 'repose':");
            println!("{}", names.join(" "));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn stub(name: &str, targets: &[String], repa: Option<&[String]>, dry: bool) -> ExitCode {
    // Validate target parse early so bad -t fails fast.
    for t in targets {
        if let Err(e) = parse_host(t) {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    }
    if let Some(rs) = repa {
        for r in rs {
            if let Err(e) = repose_core::Repa::parse(r) {
                eprintln!("error: invalid REPA {r:?}: {e}");
                return ExitCode::from(2);
            }
        }
    }
    if dry {
        eprintln!("repose {name}: dry-run accepted; command orchestration not implemented yet");
        return ExitCode::SUCCESS;
    }
    eprintln!(
        "repose {name}: not fully implemented yet (PR15a stub). \
         See docs/design/rust-rewrite.md"
    );
    ExitCode::from(2)
}
