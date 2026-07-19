//! [`Host`] / [`HostGroup`] over [`crate::session::RusshSession`].

use std::collections::BTreeMap;
use std::time::Instant;

use async_trait::async_trait;
use futures_util::future::join_all;
use repose_core::config::ConnectionConfig;
use repose_core::error::SshError;
use repose_core::host_parse::HostSpec;
use repose_core::product_parse::{parse_system, ProdFile, TRANSACTIONAL_CONF_PATHS};
use repose_core::repo_parse::parse_repositories;
use repose_core::traits::{Host, HostGroup, SshSession};
use repose_core::types::{repositories_from_raw, OutEntry, Repositories, Repository, System};

use crate::session::RusshSession;

/// Live target host (Python `AsyncTarget`).
pub struct RusshHost {
    key: String,
    #[allow(dead_code)]
    hostname: String,
    #[allow(dead_code)]
    port: u16,
    session: RusshSession,
    connected: bool,
    products: Option<System>,
    raw_repos: Option<Vec<Repository>>,
    repos: Option<Repositories>,
    out: Vec<OutEntry>,
}

impl RusshHost {
    #[must_use]
    pub fn from_spec(spec: HostSpec, config: ConnectionConfig) -> Self {
        let session = RusshSession::new(
            spec.hostname.clone(),
            spec.port,
            spec.username.clone(),
            config,
        );
        Self {
            key: spec.key,
            hostname: spec.hostname,
            port: spec.port,
            session,
            connected: false,
            products: None,
            raw_repos: None,
            repos: None,
            out: Vec::new(),
        }
    }

    #[must_use]
    pub fn key_str(&self) -> &str {
        &self.key
    }
}

