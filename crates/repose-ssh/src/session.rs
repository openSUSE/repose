//! russh-backed [`SshSession`] (single SSH backend).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use repose_core::config::{ConnectionConfig, HostKeyPolicy};
use repose_core::error::SshError;
use repose_core::shell::quote;
use repose_core::traits::SshSession;
use russh::client::{self, Handler};
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};
use russh::{ChannelMsg, Disconnect, Preferred};

struct ClientHandler {
    policy: HostKeyPolicy,
}

impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Full accept-new known_hosts matrix is PR9; v1:
        // yes/accept-new → accept (caller may still pin via custom known_hosts later)
        // no/off → accept all
        match self.policy {
            HostKeyPolicy::Yes
            | HostKeyPolicy::AcceptNew
            | HostKeyPolicy::No
            | HostKeyPolicy::Off => Ok(true),
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
            identity: None,
        }
    }

    /// Prefer a specific private key path (default: try `~/.ssh/id_ed25519` then `id_rsa`).
    #[must_use]
    pub fn with_identity(mut self, path: PathBuf) -> Self {
        self.identity = Some(path);
        self
    }

    fn candidate_keys(&self) -> Vec<PathBuf> {
        if let Some(p) = &self.identity {
            return vec![p.clone()];
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
        let conf = client::Config {
            inactivity_timeout: Some(Duration::from_secs_f64(self.config.timeout.max(5.0))),
            preferred: Preferred::default(),
            ..Default::default()
        };
        let conf = Arc::new(conf);
        let handler = ClientHandler {
            policy: self.config.host_key_policy,
        };
        let addrs = (self.hostname.as_str(), self.port);
        let mut session = client::connect(conf, addrs, handler)
            .await
            .map_err(|e| SshError::Transport(e.to_string()))?;

        let keys = self.candidate_keys();
        if keys.is_empty() {
            return Err(SshError::Transport(
                "no SSH private key found (~/.ssh/id_ed25519|id_rsa)".into(),
            ));
        }

        let mut last_err = String::new();
        for key_path in keys {
            let key_pair = match load_secret_key(&key_path, None) {
                Ok(k) => k,
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
                    self.username.clone(),
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
        Err(SshError::Transport(format!(
            "authentication failed for {}@{}:{} ({last_err})",
            self.username, self.hostname, self.port
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
                        code = Some(exit_status as i32);
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
}

#[async_trait]
impl SshSession for RusshSession {
    async fn connect(&mut self) -> Result<(), SshError> {
        if self.handle.is_some() {
            return Ok(());
        }
        self.connect_inner().await
    }

    async fn run(&mut self, command: &str) -> Result<(String, String, i32), SshError> {
        self.run_inner(command).await
    }

    async fn listdir(&mut self, path: &str) -> Result<Vec<String>, SshError> {
        let cmd = format!("ls -1A {}", quote(path));
        let (stdout, _, code) = self.run_inner(&cmd).await?;
        if code != 0 {
            return Err(SshError::Other(format!("listdir {path} exit {code}")));
        }
        Ok(stdout
            .lines()
            .filter(|l| !l.is_empty() && *l != "." && *l != "..")
            .map(str::to_string)
            .collect())
    }

    async fn readlink(&mut self, path: &str) -> Result<Option<String>, SshError> {
        let cmd = format!("readlink {}", quote(path));
        let (stdout, _, code) = self.run_inner(&cmd).await?;
        if code != 0 {
            return Ok(None);
        }
        let s = stdout.trim();
        if s.is_empty() {
            Ok(None)
        } else {
            Ok(Some(s.to_string()))
        }
    }

    async fn read_file(&mut self, path: &str) -> Result<Vec<u8>, SshError> {
        let cmd = format!("cat {}", quote(path));
        let (stdout, _, code) = self.run_inner(&cmd).await?;
        if code != 0 {
            return Err(SshError::Other(format!("read_file {path} exit {code}")));
        }
        Ok(stdout.into_bytes())
    }

    async fn close(&mut self) -> Result<(), SshError> {
        if let Some(handle) = self.handle.take() {
            let _ = handle
                .disconnect(Disconnect::ByApplication, "", "English")
                .await;
        }
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.handle.is_some()
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
        while count < retry {
            let sleep_secs = if backoff {
                2 * (timeout_secs + 5 * u64::from(count))
            } else {
                timeout_secs
            };
            tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
            match self.connect_inner().await {
                Ok(()) => return true,
                Err(_) => count += 1,
            }
        }
        self.is_active()
    }
}
