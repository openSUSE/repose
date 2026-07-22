use std::path::{Path, PathBuf};

use repose_core::host_parse::{HostSpec, parse_host};
use repose_core::{ConnectionConfig, HostKeyPolicy};
use repose_ssh::{Host, HostGroup, RusshHost, RusshHostGroup, RusshSession, SshSession};
use tempfile::tempdir;

struct Fixture {
    host: String,
    port: u16,
    user: String,
    identity: PathBuf,
    wrong_identity: PathBuf,
    known_hosts: PathBuf,
    wrong_host_key: PathBuf,
}

impl Fixture {
    fn from_env() -> Option<Self> {
        let fixture = Self {
            host: std::env::var("REPOSE_SSH_HOST").ok()?,
            port: std::env::var("REPOSE_SSH_PORT").ok()?.parse().ok()?,
            user: std::env::var("REPOSE_SSH_USER").ok()?,
            identity: std::env::var_os("REPOSE_SSH_IDENTITY")?.into(),
            wrong_identity: std::env::var_os("REPOSE_SSH_WRONG_IDENTITY")?.into(),
            known_hosts: std::env::var_os("REPOSE_SSH_KNOWN_HOSTS")?.into(),
            wrong_host_key: std::env::var_os("REPOSE_SSH_WRONG_HOST_KEY")?.into(),
        };
        Some(fixture)
    }

    fn config(
        &self,
        policy: HostKeyPolicy,
        known_hosts: PathBuf,
        timeout: f64,
    ) -> ConnectionConfig {
        ConnectionConfig {
            host_key_policy: policy,
            known_hosts: Some(known_hosts),
            timeout,
            ..ConnectionConfig::default()
        }
    }

    fn session(&self, config: ConnectionConfig) -> RusshSession {
        RusshSession::new(&self.host, self.port, &self.user, config)
            .with_identity(self.identity.clone())
    }

    fn spec(&self) -> HostSpec {
        parse_host(&format!("{}@{}:{}", self.user, self.host, self.port))
            .expect("fixture target should parse")
    }
}

#[tokio::test]
async fn live_session_covers_auth_host_keys_commands_sftp_and_reconnect() {
    let Some(fixture) = fixture() else { return };
    let temp = tempdir().expect("temporary known_hosts directory should be created");
    let known_hosts = temp.path().join("known_hosts");

    let mut unauthorized = RusshSession::new(
        &fixture.host,
        fixture.port,
        &fixture.user,
        fixture.config(HostKeyPolicy::Off, known_hosts.clone(), 2.0),
    )
    .with_identity(fixture.wrong_identity.clone());
    assert!(unauthorized.connect().await.is_err());

    let mut strict_unknown =
        fixture.session(fixture.config(HostKeyPolicy::Yes, known_hosts.clone(), 2.0));
    assert!(strict_unknown.connect().await.is_err());

    let mut accept_new =
        fixture.session(fixture.config(HostKeyPolicy::AcceptNew, known_hosts.clone(), 2.0));
    accept_new
        .connect()
        .await
        .expect("accept-new should connect");
    assert!(accept_new.is_active());
    let recorded = std::fs::read_to_string(&known_hosts).expect("host key should be recorded");
    assert!(recorded.contains(&format!("[{}]:{}", fixture.host, fixture.port)));
    accept_new.close().await.expect("session should close");

    let mut strict = fixture.session(fixture.config(HostKeyPolicy::Yes, known_hosts.clone(), 2.0));
    strict
        .connect()
        .await
        .expect("recorded host key should connect");
    let (stdout, stderr, status) = strict
        .run("printf stdout; printf stderr >&2; exit 7")
        .await
        .expect("remote command should complete");
    assert_eq!(
        (stdout.as_str(), stderr.as_str(), status),
        ("stdout", "stderr", 7)
    );

    let listing = strict
        .listdir("/etc/products.d")
        .await
        .expect("products directory should be listed");
    assert!(listing.iter().any(|entry| entry == "SLES.prod"));
    assert!(listing.iter().any(|entry| entry == "qa.prod"));
    let link = strict
        .readlink("/etc/products.d/baseproduct")
        .await
        .expect("baseproduct should be a symlink");
    assert_eq!(link.as_deref(), Some("SLES.prod"));
    let product = strict
        .read_file("/etc/products.d/SLES.prod")
        .await
        .expect("base product should be readable");
    assert!(
        product
            .windows(b"<name>SLES</name>".len())
            .any(|window| window == b"<name>SLES</name>")
    );
    let second_read = strict
        .read_file("/etc/products.d/qa.prod")
        .await
        .expect("cached SFTP session should remain usable");
    assert!(
        second_read
            .windows(b"<name>qa</name>".len())
            .any(|window| window == b"<name>qa</name>")
    );

    strict.close().await.expect("session should close");
    assert!(!strict.is_active());
    strict
        .connect()
        .await
        .expect("closed session should reconnect");
    assert_eq!(
        strict
            .run("printf reconnected")
            .await
            .expect("command should run")
            .0,
        "reconnected"
    );

    let mut timeout = fixture.session(fixture.config(HostKeyPolicy::Yes, known_hosts, 0.1));
    timeout
        .connect()
        .await
        .expect("timeout session should connect");
    let error = timeout
        .run("sleep 2")
        .await
        .expect_err("slow command should time out");
    assert!(error.to_string().contains("timed out"));
}