#[async_trait]
impl Host for RusshHost {
    fn key(&self) -> &str {
        &self.key
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn products(&self) -> Option<&System> {
        self.products.as_ref()
    }

    fn raw_repos(&self) -> Option<&[Repository]> {
        self.raw_repos.as_deref()
    }

    fn repos(&self) -> Option<&Repositories> {
        self.repos.as_ref()
    }

    fn out(&self) -> &[OutEntry] {
        &self.out
    }

    async fn connect(&mut self) -> Result<(), SshError> {
        match self.session.connect().await {
            Ok(()) => {
                self.connected = true;
                Ok(())
            }
            Err(e) => {
                self.connected = false;
                Err(e)
            }
        }
    }

    async fn close(&mut self) -> Result<(), SshError> {
        let r = self.session.close().await;
        self.connected = false;
        r
    }

    async fn run(&mut self, command: &str) -> Result<(), SshError> {
        if !self.connected {
            // Python parity (async_target.py run): a failed dispatch still
            // records a synthetic out entry (rc -1, empty streams) so the
            // report shows this command as FAILED instead of desyncing.
            self.out
                .push((command.to_string(), String::new(), String::new(), -1, 0));
            return Err(SshError::NotConnected(self.key.clone()));
        }
        let start = Instant::now();
        match self.session.run(command).await {
            Ok((stdout, stderr, exitcode)) => {
                let runtime = start.elapsed().as_secs();
                self.out
                    .push((command.to_string(), stdout, stderr, exitcode, runtime));
                Ok(())
            }
            Err(SshError::Transport(msg)) if msg.contains("timed out") => {
                // Python parity: a timeout appends (command, "", "", -1) —
                // the diagnostics go to the log, not the entry's stderr.
                log::error!("{}: command {command:?} timed out", self.key);
                self.out.push((
                    command.to_string(),
                    String::new(),
                    String::new(),
                    -1,
                    start.elapsed().as_secs(),
                ));
                Ok(())
            }
            Err(e) => {
                // Python parity: generic failures also append empty streams
                // with rc -1 and log the reason.
                log::error!("{}: failed to run command {command:?}: {e}", self.key);
                self.out.push((
                    command.to_string(),
                    String::new(),
                    String::new(),
                    -1,
                    start.elapsed().as_secs(),
                ));
                Ok(())
            }
        }
    }

    async fn read_products(&mut self) -> Result<(), SshError> {
        if !self.connected {
            self.connect().await?;
        }
        self.products = Some(discover_system(&mut self.session, &self.hostname).await?);
        Ok(())
    }

    async fn read_repos(&mut self) -> Result<(), SshError> {
        if !self.connected {
            return Ok(()); // Python: debug return without raise
        }
        // Python parity: read_repos goes through run(), so the zypper call
        // is recorded as an out entry like any other command.
        self.run("zypper -x lr").await?;
        let (stdout, stderr, exitcode) = match self.out.last() {
            Some((_, stdout, stderr, exitcode, _)) => (stdout.clone(), stderr.clone(), *exitcode),
            None => {
                return Err(SshError::Other(
                    "no output recorded for zypper -x lr".into(),
                ))
            }
        };
        if matches!(exitcode, 0 | 106 | 6) {
            self.raw_repos = Some(parse_repositories(&stdout));
            Ok(())
        } else {
            Err(SshError::Other(format!(
                "zypper -x lr failed exit {exitcode}: {stderr}"
            )))
        }
    }

    async fn parse_repos(&mut self) -> Result<(), SshError> {
        if self.products.is_none() {
            self.read_products().await?;
        }
        if self.raw_repos.is_none() {
            self.read_repos().await?;
        }
        let arch = self
            .products
            .as_ref()
            .map(|p| p.arch().to_string())
            .unwrap_or_else(|| "unknown".into());
        let raw = self.raw_repos.clone().unwrap_or_default();
        self.repos = Some(repositories_from_raw(&raw, &arch));
        Ok(())
    }

    async fn reboot(&mut self, command: &str) -> Result<bool, SshError> {
        let before = self.session.boot_id().await;
        self.session.fire_and_forget(command).await?;
        self.connected = false;
        let ok = self.session.wait_reconnect(10, 10, true).await;
        self.connected = ok;
        if ok {
            let after = self.session.boot_id().await;
            if !before.is_empty() && !after.is_empty() && before == after {
                log::warn!("boot_id unchanged after reboot on {}", self.key);
            }
            // Python parity: reboot does NOT re-read products itself; the
            // command layer (`reboot_and_verify`) re-reads them afterwards.
        }
        Ok(ok)
    }
}

/// Fetch the inputs `parse_system` needs over SSH, then decide purely.
///
/// All SSH lives here; all parsing/branching is in `repose_core::parse_system`,
/// which is the behavioral reference for the products.d / os-release / rhel6
/// discovery. `listdir` succeeding (even empty) is the SUSE path; an
/// unresolved `baseproduct` symlink is an error (matching Python), not a
/// silent os-release fallback.
async fn discover_system(session: &mut RusshSession, _hostname: &str) -> Result<System, SshError> {
    match session.listdir("/etc/products.d").await {
        Ok(listing) => {
            let prod_names: Vec<String> = listing
                .into_iter()
                .filter(|f| f.ends_with(".prod"))
                .collect();

            let base_link = session
                .readlink("/etc/products.d/baseproduct")
                .await
                .ok()
                .flatten();
            let base_file = base_link
                .as_deref()
                .map(|raw| raw.rsplit_once('/').map_or(raw, |(_, t)| t).to_string());

            // Read the base via the symlink target, even if absent from the listing.
            let base_xml = match &base_file {
                Some(bf) => session
                    .read_file(&format!("/etc/products.d/{bf}"))
                    .await
                    .ok()
                    .map(|b| String::from_utf8_lossy(&b).into_owned()),
                None => None,
            };

            let mut prod_files = Vec::new();
            for f in prod_names {
                if base_file.as_deref() == Some(f.as_str()) {
                    continue; // base already read; not an addon candidate
                }
                let xml = session
                    .read_file(&format!("/etc/products.d/{f}"))
                    .await
                    .ok()
                    .map(|b| String::from_utf8_lossy(&b).into_owned());
                prod_files.push(ProdFile { filename: f, xml });
            }

            // Transactional is probed only on the SUSE path (Python parity).
            let mut transactional = false;
            for path in TRANSACTIONAL_CONF_PATHS {
                if session.read_file(path).await.is_ok() {
                    transactional = true;
                    break;
                }
            }

            parse_system(
                Some(&prod_files),
                base_link.as_deref(),
                base_xml.as_deref(),
                None,
                transactional,
            )
            .map_err(|e| SshError::Other(e.to_string()))
        }
        Err(_) => {
            let os = session
                .read_file("/etc/os-release")
                .await
                .ok()
                .map(|b| String::from_utf8_lossy(&b).into_owned());
            parse_system(None, None, None, os.as_deref(), false)
                .map_err(|e| SshError::Other(e.to_string()))
        }
    }
}

/// Multi-host group.
pub struct RusshHostGroup {
    hosts: BTreeMap<String, RusshHost>,
}

impl RusshHostGroup {
    #[must_use]
    pub fn new() -> Self {
        Self {
            hosts: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, host: RusshHost) {
        self.hosts.insert(host.key.clone(), host);
    }

    pub fn from_targets(specs: Vec<HostSpec>, config: ConnectionConfig) -> Self {
        let mut g = Self::new();
        for s in specs {
            g.insert(RusshHost::from_spec(s, config.clone()));
        }
        g
    }
}

impl Default for RusshHostGroup {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HostGroup for RusshHostGroup {
    fn keys(&self) -> Vec<String> {
        self.hosts.keys().cloned().collect()
    }

