//! russh-backed [`SshSession`] (single SSH backend).

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use async_trait::async_trait;
use repose_core::config::ConnectionConfig;
use repose_core::error::SshError;
use repose_core::traits::SshSession;
use russh::client::{self, Handler};
use russh::keys::agent::client::AgentClient;
use russh::keys::{load_secret_key, Algorithm, PrivateKeyWithHashAlg};
use russh::{ChannelMsg, Disconnect, Preferred};
use russh_sftp::client::SftpSession;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use zeroize::Zeroize;

use crate::hostkey::HostKeyVerifier;
use crate::openssh_config::OpenSshOptions;

struct ClientHandler {
    verifier: HostKeyVerifier,
}

/// Bidirectional stdio stream supplied by an OpenSSH `ProxyCommand` process.
struct ProxyStream {
    _child: Child,
    reader: ChildStdout,
    writer: ChildStdin,
}

impl ProxyStream {
    fn spawn(command: &str) -> Result<Self, SshError> {
        let mut process = Command::new("sh");
        process
            .arg("-c")
            .arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        let mut child = process.spawn().map_err(|error| {
            SshError::Transport(format!("ProxyCommand failed to start: {error}"))
        })?;
        let reader = child
            .stdout
            .take()
            .ok_or_else(|| SshError::Transport("ProxyCommand stdout was unavailable".into()))?;
        let writer = child
            .stdin
            .take()
            .ok_or_else(|| SshError::Transport("ProxyCommand stdin was unavailable".into()))?;
        Ok(Self {
            _child: child,
            reader,
            writer,
        })
    }
}

impl AsyncRead for ProxyStream {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().reader).poll_read(context, buffer)
    }
}

impl AsyncWrite for ProxyStream {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().writer).poll_write(context, buffer)
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().writer).poll_flush(context)
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().writer).poll_shutdown(context)
    }
}

impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(self.verifier.verify_public_key(server_public_key))
    }
}

/// Live russh client session.
pub struct RusshSession {
    hostname: String,
    port: u16,
    username: String,
    config: ConnectionConfig,
    handle: Option<client::Handle<ClientHandler>>,
    sftp: Option<SftpSession>,
    identity: Option<PathBuf>,
}

impl RusshSession {
    #[must_use]
    pub fn new(
        hostname: impl Into<String>,
        port: u16,
        username: impl Into<String>,
        config: ConnectionConfig,
    ) -> Self {
        Self {
            hostname: hostname.into(),
            port,
            username: username.into(),
            config,
            handle: None,
            sftp: None,
            identity: None,
        }
    }

    /// Prefer a specific private key path (default: try `~/.ssh/id_ed25519` then `id_rsa`).
    #[must_use]
    pub fn with_identity(mut self, path: PathBuf) -> Self {
        self.identity = Some(path);
        self
    }

    fn candidate_keys(&self, configured_keys: &[PathBuf]) -> Vec<PathBuf> {
        if let Some(p) = &self.identity {
            return vec![p.clone()];
        }
        if !configured_keys.is_empty() {
            return configured_keys.to_vec();
        }
        let mut keys = Vec::new();
        if let Some(home) = std::env::var_os("HOME") {
            let ssh = Path::new(&home).join(".ssh");
            for name in ["id_ed25519", "id_rsa", "id_ecdsa"] {
                let p = ssh.join(name);
                if p.is_file() {
                    keys.push(p);
                }
            }
        }
        keys
    }