#[tokio::test]
async fn live_session_rejects_a_changed_host_key_and_off_accepts_it() {
    let Some(fixture) = fixture() else { return };
    let temp = tempdir().expect("temporary known_hosts directory should be created");
    let known_hosts = temp.path().join("known_hosts");
    write_known_host(
        &known_hosts,
        &fixture.host,
        fixture.port,
        &fixture.wrong_host_key,
    );

    let mut strict =
        fixture.session(fixture.config(HostKeyPolicy::AcceptNew, known_hosts.clone(), 2.0));
    assert!(strict.connect().await.is_err());

    let mut off = fixture.session(fixture.config(HostKeyPolicy::Off, known_hosts.clone(), 2.0));
    off.connect()
        .await
        .expect("off policy should bypass the changed key");
    assert!(off.is_active());

    let mut no = fixture.session(fixture.config(HostKeyPolicy::No, known_hosts, 2.0));
    no.connect()
        .await
        .expect("no policy should bypass the changed key");
}

#[tokio::test]
async fn live_host_discovers_system_repositories_and_isolates_connection_failures() {
    let Some(fixture) = fixture() else { return };
    let config = fixture.config(HostKeyPolicy::Yes, fixture.known_hosts.clone(), 2.0);
    let mut host = RusshHost::from_spec(fixture.spec(), config.clone());

    host.connect().await.expect("host should connect");
    host.read_products()
        .await
        .expect("products should be discovered");
    let system = host.products().expect("discovered system should be stored");
    assert_eq!(system.base.name, "SLES");
    assert_eq!(system.base.version, "16.0");
    assert!(system.addons.iter().any(|product| product.name == "qa"));
    host.read_repos()
        .await
        .expect("repositories should be discovered");
    let repos = host.raw_repos().expect("repositories should be stored");
    assert_eq!(repos.len(), 2);
    assert!(repos.iter().any(|repo| repo.alias == "SLES:16.0::pool"));
    assert_eq!(
        host.out()
            .last()
            .expect("zypper invocation should be recorded")
            .0,
        "zypper -x lr"
    );

    let failed = HostSpec {
        key: format!("localhost:{}", fixture.port),
        hostname: "localhost".into(),
        port: fixture.port,
        username: fixture.user.clone(),
    };
    let good_key = fixture.spec().key;
    let mut group = RusshHostGroup::from_targets(vec![fixture.spec(), failed], config);
    group.connect_and_prune().await;

    assert_eq!(group.keys(), vec![good_key]);
    group.close().await;
}