    fn get(&self, key: &str) -> Option<&dyn Host> {
        self.hosts.get(key).map(|h| h as &dyn Host)
    }

    fn get_mut(&mut self, key: &str) -> Option<&mut dyn Host> {
        self.hosts.get_mut(key).map(|h| h as &mut dyn Host)
    }

    // Group operations fan out per host concurrently (Python
    // `AsyncHostGroup` used one `asyncio.TaskGroup` per operation) while
    // isolating per-host failures. `join_all` preserves input order, and
    // the hosts map is a `BTreeMap`, so results stay in key order.
    async fn connect_and_prune(&mut self) {
        let failed: Vec<String> =
            join_all(self.hosts.iter_mut().map(|(key, host)| async move {
                host.connect().await.is_err().then(|| key.clone())
            }))
            .await
            .into_iter()
            .flatten()
            .collect();
        for key in failed {
            self.hosts.remove(&key);
        }
    }

    async fn read_products(&mut self) {
        join_all(self.hosts.values_mut().map(|host| async move {
            let _ = host.read_products().await;
        }))
        .await;
    }

    async fn read_repos(&mut self) {
        join_all(self.hosts.values_mut().map(|host| async move {
            let _ = host.read_repos().await;
        }))
        .await;
    }

    async fn parse_repos(&mut self) {
        join_all(self.hosts.values_mut().map(|host| async move {
            let _ = host.parse_repos().await;
        }))
        .await;
    }

    async fn run_all(&mut self, cmd: &str) {
        join_all(self.hosts.values_mut().map(|host| async move {
            let _ = host.run(cmd).await;
        }))
        .await;
    }

    async fn close(&mut self) {
        join_all(self.hosts.values_mut().map(|host| async move {
            let _ = host.close().await;
        }))
        .await;
    }
}

#[cfg(test)]
mod tests {
    use repose_core::config::ConnectionConfig;
    use repose_core::error::SshError;
    use repose_core::host_parse::HostSpec;
    use repose_core::traits::Host;

    use super::RusshHost;

    fn disconnected_host() -> RusshHost {
        RusshHost::from_spec(
            HostSpec {
                key: "h1".into(),
                hostname: "h1.example".into(),
                port: 22,
                username: "root".into(),
            },
            ConnectionConfig::default(),
        )
    }

    #[tokio::test]
    async fn run_on_a_disconnected_host_appends_a_synthetic_out_entry() {
        let mut host = disconnected_host();
        let err = host.run("uptime").await.unwrap_err();
        assert!(matches!(err, SshError::NotConnected(_)));
        // Python parity: the failed dispatch is visible in the report as
        // (command, "", "", -1).
        assert_eq!(
            host.out(),
            &[("uptime".to_string(), String::new(), String::new(), -1, 0)]
        );
    }

    #[tokio::test]
    async fn read_repos_on_a_disconnected_host_is_a_silent_no_op() {
        let mut host = disconnected_host();
        assert!(host.read_repos().await.is_ok());
        assert!(host.out().is_empty());
        assert!(host.raw_repos().is_none());
    }
}