    async fn connect_inner(&mut self) -> Result<(), SshError> {
        // Any cached SFTP channel rode on the previous transport.
        self.sftp = None;
        let options = OpenSshOptions::lookup(&self.hostname);
        // With a ProxyCommand the proxy resolves the name on its side, so
        // the known-hosts identity stays the CLI alias (paramiko parity).
        let target_host = if options.proxy_command.is_some() {
            self.hostname.clone()
        } else {
            options
                .hostname
                .clone()
                .unwrap_or_else(|| self.hostname.clone())
        };
        let target_port = options.port.unwrap_or(self.port);
        let target_user = options
            .user
            .clone()
            .unwrap_or_else(|| self.username.clone());
        let conf = client::Config {
            // The per-command timeout must NOT become russh's
            // `inactivity_timeout`: that would garbage-collect an
            // idle-but-healthy transport a few seconds after every command,
            // permanently degrading the host. Keep the transport open
            // indefinitely and detect liveness with protocol keepalives
            // instead (the user-visible `ServerAliveInterval` semantics).
            inactivity_timeout: None,
            keepalive_interval: Some(Duration::from_secs(10)),
            preferred: Preferred::default(),
            ..Default::default()
        };
        let conf = Arc::new(conf);
        let verifier = HostKeyVerifier::new(
            self.config.host_key_policy,
            self.config.known_hosts.clone(),
            target_host.clone(),
            target_port,
        )
        .map(|verifier| verifier.with_alias(self.hostname.clone()))?;
        let handler = ClientHandler { verifier };
        let mut session = if let Some(command) =
            proxy_command_line(&options, &self.hostname, self.port, &target_user)
        {
            let stream = ProxyStream::spawn(&command)?;
            client::connect_stream(conf, stream, handler)
                .await
                .map_err(|e| SshError::Transport(e.to_string()))?
        } else {
            let addrs = (target_host.as_str(), target_port);
            client::connect(conf, addrs, handler)
                .await
                .map_err(|e| SshError::Transport(e.to_string()))?
        };

        // Auth order mirrors asyncssh's default: ssh-agent first (silently
        // skipped when SSH_AUTH_SOCK is absent or unusable), then
        // IdentityFile / default key files, then one password prompt.
        let mut last_err = "no SSH private key found (~/.ssh/id_ed25519|id_rsa)".to_string();
        if try_agent_auth(&mut session, &target_user, &mut last_err).await {
            self.handle = Some(session);
            return Ok(());
        }

        for key_path in self.candidate_keys(&options.identity_files) {
            let key_pair = match load_secret_key(&key_path, None) {
                Ok(k) => k,
                Err(russh::keys::Error::KeyIsEncrypted) if std::io::stdin().is_terminal() => {
                    match load_encrypted_key(&key_path) {
                        Ok(k) => k,
                        Err(e) => {
                            last_err = e;
                            continue;
                        }
                    }
                }
                // Fail-closed parity: without a TTY an encrypted key is
                // skipped instead of blocking on a passphrase prompt.
                Err(e) => {
                    last_err = format!("{}: {e}", key_path.display());
                    continue;
                }
            };
            let hash = session
                .best_supported_rsa_hash()
                .await
                .map_err(|e| SshError::Transport(e.to_string()))?
                .flatten();
            let auth = session
                .authenticate_publickey(
                    target_user.clone(),
                    PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash),
                )
                .await
                .map_err(|e| SshError::Transport(e.to_string()))?;
            if auth.success() {
                self.handle = Some(session);
                return Ok(());
            }
            last_err = format!("publickey auth failed with {}", key_path.display());
        }

        // Non-TTY: fail closed for password (design decision).
        if !std::io::stdin().is_terminal() {
            return Err(SshError::Transport(format!(
                "authentication failed for {}@{}:{} ({last_err}; password authentication requires a TTY)",
                target_user, target_host, target_port
            )));
        }

        let mut password =
            rpassword::prompt_password(format!("Password for {}@{}: ", target_user, target_host))
                .map_err(|e| SshError::Transport(format!("could not read SSH password: {e}")))?;
        let auth = session
            .authenticate_password(target_user.clone(), &password)
            .await;
        password.zeroize();
        let auth = auth.map_err(|e| SshError::Transport(e.to_string()))?;
        if auth.success() {
            self.handle = Some(session);
            return Ok(());
        }