/// P1 step 12: `RusshHostGroup`'s bounded fan-out still processes every
/// host exactly once, with correct per-host results and sorted keys, at
/// the most constrained limit (1 — fully serial admission). Peak-
/// concurrency measurement itself is covered by the identical algorithm's
/// gated tests in `repose_core::mock` (a real SSH session has no gate to
/// instrument); this proves end-to-end correctness against a real
/// transport instead.
#[tokio::test]
async fn live_group_bounded_fan_out_at_limit_one_processes_every_host_exactly_once() {
    let Some(fixture) = fixture() else { return };
    let config = ConnectionConfig {
        host_operation_limit: std::num::NonZeroUsize::new(1).unwrap(),
        ..fixture.config(HostKeyPolicy::Yes, fixture.known_hosts.clone(), 2.0)
    };
    let specs: Vec<HostSpec> = ["hc", "ha", "hb"]
        .into_iter()
        .map(|key| HostSpec {
            key: key.into(),
            hostname: fixture.host.clone(),
            port: fixture.port,
            username: fixture.user.clone(),
        })
        .collect();
    let mut group = RusshHostGroup::from_targets(specs, config);
    assert_eq!(group.host_operation_limit().get(), 1);

    group.connect_and_prune().await;
    assert_eq!(
        group.keys(),
        vec!["ha".to_string(), "hb".to_string(), "hc".to_string()],
        "BTreeMap keeps ascending key order regardless of admission order"
    );

    group.read_products().await;
    for key in ["ha", "hb", "hc"] {
        let host = group.get(key).expect("host should remain after connect");
        assert!(
            host.products().is_some(),
            "{key} should have discovered products"
        );
    }

    group.run_all("printf ok").await;
    for key in ["ha", "hb", "hc"] {
        let out = group.get(key).expect("host should remain").out();
        assert_eq!(
            out.last().expect("run_all should append an out entry").1,
            "ok",
            "{key} should have run the command exactly once"
        );
    }

    group.close().await;
}

/// P1 step 28: an SFTP file read enforces the configured byte limit —
/// exactly-at-limit succeeds, limit-1 (one byte over) returns a typed
/// `SftpFileTooLarge` error, and the default (generous) limit is
/// unaffected for the same real file.
#[tokio::test]
async fn live_sftp_read_enforces_the_configured_byte_limit() {
    let Some(fixture) = fixture() else { return };
    const SLES_PROD_BYTES: usize = 4085; // tests/vectors/refhosts/sles-16-0/products.d/SLES.prod

    let base_config = fixture.config(HostKeyPolicy::Yes, fixture.known_hosts.clone(), 2.0);

    let mut at_limit = fixture.session(ConnectionConfig {
        max_sftp_file_bytes: std::num::NonZeroUsize::new(SLES_PROD_BYTES).unwrap(),
        ..base_config.clone()
    });
    at_limit.connect().await.expect("session should connect");
    let content = at_limit
        .read_file("/etc/products.d/SLES.prod")
        .await
        .expect("exactly-at-limit read should succeed");
    assert_eq!(content.len(), SLES_PROD_BYTES);

    let mut over_limit = fixture.session(ConnectionConfig {
        max_sftp_file_bytes: std::num::NonZeroUsize::new(SLES_PROD_BYTES - 1).unwrap(),
        ..base_config
    });
    over_limit.connect().await.expect("session should connect");
    let error = over_limit
        .read_file("/etc/products.d/SLES.prod")
        .await
        .expect_err("one byte over the limit must be rejected");
    assert_eq!(
        error,
        repose_core::error::SshError::SftpFileTooLarge {
            path: "/etc/products.d/SLES.prod".to_string(),
            limit: SLES_PROD_BYTES - 1,
        }
    );
}

