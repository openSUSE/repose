//! [`Host`] / [`HostGroup`] over [`crate::session::RusshSession`].

use std::collections::BTreeMap;
use std::time::Instant;

use async_trait::async_trait;
use repose_core::config::ConnectionConfig;
use repose_core::error::SshError;
use repose_core::host_parse::HostSpec;
use repose_core::product_parse::{parse_os_release, parse_prod_xml, TRANSACTIONAL_CONF_PATHS};
use repose_core::repo_parse::parse_repositories;
use repose_core::traits::{Host, HostGroup, SshSession};
use repose_core::types::{
    repositories_from_raw, OutEntry, Product, Repositories, Repository, System,
};

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
                // Contract: timeout still appends exitcode -1.
                self.out.push((
                    command.to_string(),
                    String::new(),
                    msg.clone(),
                    -1,
                    start.elapsed().as_secs(),
                ));
                Ok(())
            }
            Err(e) => {
                // Prefer append when possible.
                self.out.push((
                    command.to_string(),
                    String::new(),
                    e.to_string(),
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
        let _ = self.session.run("zypper -x lr").await;
        // Prefer structured: run via Host::run to populate out, or session directly.
        match self.session.run("zypper -x lr").await {
            Ok((stdout, _, 0 | 106 | 6)) => {
                self.raw_repos = Some(parse_repositories(&stdout));
                Ok(())
            }
            Ok((_, stderr, code)) => Err(SshError::Other(format!(
                "zypper -x lr failed exit {code}: {stderr}"
            ))),
            Err(e) => Err(e),
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
            let _ = self.read_products().await;
        }
        Ok(ok)
    }
}

async fn discover_system(session: &mut RusshSession, hostname: &str) -> Result<System, SshError> {
    // transactional detect
    let mut transactional = false;
    for path in TRANSACTIONAL_CONF_PATHS {
        if session.read_file(path).await.is_ok() {
            transactional = true;
            break;
        }
    }

    // Try products.d
    match session.listdir("/etc/products.d").await {
        Ok(files) => {
            let mut prods: Vec<String> =
                files.into_iter().filter(|f| f.ends_with(".prod")).collect();
            let base_link = session
                .readlink("/etc/products.d/baseproduct")
                .await
                .ok()
                .flatten();
            let base_name = base_link.as_deref().and_then(|p| {
                Path::new(p)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
            });
            if let Some(ref bn) = base_name {
                prods.retain(|p| p != bn);
            }
            let base_file = base_name.unwrap_or_else(|| prods.first().cloned().unwrap_or_default());
            if base_file.is_empty() {
                return fallback_os_release(session, hostname, transactional).await;
            }
            let base_path = format!("/etc/products.d/{base_file}");
            let data = session.read_file(&base_path).await?;
            let xml = String::from_utf8_lossy(&data);
            let base = parse_prod_xml(&xml, &base_file)
                .ok_or_else(|| SshError::Other(format!("malformed base product {base_file}")))?;
            let mut addons = Vec::new();
            for f in prods {
                if f.ends_with("-migration.prod") || f.contains("-migration.") {
                    continue;
                }
                // skip *-migration product names
                let path = format!("/etc/products.d/{f}");
                if let Ok(bytes) = session.read_file(&path).await {
                    let x = String::from_utf8_lossy(&bytes);
                    if let Some(p) = parse_prod_xml(&x, &f) {
                        if !p.name.ends_with("-migration") {
                            addons.push(p);
                        }
                    }
                }
            }
            Ok(System {
                base,
                addons,
                transactional,
            })
        }
        Err(_) => fallback_os_release(session, hostname, transactional).await,
    }
}

async fn fallback_os_release(
    session: &mut RusshSession,
    _hostname: &str,
    transactional: bool,
) -> Result<System, SshError> {
    match session.read_file("/etc/os-release").await {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let (name, version, arch) = parse_os_release(&text);
            Ok(System {
                base: Product {
                    name,
                    version,
                    arch,
                },
                addons: vec![],
                transactional,
            })
        }
        Err(_) => Ok(System {
            base: Product {
                name: "rhel".into(),
                version: "6".into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional,
        }),
    }
}

use std::path::Path;

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

    async fn connect_and_prune(&mut self) {
        let keys: Vec<_> = self.hosts.keys().cloned().collect();
        for key in keys {
            let fail = {
                let h = self.hosts.get_mut(&key).expect("key");
                h.connect().await.is_err()
            };
            if fail {
                self.hosts.remove(&key);
            }
        }
    }

    async fn read_products(&mut self) {
        for h in self.hosts.values_mut() {
            let _ = h.read_products().await;
        }
    }

    async fn read_repos(&mut self) {
        for h in self.hosts.values_mut() {
            let _ = h.read_repos().await;
        }
    }

    async fn parse_repos(&mut self) {
        for h in self.hosts.values_mut() {
            let _ = h.parse_repos().await;
        }
    }

    async fn run_all(&mut self, cmd: &str) {
        let keys: Vec<_> = self.hosts.keys().cloned().collect();
        for key in keys {
            if let Some(h) = self.hosts.get_mut(&key) {
                let _ = h.run(cmd).await;
            }
        }
    }

    async fn close(&mut self) {
        for h in self.hosts.values_mut() {
            let _ = h.close().await;
        }
    }
}
