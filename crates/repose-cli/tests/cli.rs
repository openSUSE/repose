use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repose(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repose"))
        .args(args)
        .env_remove("COLOR")
        .env_remove("NO_COLOR")
        // Keep the log-color tests hermetic: a cert store pointed at empty
        // locations would suppress the debug line they rely on.
        .env_remove("SSL_CERT_FILE")
        .env_remove("SSL_CERT_DIR")
        .output()
        .expect("repose process should start")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn vector(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/vectors")
        .join(path)
}

#[test]
fn no_command_prints_help_and_succeeds() {
    let output = repose(&[]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("Usage: repose [OPTIONS] [COMMAND]"));
}

#[test]
fn help_and_version_are_process_visible() {
    let help = repose(&["--help"]);
    let version = repose(&["--version"]);

    assert!(help.status.success());
    assert!(stdout(&help).contains("known-products"));
    assert!(version.status.success());
    assert_eq!(
        stdout(&version),
        format!("repose version: {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn conflicting_verbosity_exits_two() {
    let output = repose(&["--debug", "--quiet", "known-products"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("mutually exclusive"));
}

#[test]
fn clap_rejects_missing_command_arguments() {
    for args in [&["add"][..], &["add", "-t", "host"][..]] {
        let output = repose(args);
        assert_eq!(output.status.code(), Some(2), "args: {args:?}");
        assert!(stderr(&output).contains("required"), "args: {args:?}");
    }
}

#[test]
fn invalid_host_and_repa_exit_two_without_connecting() {
    let host = repose(&["list-products", "-t", "host:not-a-port"]);
    let repa = repose(&["remove", "-t", "host", "a:b:c:d:e"]);

    assert_eq!(host.status.code(), Some(2));
    assert!(stderr(&host).contains("Wrong port specification"));
    assert_eq!(repa.status.code(), Some(2));
    assert!(stderr(&repa).contains("invalid REPA"));
}

#[test]
fn known_products_matches_text_golden() {
    let config = vector("template/sample.yml");
    let golden = std::fs::read(vector("cli/known_products.txt"))
        .expect("known-products golden should be readable");
    let output = repose(&["--config", path(&config), "known-products"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, golden);
}

#[test]
fn known_products_json_is_ndjson() {
    let config = vector("template/sample.yml");
    let output = repose(&["--config", path(&config), "--format=json", "known-products"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let output_text = stdout(&output);
    let lines: Vec<&str> = output_text.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains(r#""event": "known_product""#));
    assert!(lines[0].contains(r#""name": "PackageHub""#));
    assert!(lines[2].contains(r#""name": "SLES""#));
}

#[test]
fn missing_products_config_exits_two() {
    let output = repose(&[
        "--config",
        "/definitely/missing/repose-products.yml",
        "known-products",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).starts_with("error:"));
}

#[test]
fn no_color_output_contains_no_ansi_escape() {
    let config = vector("template/sample.yml");
    let output = repose(&["--no-color", "--config", path(&config), "known-products"]);

    assert!(output.status.success());
    assert!(!output.stdout.contains(&0x1b));
    assert!(!output.stderr.contains(&0x1b));
}

#[test]
fn color_never_flag_contains_no_ansi_escape() {
    let config = vector("template/sample.yml");
    let output = repose(&["--color=never", "--config", path(&config), "known-products"]);

    assert!(output.status.success());
    assert!(!output.stdout.contains(&0x1b));
    assert!(!output.stderr.contains(&0x1b));
}

#[test]
fn color_always_flag_colorizes_known_products_label() {
    // Python 2.1.0 colors the `Products known by 'repose':` label green
    // (utils.green): `\x1b[1;32m…\x1b[1;m\x1b[0m`.
    let config = vector("template/sample.yml");
    let output = repose(&[
        "--color=always",
        "--config",
        path(&config),
        "known-products",
    ]);

    assert!(output.status.success());
    assert!(
        stdout(&output).contains("\x1b[1;32mProducts known by 'repose':\x1b[1;m\x1b[0m"),
        "{}",
        stdout(&output)
    );
}

// Linux-only: both stderr-log color tests below rely on `-d` emitting at least
// one DEBUG record. The only such record for these no-network commands is
// "Loaded N CA root certificates" from rustls-platform-verifier — and that
// string is emitted solely on the Linux/BSD path that reads CA files from disk.
// On macOS the verifier uses Apple's Security framework, and on Windows the
// CryptoAPI, neither of which logs it; the commands then produce no DEBUG line
// and there is nothing to (not) colorize. The tests knowingly couple to this
// third-party string, so gate them to the platform where it exists.
#[cfg(target_os = "linux")]
#[test]
fn color_never_disables_ansi_in_stderr_logs() {
    // --color=never must keep stderr ANSI-free, exactly like its documented
    // alias --no-color.
    let output = repose(&[
        "-d",
        "--color=never",
        "add",
        "-t",
        "nonexistent.invalid",
        "--no-probe",
        "SLES",
    ]);
    let logs = stderr(&output);
    assert!(
        logs.contains("DEBUG"),
        "expected a debug log record: {logs:?}"
    );
    assert!(
        !logs.contains('\u{1b}'),
        "no ANSI escapes allowed: {logs:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn color_always_enables_ansi_in_stderr_logs_even_when_piped() {
    let output = repose(&[
        "-d",
        "--color=always",
        "add",
        "-t",
        "nonexistent.invalid",
        "--no-probe",
        "SLES",
    ]);
    assert!(stderr(&output).contains('\u{1b}'));
}

#[test]
fn no_color_env_var_contains_no_ansi_escape() {
    // NO_COLOR is process-global; each subprocess gets an isolated env, so this
    // is the safe place to exercise the env-var force-off precedence.
    let config = vector("template/sample.yml");
    let output = Command::new(env!("CARGO_BIN_EXE_repose"))
        .args(["--config", path(&config), "known-products"])
        .env_remove("COLOR")
        .env("NO_COLOR", "1")
        .output()
        .expect("repose process should start");

    assert!(output.status.success());
    assert!(!output.stdout.contains(&0x1b));
    assert!(!output.stderr.contains(&0x1b));
}

#[test]
fn ssh_fixture_exercises_query_and_dry_run_commands() {
    let Some(target) = std::env::var_os("REPOSE_SSH_TARGET") else {
        assert!(
            std::env::var_os("REPOSE_SSH_REQUIRED").is_none(),
            "OpenSSH fixture variables are required"
        );
        return;
    };
    let known_hosts = std::env::var_os("REPOSE_SSH_KNOWN_HOSTS")
        .expect("fixture should provide REPOSE_SSH_KNOWN_HOSTS");
    let common = [
        "--strict-host-key-checking=yes",
        "--known-hosts",
        path(Path::new(&known_hosts)),
    ];
    let target = target.to_string_lossy();

    let products = repose_with(&common, &["list-products", "-t", &target]);
    assert!(products.status.success(), "{}", stderr(&products));
    assert!(stdout(&products).contains("Base product: SLES-16.0-x86_64"));
    assert!(stdout(&products).contains("Addon: qa"));

    let products_json = repose_with(&common, &["--format=json", "list-products", "-t", &target]);
    assert!(products_json.status.success(), "{}", stderr(&products_json));
    assert!(stdout(&products_json).contains(r#""event": "product""#));

    let repos = repose_with(&common, &["list-repos", "-t", &target]);
    assert!(repos.status.success(), "{}", stderr(&repos));
    assert!(stdout(&repos).contains("SLES:16.0::pool"));

    let repos_json = repose_with(&common, &["--format=json", "list-repos", "-t", &target]);
    assert!(repos_json.status.success(), "{}", stderr(&repos_json));
    assert!(stdout(&repos_json).contains(r#""event": "repo""#));

    let dry = repose_with(&common, &["--print", "clear", "-t", &target]);
    assert!(dry.status.success(), "{}", stderr(&dry));
    assert!(stdout(&dry).contains("zypper -n rr"));

    let dry_json = repose_with(
        &common,
        &["--print", "--format=json", "clear", "-t", &target],
    );
    assert!(dry_json.status.success(), "{}", stderr(&dry_json));
    assert!(stdout(&dry_json).contains(r#""event":"dry""#));
}

fn repose_with(common: &[&str], args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_repose"))
        .args(common)
        .args(args)
        .env_remove("SSH_AUTH_SOCK")
        .env_remove("COLOR")
        .env("NO_COLOR", "1")
        .output()
        .expect("repose process should start")
}

fn path(path: &Path) -> &str {
    path.to_str().expect("test paths should be UTF-8")
}