        Err(SshError::Transport(format!(
            "authentication failed for {}@{}:{} ({last_err})",
            target_user, target_host, target_port
        )))
    }

    async fn run_inner(&mut self, command: &str) -> Result<(String, String, i32), SshError> {
        let handle = self
            .handle
            .as_mut()
            .ok_or_else(|| SshError::NotConnected(self.hostname.clone()))?;
        let mut channel = handle
            .channel_open_session()
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        channel
            .exec(true, command)
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;

        let timeout = Duration::from_secs_f64(self.config.timeout);
        let collect = async {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let mut code = None;
            loop {
                let Some(msg) = channel.wait().await else {
                    break;
                };
                match msg {
                    ChannelMsg::Data { ref data } => stdout.extend_from_slice(data),
                    ChannelMsg::ExtendedData { ref data, .. } => {
                        stderr.extend_from_slice(data);
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        // SSH exit codes fit in 0..=255; make truncation explicit.
                        code = Some(i32::try_from(exit_status).unwrap_or(-1));
                    }
                    _ => {}
                }
            }
            let code = code.unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&stdout).into_owned();
            let stderr = String::from_utf8_lossy(&stderr).into_owned();
            Ok::<_, SshError>((stdout, stderr, code))
        };

        match tokio::time::timeout(timeout, collect).await {
            Ok(r) => r,
            Err(_) => Err(SshError::Transport(format!(
                "command timed out after {}s",
                self.config.timeout
            ))),
        }
    }

    fn sftp_timeout_secs(&self) -> u64 {
        if self.config.timeout.is_finite() && self.config.timeout > 0.0 {
            self.config.timeout.ceil() as u64
        } else {
            1
        }
    }

    async fn sftp_session(&mut self) -> Result<&SftpSession, SshError> {
        if self.sftp.is_none() {
            let handle = self
                .handle
                .as_mut()
                .ok_or_else(|| SshError::NotConnected(self.hostname.clone()))?;
            let channel = handle
                .channel_open_session()
                .await
                .map_err(|e| SshError::Transport(e.to_string()))?;
            channel
                .request_subsystem(true, "sftp")
                .await
                .map_err(|e| SshError::Transport(e.to_string()))?;
            let sftp = SftpSession::new(channel.into_stream())
                .await
                .map_err(|e| SshError::Transport(format!("SFTP initialization failed: {e}")))?;
            sftp.set_timeout(self.sftp_timeout_secs());
            self.sftp = Some(sftp);
        }

        self.sftp
            .as_ref()
            .ok_or_else(|| SshError::Other("SFTP session was not initialized".into()))
    }
}

/// Try every ssh-agent identity before touching key files (asyncssh's
/// default order). Returns `true` on success; `false` when the agent is
/// absent (no `SSH_AUTH_SOCK`), empty, or refused — callers silently fall
/// through to file-based keys. Certificate identities are attempted with
/// their embedded public key (agents typically hold the plain key too);
/// full OpenSSH client-certificate auth is intentionally out of scope.
async fn try_agent_auth(
    session: &mut client::Handle<ClientHandler>,
    user: &str,
    last_err: &mut String,
) -> bool {
    let Ok(mut agent) = AgentClient::connect_env().await else {
        return false;
    };
    let identities = match agent.request_identities().await {
        Ok(identities) => identities,
        Err(error) => {
            log::debug!("ssh-agent identity listing failed: {error}");
            return false;
        }
    };
    for identity in identities {
        let key = identity.public_key().into_owned();
        let hash = if matches!(key.algorithm(), Algorithm::Rsa { .. }) {
            match session.best_supported_rsa_hash().await {
                Ok(hash) => hash.flatten(),
                Err(_) => None,
            }
        } else {
            None
        };
        match session
            .authenticate_publickey_with(user.to_string(), key, hash, &mut agent)
            .await
        {
            Ok(auth) if auth.success() => return true,
            Ok(_) => *last_err = "ssh-agent keys were refused".to_string(),
            Err(error) => *last_err = format!("ssh-agent auth failed: {error}"),
        }
    }
    false
}