/// P1 step 28: a missing remote file keeps its existing error type/shape
/// (unaffected by the bounded-reader refactor).
#[tokio::test]
async fn live_sftp_read_of_a_missing_file_is_unchanged() {
    let Some(fixture) = fixture() else { return };
    let mut session =
        fixture.session(fixture.config(HostKeyPolicy::Yes, fixture.known_hosts.clone(), 2.0));
    session.connect().await.expect("session should connect");
    let error = session
        .read_file("/etc/products.d/does-not-exist.prod")
        .await
        .expect_err("missing file must fail");
    assert!(matches!(error, repose_core::error::SshError::Other(_)));
}

/// P1 step 30: a `/etc/products.d` listing above the configured plausible-
/// entry cap is rejected before any addon path is constructed, while a
/// normal (small) listing discovers products exactly as before.
#[tokio::test]
async fn live_products_d_listing_above_the_cap_is_rejected() {
    let Some(fixture) = fixture() else { return };
    let config = ConnectionConfig {
        // The fixture's /etc/products.d has 3 entries (SLES.prod, qa.prod,
        // and the baseproduct symlink).
        max_products_d_entries: std::num::NonZeroUsize::new(1).unwrap(),
        ..fixture.config(HostKeyPolicy::Yes, fixture.known_hosts.clone(), 2.0)
    };
    let mut host = RusshHost::from_spec(fixture.spec(), config);
    host.connect().await.expect("host should connect");
    let error = host
        .read_products()
        .await
        .expect_err("an over-cap listing must be rejected");
    assert_eq!(
        error,
        repose_core::error::SshError::DirectoryTooLarge {
            path: "/etc/products.d".to_string(),
            limit: 1,
            observed: 3,
        }
    );
}

/// P1 step 31: stdout exactly at the configured byte limit succeeds and
/// returns the complete payload unchanged.
#[tokio::test]
async fn live_command_output_stdout_at_the_byte_limit_succeeds() {
    let Some(fixture) = fixture() else { return };
    let config = ConnectionConfig {
        max_stdout_bytes: std::num::NonZeroUsize::new(16).unwrap(),
        ..fixture.config(HostKeyPolicy::Yes, fixture.known_hosts.clone(), 2.0)
    };
    let mut session = fixture.session(config);
    session.connect().await.expect("session should connect");
    let (stdout, _, status) = session
        .run("head -c 16 /dev/zero | tr '\\0' 'x'")
        .await
        .expect("exactly-at-limit stdout should succeed");
    assert_eq!(stdout, "x".repeat(16));
    assert_eq!(status, 0);
}

/// P1 step 31: one byte over the stdout limit returns a typed
/// `OutputTooLarge` error, and the same session remains usable afterward
/// (the overflow cleanup must not leave the transport unusable).
#[tokio::test]
async fn live_command_output_stdout_one_byte_over_the_limit_is_rejected_and_session_is_reusable() {
    let Some(fixture) = fixture() else { return };
    let config = ConnectionConfig {
        max_stdout_bytes: std::num::NonZeroUsize::new(16).unwrap(),
        ..fixture.config(HostKeyPolicy::Yes, fixture.known_hosts.clone(), 2.0)
    };
    let mut session = fixture.session(config);
    session.connect().await.expect("session should connect");
    let error = session
        .run("head -c 17 /dev/zero | tr '\\0' 'x'")
        .await
        .expect_err("one byte over the stdout limit must be rejected");
    assert_eq!(
        error,
        repose_core::error::SshError::OutputTooLarge {
            stream: repose_core::error::OutputStream::Stdout,
            limit: 16,
        }
    );
    let (stdout, _, status) = session
        .run("printf ok")
        .await
        .expect("the session must remain usable after an overflow");
    assert_eq!((stdout.as_str(), status), ("ok", 0));
}

