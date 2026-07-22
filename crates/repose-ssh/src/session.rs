//! russh-backed [`SshSession`] (single SSH backend).

use std::future::Future;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use repose_core::config::ConnectionConfig;
use repose_core::error::{OutputStream, SshError, TimeoutPhase};
use repose_core::traits::SshSession;
use russh::client::{self, Handler};
use russh::keys::agent::client::AgentClient;
use russh::keys::{Algorithm, PrivateKeyWithHashAlg, load_secret_key};
use russh::{ChannelMsg, Disconnect, Preferred};
use russh_sftp::client::SftpSession;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use zeroize::Zeroize;

use crate::hostkey::{HostKeyVerifier, KeyDecision, persist_first_contact};
use crate::openssh_config::OpenSshOptions;

/// Run `fut` under `deadline`, mapping an elapsed deadline to a typed
/// [`SshError::Timeout`] identifying `phase` (P1 step 24). A phase that
/// completes before its own deadline returns `fut`'s result unchanged.
async fn with_deadline<T>(
    phase: TimeoutPhase,
    deadline: Duration,
    fut: impl Future<Output = Result<T, SshError>>,
) -> Result<T, SshError> {
    match tokio::time::timeout(deadline, fut).await {
        Ok(result) => result,
        Err(_) => Err(SshError::Timeout { phase, deadline }),
    }
}

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
        match self.verifier.decide_public_key(server_public_key) {
            KeyDecision::Accept => Ok(true),
            KeyDecision::Reject => Ok(false),
            // `accept-new` first contact (P1 steps 34–35): persist off the
            // async runtime via `spawn_blocking` and trust the key only
            // after a successful durable write — a slow disk delays this
            // one handshake, not sibling connections/timers, and a
            // persistence failure rejects the session (fail-closed) rather
            // than silently trusting an unrecorded key.
            KeyDecision::PersistFirstContact { path, key } => {
                let host = self.verifier.host().to_string();
                let port = self.verifier.port();
                let persist_host = host.clone();
                let persist_key = key.clone();
                let persist_path = path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    persist_first_contact(&persist_path, &persist_host, port, &persist_key)
                })
                .await;
                match result {
                    Ok(Ok(())) => {
                        self.verifier.record_first_contact(key);
                        Ok(true)
                    }
                    Ok(Err(error)) => {
                        let persist_error = SshError::KnownHostsPersistFailed {
                            host,
                            path: path.display().to_string(),
                            reason: error.to_string(),
                        };
                        log::error!("accept-new: {persist_error}");
                        Ok(false)
                    }
                    Err(join_error) => {
                        log::error!(
                            "accept-new: host key persistence task failed for {host}: {join_error}"
                        );
                        Ok(false)
                    }
                }
            }
        }
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
    /// `~/.ssh/config` options resolved once at first connect; reconnect
    /// attempts reuse them instead of re-reading and re-parsing the file.
    openssh_options: Option<OpenSshOptions>,
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
            openssh_options: None,
        }
    }

    /// Prefer a specific private key path (default: try `~/.ssh/id_ed25519` then `id_rsa`).
    #[must_use]
    pub fn with_identity(mut self, path: PathBuf) -> Self {
        self.identity = Some(path);
        self
    }

    /// The configured plausible-entry cap for `/etc/products.d` listings
    /// (P1 step 30).
    #[must_use]
    pub(crate) fn max_products_d_entries(&self) -> usize {
        self.config.max_products_d_entries.get()
    }

    /// Resolve the candidate private-key paths for automatic
    /// authentication. The fast paths (`--identity`, or `IdentityFile`
    /// entries already resolved from `~/.ssh/config`) touch no filesystem
    /// state; only the fallback default-key scan performs blocking `stat`
    /// calls, so only that branch runs on the blocking pool (P1 step 33) —
    /// per this plan's decision, local credential discovery is off the
    /// async thread but outside the network-authentication deadline.
    async fn candidate_keys(&self, configured_keys: &[PathBuf]) -> Result<Vec<PathBuf>, SshError> {
        if let Some(p) = &self.identity {
            return Ok(vec![p.clone()]);
        }
        if !configured_keys.is_empty() {
            return Ok(configured_keys.to_vec());
        }
        tokio::task::spawn_blocking(default_key_candidates)
            .await
            .map_err(|error| SshError::Other(format!("key discovery failed: {error}")))
    }

    async fn connect_inner(&mut self) -> Result<(), SshError> {
        // Any cached SFTP channel rode on the previous transport.
        self.sftp = None;
        // Resolve ~/.ssh/config once per session: each of the up-to-10
        // wait_reconnect attempts goes through connect_inner, and the
        // options for a fixed hostname cannot change meaningfully mid-run.
        let options = match &self.openssh_options {
            Some(options) => options.clone(),
            None => {
                let options = OpenSshOptions::lookup(&self.hostname).await;
                self.openssh_options = Some(options.clone());
                options
            }
        };
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
        // known_hosts is deliberately re-read on every connect — accept-new
        // may have appended entries since the previous attempt — but the
        // file I/O runs on the blocking pool, not an async worker.
        let verifier = {
            let policy = self.config.host_key_policy;
            let known_hosts = self.config.known_hosts.clone();
            let host = target_host.clone();
            tokio::task::spawn_blocking(move || {
                HostKeyVerifier::new(policy, known_hosts, host, target_port)
            })
            .await
            .map_err(|error| SshError::Other(format!("known_hosts load failed: {error}")))?
            .map(|verifier| verifier.with_alias(self.hostname.clone()))?
        };
        let handler = ClientHandler { verifier };
        // DNS/TCP/proxy connect + SSH handshake: one deadline (P1 step 24).
        let mut session =
            with_deadline(TimeoutPhase::Connect, self.config.connect_deadline, async {
                if let Some(command) =
                    proxy_command_line(&options, &self.hostname, self.port, &target_user)
                {
                    let stream = ProxyStream::spawn(&command)?;
                    client::connect_stream(conf, stream, handler)
                        .await
                        .map_err(|e| SshError::Transport(e.to_string()))
                } else {
                    let addrs = (target_host.as_str(), target_port);
                    client::connect(conf, addrs, handler)
                        .await
                        .map_err(|e| SshError::Transport(e.to_string()))
                }
            })
            .await?;

        // Auth order mirrors asyncssh's default: ssh-agent first (silently
        // skipped when SSH_AUTH_SOCK is absent or unusable), then
        // IdentityFile / default key files, then one password prompt.
        // Agent + public-key attempts share one absolute deadline (P1 step
        // 25) so many identities or a stalled agent/server cannot multiply
        // timeout duration indefinitely; the interactive password prompt
        // below stays user-paced (excluded), and only the subsequent
        // network `authenticate_password` call is separately deadline-bound.
        let mut last_err = "no SSH private key found (~/.ssh/id_ed25519|id_rsa)".to_string();
        let candidate_keys = self.candidate_keys(&options.identity_files).await?;
        let automatic_ok = with_deadline(
            TimeoutPhase::Authentication,
            self.config.auth_deadline,
            authenticate_automatic(&mut session, &target_user, &candidate_keys, &mut last_err),
        )
        .await?;
        if automatic_ok {
            self.handle = Some(session);
            return Ok(());
        }

        // Non-TTY: fail closed for password (design decision).
        if !std::io::stdin().is_terminal() {
            return Err(SshError::Transport(format!(
                "authentication failed for {}@{}:{} ({last_err}; password authentication requires a TTY)",
                target_user, target_host, target_port
            )));
        }

        // The interactive prompt blocks; keep it off the async worker so
        // other hosts' progress continues on a current_thread runtime.
        let prompt = format!("Password for {}@{}: ", target_user, target_host);
        let mut password = tokio::task::spawn_blocking(move || rpassword::prompt_password(prompt))
            .await
            .map_err(|e| SshError::Transport(format!("password prompt task failed: {e}")))?
            .map_err(|e| SshError::Transport(format!("could not read SSH password: {e}")))?;
        let auth = with_deadline(
            TimeoutPhase::Authentication,
            self.config.auth_deadline,
            async {
                session
                    .authenticate_password(target_user.clone(), &password)
                    .await
                    .map_err(|e| SshError::Transport(e.to_string()))
            },
        )
        .await;
        password.zeroize();
        let auth = auth?;
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
        let mut channel = with_deadline(
            TimeoutPhase::ChannelOpen,
            self.config.channel_open_deadline,
            async {
                handle
                    .channel_open_session()
                    .await
                    .map_err(|e| SshError::Transport(e.to_string()))
            },
        )
        .await?;
        with_deadline(
            TimeoutPhase::Dispatch,
            self.config.dispatch_deadline,
            async {
                channel
                    .exec(true, command)
                    .await
                    .map_err(|e| SshError::Transport(e.to_string()))
            },
        )
        .await?;

        // Command completion keeps its existing, separately configured
        // budget (`ConnectionConfig::timeout`) — unchanged by P1 — but now
        // returns the same typed timeout as every other phase (P1 step 27).
        let deadline = Duration::from_secs_f64(self.config.timeout);
        let stdout_limit = self.config.max_stdout_bytes.get();
        let stderr_limit = self.config.max_stderr_bytes.get();
        let cleanup_deadline = self.config.overflow_cleanup_deadline;
        let collect = async {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let mut code = None;
            loop {
                let Some(msg) = channel.wait().await else {
                    break;
                };
                match msg {
                    ChannelMsg::Data { ref data } => {
                        if let Err(error) = accumulate_or_overflow(
                            &mut stdout,
                            data,
                            stdout_limit,
                            OutputStream::Stdout,
                        ) {
                            close_on_overflow(&channel, cleanup_deadline).await;
                            return Err(error);
                        }
                    }
                    ChannelMsg::ExtendedData { ref data, .. } => {
                        if let Err(error) = accumulate_or_overflow(
                            &mut stderr,
                            data,
                            stderr_limit,
                            OutputStream::Stderr,
                        ) {
                            close_on_overflow(&channel, cleanup_deadline).await;
                            return Err(error);
                        }
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
        with_deadline(TimeoutPhase::Command, deadline, collect).await
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
            let channel = with_deadline(
                TimeoutPhase::ChannelOpen,
                self.config.channel_open_deadline,
                async {
                    handle
                        .channel_open_session()
                        .await
                        .map_err(|e| SshError::Transport(e.to_string()))
                },
            )
            .await?;
            let sftp = with_deadline(
                TimeoutPhase::Dispatch,
                self.config.dispatch_deadline,
                async {
                    channel
                        .request_subsystem(true, "sftp")
                        .await
                        .map_err(|e| SshError::Transport(e.to_string()))?;
                    SftpSession::new(channel.into_stream()).await.map_err(|e| {
                        SshError::Transport(format!("SFTP initialization failed: {e}"))
                    })
                },
            )
            .await?;
            sftp.set_timeout(self.sftp_timeout_secs());
            self.sftp = Some(sftp);
        }

        self.sftp
            .as_ref()
            .ok_or_else(|| SshError::Other("SFTP session was not initialized".into()))
    }

    /// Read many remote files concurrently over the shared SFTP channel,
    /// bounded to `sftp_read_concurrency_limit` in-flight reads (P1 step
    /// 29) instead of one in-flight request per directory entry.
    ///
    /// Results keep the order of `paths`; a missing, unreadable, or
    /// oversized file (or a failed SFTP setup) yields `None` — the same
    /// per-file `.ok()` semantics as awaiting [`SshSession::read_file`] for
    /// each path in turn. One failed read never cancels the others.
    pub(crate) async fn read_files(&mut self, paths: Vec<String>) -> Vec<Option<Vec<u8>>> {
        let count = paths.len();
        let cap = self.config.sftp_read_concurrency_limit.get();
        let limit = self.config.max_sftp_file_bytes.get();
        let deadline = self.config.sftp_operation_deadline;
        let sftp = match self.sftp_session().await {
            Ok(sftp) => sftp,
            Err(_) => return vec![None; count],
        };
        let ctx = BoundedReadCtx {
            sftp,
            limit,
            deadline,
        };
        let indexed: Vec<(usize, Option<Vec<u8>>)> =
            futures_util::stream::iter(paths.into_iter().enumerate().zip(std::iter::repeat(ctx)))
                .map(read_one_bounded)
                .buffer_unordered(cap)
                .collect()
                .await;
        let mut out = vec![None; count];
        for (idx, result) in indexed {
            out[idx] = result;
        }
        out
    }
}

/// Append `chunk` to `buffer` unless doing so would exceed `limit` (P1
/// step 31), checked *before* growth — mirroring `read_bounded`'s SFTP-side
/// bound — so an oversized command output stream is never fully buffered
/// before being rejected. Exactly-at-limit payloads succeed.
fn accumulate_or_overflow(
    buffer: &mut Vec<u8>,
    chunk: &[u8],
    limit: usize,
    stream: OutputStream,
) -> Result<(), SshError> {
    if buffer.len() + chunk.len() > limit {
        return Err(SshError::OutputTooLarge { stream, limit });
    }
    buffer.extend_from_slice(chunk);
    Ok(())
}

/// Best-effort channel close after a P1 step 31 output-limit overflow,
/// bounded by `overflow_cleanup_deadline` so a stalled close request
/// cannot itself pin the host slot indefinitely. The channel is discarded
/// immediately afterward regardless of whether the close completes.
async fn close_on_overflow(channel: &russh::Channel<client::Msg>, deadline: Duration) {
    let _ = tokio::time::timeout(deadline, channel.close()).await;
}

/// Bundles the SFTP session reference plus per-read limit/deadline so
/// `read_files`' `.map()` argument can be a bare function item (`Copy`
/// item, zipped in via `repeat`) rather than a capturing closure — see
/// `repose_core::mock`'s identical note on `buffer_unordered` and
/// higher-ranked lifetime inference.
#[derive(Clone, Copy)]
struct BoundedReadCtx<'a> {
    sftp: &'a SftpSession,
    limit: usize,
    deadline: Duration,
}

async fn read_one_bounded(
    ((idx, path), ctx): ((usize, String), BoundedReadCtx<'_>),
) -> (usize, Option<Vec<u8>>) {
    let result = with_deadline(
        TimeoutPhase::SftpOperation,
        ctx.deadline,
        read_bounded(ctx.sftp, &path, ctx.limit),
    )
    .await
    .ok();
    (idx, result)
}

/// Read `path` incrementally, enforcing `limit` *before* growing the
/// buffer past it (P1 step 28) — checking size only after a whole-file
/// read would already have allocated the oversized payload, defeating the
/// bound. The remote file handle is closed (via [`russh_sftp`]'s `File`
/// `Drop`) on every exit path: success, a real I/O error, or overflow.
async fn read_bounded(sftp: &SftpSession, path: &str, limit: usize) -> Result<Vec<u8>, SshError> {
    use tokio::io::AsyncReadExt;
    let mut file = sftp
        .open(path)
        .await
        .map_err(|e| SshError::Other(format!("SFTP read_file {path}: {e}")))?;
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = file
            .read(&mut chunk)
            .await
            .map_err(|e| SshError::Other(format!("SFTP read_file {path}: {e}")))?;
        if n == 0 {
            break;
        }
        if buffer.len() + n > limit {
            // Overflow: drop the oversized read; `file` closes on drop.
            return Err(SshError::SftpFileTooLarge {
                path: path.to_string(),
                limit,
            });
        }
        buffer.extend_from_slice(&chunk[..n]);
    }
    Ok(buffer)
}

/// Blocking half of `RusshSession::candidate_keys`'s default-key fallback:
/// `HOME` lookup plus a `stat` per candidate filename. Run only via
/// `spawn_blocking` (P1 step 33) — never called directly on the async
/// runtime.
fn default_key_candidates() -> Vec<PathBuf> {
    match std::env::var_os("HOME") {
        Some(home) => default_key_candidates_in(&Path::new(&home).join(".ssh")),
        None => Vec::new(),
    }
}

/// The `HOME`-independent half of `default_key_candidates`, split out so
/// tests can point it at a temporary directory instead of mutating the
/// process-global `HOME` environment variable.
fn default_key_candidates_in(ssh_dir: &Path) -> Vec<PathBuf> {
    let mut keys = Vec::new();
    for name in ["id_ed25519", "id_rsa", "id_ecdsa"] {
        let p = ssh_dir.join(name);
        if p.is_file() {
            keys.push(p);
        }
    }
    keys
}

/// Non-interactive authentication methods only (ssh-agent, then
/// `IdentityFile`/default key files) — the automatic-order portion of
/// `connect_inner`'s auth sequence, extracted so it can be wrapped in one
/// absolute deadline (P1 step 25) without also bounding the interactive
/// password prompt. Returns `Ok(true)` on success, `Ok(false)` if every
/// automatic method was exhausted without success (caller falls through to
/// the password prompt), `Err` only for a hard transport failure.
async fn authenticate_automatic(
    session: &mut client::Handle<ClientHandler>,
    target_user: &str,
    candidate_keys: &[PathBuf],
    last_err: &mut String,
) -> Result<bool, SshError> {
    if try_agent_auth(session, target_user, last_err).await {
        return Ok(true);
    }

    for key_path in candidate_keys {
        let key_pair = match load_unencrypted_key(key_path.clone()).await? {
            Ok(k) => k,
            Err(russh::keys::Error::KeyIsEncrypted) if std::io::stdin().is_terminal() => {
                match load_encrypted_key(key_path).await {
                    Ok(k) => k,
                    Err(e) => {
                        *last_err = e;
                        continue;
                    }
                }
            }
            // Fail-closed parity: without a TTY an encrypted key is
            // skipped instead of blocking on a passphrase prompt.
            Err(e) => {
                *last_err = format!("{}: {e}", key_path.display());
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
                target_user.to_string(),
                PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash),
            )
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;
        if auth.success() {
            return Ok(true);
        }
        *last_err = format!("publickey auth failed with {}", key_path.display());
    }
    Ok(false)
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

/// Load and parse an unencrypted private key on the blocking pool (P1
/// step 33) so file I/O and key parsing do not stall the current-thread
/// runtime. The outer `Result` is a hard failure (the blocking task
/// itself panicked/was cancelled) mapped to a typed `SshError` and
/// propagated; the inner `Result` is an ordinary key-load outcome
/// (missing, malformed, or `KeyIsEncrypted`) that the caller matches on
/// exactly as before, skipping to the next candidate on failure.
async fn load_unencrypted_key(
    key_path: PathBuf,
) -> Result<Result<russh::keys::PrivateKey, russh::keys::Error>, SshError> {
    tokio::task::spawn_blocking(move || load_secret_key(&key_path, None))
        .await
        .map_err(|error| SshError::Other(format!("key load task failed: {error}")))
}

/// Prompt once for the passphrase of an encrypted key and load it.
/// Only called when stdin is a TTY. The blocking prompt runs on the
/// blocking pool so a current_thread runtime keeps making progress.
async fn load_encrypted_key(key_path: &Path) -> Result<russh::keys::PrivateKey, String> {
    let prompt = format!("Enter passphrase for {}: ", key_path.display());
    let path = key_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut passphrase = rpassword::prompt_password(prompt)
            .map_err(|error| format!("could not read passphrase: {error}"))?;
        let loaded = load_secret_key(&path, Some(&passphrase));
        passphrase.zeroize();
        loaded.map_err(|error| format!("{}: {error}", path.display()))
    })
    .await
    .map_err(|error| format!("passphrase prompt task failed: {error}"))?
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
        let deadline = self.config.sftp_operation_deadline;
        let sftp = self.sftp_session().await?;
        with_deadline(TimeoutPhase::SftpOperation, deadline, async {
            let entries = sftp
                .read_dir(path)
                .await
                .map_err(|e| SshError::Other(format!("SFTP listdir {path}: {e}")))?;
            Ok(entries.map(|entry| entry.file_name()).collect())
        })
        .await
    }

    async fn readlink(&mut self, path: &str) -> Result<Option<String>, SshError> {
        let deadline = self.config.sftp_operation_deadline;
        let sftp = self.sftp_session().await?;
        with_deadline(TimeoutPhase::SftpOperation, deadline, async {
            sftp.read_link(path)
                .await
                .map(Some)
                .map_err(|e| SshError::Other(format!("SFTP readlink {path}: {e}")))
        })
        .await
    }

    async fn read_file(&mut self, path: &str) -> Result<Vec<u8>, SshError> {
        let limit = self.config.max_sftp_file_bytes.get();
        let deadline = self.config.sftp_operation_deadline;
        let sftp = self.sftp_session().await?;
        with_deadline(
            TimeoutPhase::SftpOperation,
            deadline,
            read_bounded(sftp, path, limit),
        )
        .await
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
        let channel = with_deadline(
            TimeoutPhase::ChannelOpen,
            self.config.channel_open_deadline,
            async {
                handle
                    .channel_open_session()
                    .await
                    .map_err(|e| SshError::Transport(e.to_string()))
            },
        )
        .await?;
        with_deadline(
            TimeoutPhase::Dispatch,
            self.config.dispatch_deadline,
            async {
                channel
                    .exec(true, command)
                    .await
                    .map_err(|e| SshError::Transport(e.to_string()))
            },
        )
        .await?;
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
    use repose_core::error::{OutputStream, SshError, TimeoutPhase};
    use repose_core::traits::SshSession;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use super::{ClientHandler, RusshSession};
    use crate::hostkey::HostKeyVerifier;
    use crate::openssh_config::OpenSshOptions;
    use repose_core::config::HostKeyPolicy;
    use russh::client::Handler;
    use russh::keys::ssh_key::PublicKey;

    const TEST_KEY: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti";

    #[tokio::test]
    async fn with_deadline_returns_ok_unchanged_for_a_fast_future() {
        let result = super::with_deadline(TimeoutPhase::Connect, Duration::from_secs(5), async {
            Ok::<_, SshError>(42)
        })
        .await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn with_deadline_returns_a_typed_timeout_for_a_stalled_future() {
        let start = Instant::now();
        let deadline = Duration::from_millis(50);
        let result = super::with_deadline(
            TimeoutPhase::Dispatch,
            deadline,
            std::future::pending::<Result<(), SshError>>(),
        )
        .await;
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "deadline must fire promptly, not hang"
        );
        assert_eq!(
            result,
            Err(SshError::Timeout {
                phase: TimeoutPhase::Dispatch,
                deadline
            })
        );
    }

    #[tokio::test]
    async fn with_deadline_does_not_delay_an_independent_sibling_future() {
        // P1 step 24 verify: a stalled phase must not block an unrelated
        // concurrent timer/host operation. `tokio::join!` only returns once
        // *both* branches finish, so the sibling records its own
        // completion time rather than relying on the join's return time.
        let stalled = super::with_deadline(
            TimeoutPhase::Connect,
            Duration::from_millis(300),
            std::future::pending::<Result<(), SshError>>(),
        );
        let sibling_done_at = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sibling_done_at_write = sibling_done_at.clone();
        let start = Instant::now();
        let sibling = async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            *sibling_done_at_write.lock().unwrap() = Some(start.elapsed());
        };
        let (stalled_result, ()) = tokio::join!(stalled, sibling);
        let sibling_elapsed = sibling_done_at.lock().unwrap().expect("sibling recorded");
        assert!(
            sibling_elapsed < Duration::from_millis(300),
            "the sibling must complete on its own schedule, not wait for the stalled phase"
        );
        assert!(matches!(
            stalled_result,
            Err(SshError::Timeout {
                phase: TimeoutPhase::Connect,
                ..
            })
        ));
    }

    /// P1 step 24: a server that accepts the TCP connection but never
    /// speaks SSH must not hang `connect()` forever — a real (not mocked)
    /// deterministic stall, using a plain `TcpListener`.
    #[tokio::test]
    async fn connect_deadline_fires_against_a_server_that_never_speaks_ssh() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
        let port = listener.local_addr().expect("local addr").port();
        std::thread::spawn(move || {
            // Accept and hold the connection open without ever writing the
            // SSH identification string, so the client's handshake blocks
            // on read indefinitely without the connect deadline.
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        std::thread::sleep(Duration::from_secs(30));
                        drop(stream);
                    }
                    Err(_) => break,
                }
            }
        });

        let config = ConnectionConfig {
            connect_deadline: Duration::from_millis(200),
            ..ConnectionConfig::default()
        };
        let mut session = RusshSession::new("127.0.0.1", port, "user", config);
        let start = Instant::now();
        let err = session
            .connect()
            .await
            .expect_err("handshake never completes");
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "connect deadline must fire well before the 30s fake-server sleep"
        );
        assert_eq!(
            err,
            SshError::Timeout {
                phase: TimeoutPhase::Connect,
                deadline: Duration::from_millis(200)
            }
        );
    }

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

    // P1 step 31: `accumulate_or_overflow` is the pure boundary check behind
    // the stdout/stderr byte caps in `run_inner`'s collect loop. It is
    // tested directly here (no live SSH channel required); the full
    // channel-close/exit-status/session-reuse behavior around it is
    // covered by the live-gated tests in `ssh_integration.rs`.
    #[test]
    fn accumulate_or_overflow_accepts_an_empty_chunk_into_an_empty_buffer() {
        let mut buffer = Vec::new();
        assert_eq!(
            super::accumulate_or_overflow(&mut buffer, b"", 0, OutputStream::Stdout),
            Ok(())
        );
        assert!(buffer.is_empty());
    }

    #[test]
    fn accumulate_or_overflow_accepts_a_chunk_landing_exactly_at_the_limit() {
        let mut buffer = Vec::new();
        assert_eq!(
            super::accumulate_or_overflow(&mut buffer, b"hello", 5, OutputStream::Stdout),
            Ok(())
        );
        assert_eq!(buffer, b"hello");
    }

    #[test]
    fn accumulate_or_overflow_rejects_one_byte_over_the_limit() {
        let mut buffer = Vec::new();
        let error = super::accumulate_or_overflow(&mut buffer, b"hello!", 5, OutputStream::Stdout)
            .unwrap_err();
        assert_eq!(
            error,
            SshError::OutputTooLarge {
                stream: OutputStream::Stdout,
                limit: 5,
            }
        );
        // Rejected on arrival: the buffer must not retain the oversized chunk.
        assert!(buffer.is_empty());
    }

    #[test]
    fn accumulate_or_overflow_rejects_once_prior_chunks_already_reached_the_limit() {
        let mut buffer = b"abc".to_vec();
        let error =
            super::accumulate_or_overflow(&mut buffer, b"d", 3, OutputStream::Stderr).unwrap_err();
        assert_eq!(
            error,
            SshError::OutputTooLarge {
                stream: OutputStream::Stderr,
                limit: 3,
            }
        );
        assert_eq!(
            buffer, b"abc",
            "the previously accepted bytes are unchanged"
        );
    }

    // P1 step 33: default key-file discovery and unencrypted key loading
    // now run via `spawn_blocking`. `default_key_candidates_in` is tested
    // directly against a temporary directory (not the process-global
    // `HOME`, which parallel tests must not mutate); `candidate_keys`'s
    // two filesystem-free fast paths are tested through the real method.

    #[tokio::test]
    async fn candidate_keys_prefers_an_explicit_identity_over_everything_else() {
        let session = RusshSession::new("host", 22, "root", ConnectionConfig::default())
            .with_identity(PathBuf::from("/explicit/identity"));
        let configured = vec![PathBuf::from("/configured/key")];
        let keys = session.candidate_keys(&configured).await.unwrap();
        assert_eq!(keys, vec![PathBuf::from("/explicit/identity")]);
    }

    #[tokio::test]
    async fn candidate_keys_uses_configured_identity_files_when_no_explicit_identity() {
        let session = RusshSession::new("host", 22, "root", ConnectionConfig::default());
        let configured = vec![
            PathBuf::from("/configured/a"),
            PathBuf::from("/configured/b"),
        ];
        let keys = session.candidate_keys(&configured).await.unwrap();
        assert_eq!(keys, configured);
    }

    #[test]
    fn default_key_candidates_in_returns_only_existing_default_names_in_order() {
        let dir = tempfile::tempdir().expect("temp ssh dir should be created");
        std::fs::write(dir.path().join("id_ed25519"), b"fake key").unwrap();
        std::fs::write(dir.path().join("id_ecdsa"), b"fake key").unwrap();
        // id_rsa is deliberately absent.
        let keys = super::default_key_candidates_in(dir.path());
        assert_eq!(
            keys,
            vec![dir.path().join("id_ed25519"), dir.path().join("id_ecdsa")]
        );
    }

    #[test]
    fn default_key_candidates_in_an_empty_directory_returns_no_candidates() {
        let dir = tempfile::tempdir().expect("temp ssh dir should be created");
        assert!(super::default_key_candidates_in(dir.path()).is_empty());
    }

    /// P1 step 33 verify: a deliberately slow blocking task (representing
    /// the shape of `default_key_candidates`/`load_unencrypted_key`'s
    /// filesystem/crypto work) must not delay an independent async
    /// sibling — the same `spawn_blocking` isolation both functions rely
    /// on, demonstrated directly since real key I/O cannot be forced slow
    /// deterministically in a unit test.
    #[tokio::test]
    async fn spawn_blocking_key_work_does_not_delay_an_independent_sibling() {
        let start = Instant::now();
        let blocked = tokio::task::spawn_blocking(|| {
            std::thread::sleep(Duration::from_millis(300));
        });
        let sibling_done_at = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sibling_done_at_write = sibling_done_at.clone();
        let sibling = async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            *sibling_done_at_write.lock().unwrap() = Some(start.elapsed());
        };
        let (blocked_result, ()) = tokio::join!(blocked, sibling);
        blocked_result.expect("blocking task should not panic");
        let sibling_elapsed = sibling_done_at.lock().unwrap().expect("sibling recorded");
        assert!(
            sibling_elapsed < Duration::from_millis(300),
            "the sibling must complete on its own schedule, not wait for the blocked task"
        );
    }

    // P1 steps 34–35: `ClientHandler::check_server_key` awaits `accept-new`
    // persistence via `spawn_blocking` and trusts a first-contact key only
    // after a successful durable write.

    #[tokio::test]
    async fn check_server_key_accepts_first_contact_only_after_a_durable_write() {
        let key = PublicKey::from_openssh(TEST_KEY).expect("test key should parse");
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let path = temp.path().join("known_hosts");
        let verifier =
            HostKeyVerifier::new(HostKeyPolicy::AcceptNew, Some(path.clone()), "host", 22)
                .expect("verifier should build");
        let mut handler = ClientHandler { verifier };

        assert!(
            handler
                .check_server_key(&key)
                .await
                .expect("check_server_key should not error"),
            "first contact should be accepted once persisted"
        );
        assert!(
            std::fs::read_to_string(&path)
                .expect("known_hosts should have been written")
                .contains("host "),
            "the key must actually be recorded, not just trusted in memory"
        );

        // A second check within the same session sees the in-memory entry
        // recorded by the first call and does not need to write again.
        assert!(
            handler
                .check_server_key(&key)
                .await
                .expect("check_server_key should not error")
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn check_server_key_rejects_first_contact_when_persistence_fails() {
        use std::os::unix::fs::PermissionsExt;
        let key = PublicKey::from_openssh(TEST_KEY).expect("test key should parse");
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let dir = temp.path().join("readonly");
        std::fs::create_dir(&dir).expect("readonly dir should be created");
        let path = dir.join("known_hosts");
        let verifier =
            HostKeyVerifier::new(HostKeyPolicy::AcceptNew, Some(path.clone()), "host", 22)
                .expect("verifier should build");
        let mut handler = ClientHandler { verifier };
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555))
            .expect("directory should become read-only");

        assert!(
            !handler
                .check_server_key(&key)
                .await
                .expect("check_server_key should not error"),
            "a persistence failure must reject the session (fail-closed), not trust the key"
        );
        assert!(!path.exists(), "the key must not be recorded on failure");

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755))
            .expect("permissions should be restorable for cleanup");
    }

    /// P1 step 35 verify: a slow `accept-new` persistence write must not
    /// delay an independent sibling connection/timer — the same
    /// `spawn_blocking` isolation guarantee already demonstrated for key
    /// loading (P1 step 33), applied to `check_server_key`'s persistence
    /// path specifically.
    #[tokio::test]
    async fn check_server_key_persistence_does_not_delay_an_independent_sibling() {
        let key = PublicKey::from_openssh(TEST_KEY).expect("test key should parse");
        let temp = tempfile::tempdir().expect("temp dir should be created");
        let path = temp.path().join("known_hosts");
        let verifier = HostKeyVerifier::new(HostKeyPolicy::AcceptNew, Some(path), "host", 22)
            .expect("verifier should build");
        let mut handler = ClientHandler { verifier };

        let start = Instant::now();
        let check = handler.check_server_key(&key);
        let sibling_done_at = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sibling_done_at_write = sibling_done_at.clone();
        let sibling = async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            *sibling_done_at_write.lock().unwrap() = Some(start.elapsed());
        };
        let (result, ()) = tokio::join!(check, sibling);
        assert!(result.expect("check_server_key should not error"));
        let sibling_elapsed = sibling_done_at.lock().unwrap().expect("sibling recorded");
        assert!(
            sibling_elapsed < Duration::from_secs(1),
            "the sibling must complete on its own schedule"
        );
    }
}
