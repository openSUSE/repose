use std::path::{Path, PathBuf};

use repose_core::host_parse::{parse_host, HostSpec};
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
    assert!(product
        .windows(b"<name>SLES</name>".len())
        .any(|window| window == b"<name>SLES</name>"));
    let second_read = strict
        .read_file("/etc/products.d/qa.prod")
        .await
        .expect("cached SFTP session should remain usable");
    assert!(second_read
        .windows(b"<name>qa</name>".len())
        .any(|window| window == b"<name>qa</name>"));

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