/// P1 step 31: stderr has its own, independently enforced byte limit.
#[tokio::test]
async fn live_command_output_stderr_one_byte_over_the_limit_is_rejected() {
    let Some(fixture) = fixture() else { return };
    let config = ConnectionConfig {
        max_stderr_bytes: std::num::NonZeroUsize::new(16).unwrap(),
        ..fixture.config(HostKeyPolicy::Yes, fixture.known_hosts.clone(), 2.0)
    };
    let mut session = fixture.session(config);
    session.connect().await.expect("session should connect");
    let error = session
        .run("head -c 17 /dev/zero | tr '\\0' 'x' 1>&2")
        .await
        .expect_err("one byte over the stderr limit must be rejected");
    assert_eq!(
        error,
        repose_core::error::SshError::OutputTooLarge {
            stream: repose_core::error::OutputStream::Stderr,
            limit: 16,
        }
    );
}

/// P1 step 31: stdout and stderr caps are independent — an in-limit
/// stderr payload does not mask, and is not mistakenly counted toward, an
/// oversized stdout payload.
#[tokio::test]
async fn live_command_output_mixed_streams_enforce_independent_limits() {
    let Some(fixture) = fixture() else { return };
    let config = ConnectionConfig {
        max_stdout_bytes: std::num::NonZeroUsize::new(16).unwrap(),
        max_stderr_bytes: std::num::NonZeroUsize::new(1024).unwrap(),
        ..fixture.config(HostKeyPolicy::Yes, fixture.known_hosts.clone(), 2.0)
    };
    let mut session = fixture.session(config);
    session.connect().await.expect("session should connect");
    let error = session
        .run("head -c 8 /dev/zero | tr '\\0' 'e' 1>&2; head -c 17 /dev/zero | tr '\\0' 'o'")
        .await
        .expect_err("stdout overflow must still be detected despite in-limit stderr");
    assert_eq!(
        error,
        repose_core::error::SshError::OutputTooLarge {
            stream: repose_core::error::OutputStream::Stdout,
            limit: 16,
        }
    );
}

/// P1 step 32: an output-limit overflow reaches `RusshHost::run` through
/// the same generic failure contract as other transport errors: exactly
/// one synthetic out entry (rc -1, empty streams) — never the oversized or
/// partial payload.
#[tokio::test]
async fn live_host_run_maps_an_output_overflow_to_a_single_synthetic_out_entry() {
    let Some(fixture) = fixture() else { return };
    let config = ConnectionConfig {
        max_stdout_bytes: std::num::NonZeroUsize::new(4).unwrap(),
        ..fixture.config(HostKeyPolicy::Yes, fixture.known_hosts.clone(), 2.0)
    };
    let mut host = RusshHost::from_spec(fixture.spec(), config);
    host.connect().await.expect("host should connect");
    host.run("printf overflowed")
        .await
        .expect("run() maps the overflow into the out history rather than returning Err");
    assert_eq!(
        host.out().len(),
        1,
        "exactly one out entry, no partial content"
    );
    let (command, stdout, stderr, exit_code, _runtime) = &host.out()[0];
    assert_eq!(command, "printf overflowed");
    assert_eq!(stdout, "");
    assert_eq!(stderr, "");
    assert_eq!(*exit_code, -1);
}

fn write_known_host(path: &Path, host: &str, port: u16, public_key: &Path) {
    let key = std::fs::read_to_string(public_key).expect("wrong public key should be readable");
    let fields: Vec<&str> = key.split_whitespace().take(2).collect();
    std::fs::write(
        path,
        format!("[{host}]:{port} {} {}\n", fields[0], fields[1]),
    )
    .expect("known_hosts should be writable");
}

fn fixture() -> Option<Fixture> {
    let fixture = Fixture::from_env();
    assert!(
        fixture.is_some() || std::env::var_os("REPOSE_SSH_REQUIRED").is_none(),
        "OpenSSH fixture variables are required"
    );
    fixture
}