/// Prompt once for the passphrase of an encrypted key and load it.
/// Only called when stdin is a TTY.
fn load_encrypted_key(key_path: &Path) -> Result<russh::keys::PrivateKey, String> {
    let mut passphrase =
        rpassword::prompt_password(format!("Enter passphrase for {}: ", key_path.display()))
            .map_err(|error| format!("could not read passphrase: {error}"))?;
    let loaded = load_secret_key(key_path, Some(&passphrase));
    passphrase.zeroize();
    loaded.map_err(|error| format!("{}: {error}", key_path.display()))
}

/// Build the ProxyCommand line. OpenSSH expands `%h` with the
/// post-`Hostname`-resolution host — not the CLI alias — and `%p` with the
/// resolved port.
fn proxy_command_line(
    options: &OpenSshOptions,
    alias: &str,
    fallback_port: u16,
    user: &str,
) -> Option<String> {
    let proxy_command = options.proxy_command.as_deref()?;
    let host = options.hostname.as_deref().unwrap_or(alias);
    let port = options.port.unwrap_or(fallback_port);
    Some(expand_proxy_command(proxy_command, host, port, user))
}

/// Sleep (seconds) before reconnect `attempt` (1-based), matching the Python
/// schedule (aiossh.py `wait_reconnect`): the first sleep is the base
/// timeout; attempt n > 1 sleeps `2 * (timeout + 5 * (n - 1))`. For the
/// defaults (retry=10, timeout=10) that is 10,30,40,...,110 — 640 s total.
fn reconnect_sleep_secs(attempt: u32, timeout_secs: u64, backoff: bool) -> u64 {
    if attempt <= 1 || !backoff {
        timeout_secs
    } else {
        2 * (timeout_secs + 5 * u64::from(attempt - 1))
    }
}

fn expand_proxy_command(command: &str, host: &str, port: u16, user: &str) -> String {
    let mut expanded = String::with_capacity(command.len());
    let mut characters = command.chars();
    while let Some(character) = characters.next() {
        if character != '%' {
            expanded.push(character);
            continue;
        }
        match characters.next() {
            Some('h') => expanded.push_str(host),
            Some('p') => expanded.push_str(&port.to_string()),
            Some('r') => expanded.push_str(user),
            Some('%') => expanded.push('%'),
            Some(other) => {
                expanded.push('%');
                expanded.push(other);
            }
            None => expanded.push('%'),
        }
    }
    expanded
}

#[async_trait]
impl SshSession for RusshSession {
    async fn connect(&mut self) -> Result<(), SshError> {
        if let Some(handle) = &self.handle {
            if !handle.is_closed() {
                return Ok(());
            }
            // The transport died underneath us (network drop, server-side
            // disconnect): discard it so a fresh connect replaces the dead
            // handle instead of no-op'ing the host into a permanently
            // degraded state (every run() would return rc -1 forever).
            self.handle = None;
            self.sftp = None;
        }
        self.connect_inner().await
    }

    async fn run(&mut self, command: &str) -> Result<(String, String, i32), SshError> {
        self.run_inner(command).await
    }

    async fn listdir(&mut self, path: &str) -> Result<Vec<String>, SshError> {
        let entries = self
            .sftp_session()
            .await?
            .read_dir(path)
            .await
            .map_err(|e| SshError::Other(format!("SFTP listdir {path}: {e}")))?;
        Ok(entries.map(|entry| entry.file_name()).collect())
    }

    async fn readlink(&mut self, path: &str) -> Result<Option<String>, SshError> {
        self.sftp_session()
            .await?
            .read_link(path)
            .await
            .map(Some)
            .map_err(|e| SshError::Other(format!("SFTP readlink {path}: {e}")))
    }

    async fn read_file(&mut self, path: &str) -> Result<Vec<u8>, SshError> {
        self.sftp_session()
            .await?
            .read(path)
            .await
            .map_err(|e| SshError::Other(format!("SFTP read_file {path}: {e}")))
    }

    async fn close(&mut self) -> Result<(), SshError> {
        if let Some(sftp) = self.sftp.take() {
            let _ = sftp.close().await;
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle
                .disconnect(Disconnect::ByApplication, "", "English")
                .await;
        }
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| !handle.is_closed())
    }

    async fn fire_and_forget(&mut self, command: &str) -> Result<(), SshError> {
        let handle = self
            .handle
            .as_mut()
            .ok_or_else(|| SshError::NotConnected(self.hostname.clone()))?;
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        channel
            .exec(true, command)
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        // Do not wait for exit — reboot drops the link.
        Ok(())
    }

    async fn boot_id(&mut self) -> String {
        match self.run_inner("cat /proc/sys/kernel/random/boot_id").await {
            Ok((stdout, _, 0)) => stdout.trim().to_string(),
            _ => String::new(),
        }
    }

    async fn wait_reconnect(&mut self, retry: u32, timeout_secs: u64, backoff: bool) -> bool {
        let _ = self.close().await;
        let mut count = 0u32;
        while !self.is_active() && count < retry {
            count += 1;
            let sleep_secs = reconnect_sleep_secs(count, timeout_secs, backoff);
            tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
            if let Err(error) = self.connect_inner().await {
                log::debug!(
                    "reconnect attempt {count}/{retry} to {}:{} failed: {error}",
                    self.hostname,
                    self.port
                );
            }
        }
        self.is_active()
    }
}

#[cfg(test)]
mod tests {
    use repose_core::ConnectionConfig;

    use super::RusshSession;
    use crate::openssh_config::OpenSshOptions;

    #[test]
    fn sftp_timeout_rounds_up_and_has_a_safe_minimum() {
        let config = ConnectionConfig {
            timeout: 1.2,
            ..ConnectionConfig::default()
        };
        let session = RusshSession::new("host", 22, "root", config);
        assert_eq!(session.sftp_timeout_secs(), 2);

        let config = ConnectionConfig {
            timeout: 0.0,
            ..ConnectionConfig::default()
        };
        let session = RusshSession::new("host", 22, "root", config);
        assert_eq!(session.sftp_timeout_secs(), 1);
    }

    #[test]
    fn proxy_command_expands_openssh_tokens() {
        assert_eq!(
            super::expand_proxy_command("nc %h %p as-%r %% %x", "host", 2200, "root"),
            "nc host 2200 as-root % %x"
        );
    }

    #[test]
    fn proxy_command_expands_the_resolved_hostname_not_the_alias() {
        let options = OpenSshOptions {
            hostname: Some("real.example".into()),
            port: Some(2200),
            proxy_command: Some("nc %h %p".into()),
            ..OpenSshOptions::default()
        };
        assert_eq!(
            super::proxy_command_line(&options, "alias", 22, "root").as_deref(),
            Some("nc real.example 2200")
        );

        // Without a Hostname override the alias is the resolved host.
        let bare = OpenSshOptions {
            proxy_command: Some("nc %h %p".into()),
            ..OpenSshOptions::default()
        };
        assert_eq!(
            super::proxy_command_line(&bare, "alias", 22, "root").as_deref(),
            Some("nc alias 22")
        );
        assert_eq!(
            super::proxy_command_line(&OpenSshOptions::default(), "alias", 22, "root"),
            None
        );
    }

    #[test]
    fn reconnect_schedule_matches_the_python_backoff() {
        // aiossh.py wait_reconnect: 10,30,40,...,110 — 640 s in total.
        let schedule: Vec<u64> = (1..=10)
            .map(|attempt| super::reconnect_sleep_secs(attempt, 10, true))
            .collect();
        assert_eq!(schedule, [10, 30, 40, 50, 60, 70, 80, 90, 100, 110]);
        assert_eq!(schedule.iter().sum::<u64>(), 640);
        assert!((1..=10).all(|attempt| super::reconnect_sleep_secs(attempt, 10, false) == 10));
    }
}
