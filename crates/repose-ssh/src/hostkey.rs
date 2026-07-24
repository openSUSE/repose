//! OpenSSH-style `known_hosts` verification for the single russh backend.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use repose_core::config::HostKeyPolicy;
use repose_core::error::SshError;
use russh::keys::ssh_key::PublicKey;

static KNOWN_HOSTS_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone)]
struct KnownHostEntry {
    patterns: Vec<String>,
    key: String,
    marker: Option<String>,
}

impl KnownHostEntry {
    fn matches(&self, names: &[String]) -> bool {
        let mut matched = false;
        for pattern in &self.patterns {
            // `HashKnownHosts` entry: `|1|base64(salt)|base64(hmac)`.
            // Hashed patterns support neither negation nor globs; the
            // hashed name is the same lookup key used for plain
            // patterns (`[host]:port` for non-22, plain host otherwise).
            if let Some(salt_and_hash) = pattern.strip_prefix("|1|") {
                if names
                    .iter()
                    .any(|name| hashed_pattern_matches(salt_and_hash, name))
                {
                    matched = true;
                }
                continue;
            }
            let (negated, pattern) = pattern
                .strip_prefix('!')
                .map_or((false, pattern.as_str()), |pattern| (true, pattern));
            if names
                .iter()
                .any(|name| crate::glob::glob_matches(pattern, name))
            {
                if negated {
                    return false;
                }
                matched = true;
            }
        }
        matched
    }

    fn is_revoked(&self) -> bool {
        self.marker.as_deref() == Some("@revoked")
    }

    // `@cert-authority` entries are intentionally not interpreted (repose
    // does not authenticate hosts via OpenSSH CA certificates); they are
    // neither trusted pins nor revocations, so they are simply ignored.
    const fn is_trusted(&self) -> bool {
        self.marker.is_none()
    }
}

/// Host-key policy evaluator used by [`crate::session::RusshSession`].
///
/// `accept-new` uses trust-on-first-use: it appends a key only when no trusted
/// entry exists for the host, and rejects a changed or explicitly revoked key.
///
/// Evaluation is split in two so the async handshake never does filesystem
/// I/O on a worker thread: [`HostKeyVerifier::decide_public_key`] is pure,
/// and [`persist_first_contact`] runs on the blocking pool (see
/// `session.rs`'s `check_server_key`).
pub(crate) struct HostKeyVerifier {
    policy: HostKeyPolicy,
    host: String,
    port: u16,
    aliases: Vec<String>,
    path: Option<PathBuf>,
    entries: Vec<KnownHostEntry>,
}

/// Outcome of evaluating the offered host key against `known_hosts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HostKeyDecision {
    /// Trusted (or policy off): proceed.
    Accept,
    /// First contact under accept-new: persist the key material, then proceed.
    Persist(String),
    /// Unknown, changed, or revoked key: abort the connection.
    Reject,
}

impl HostKeyVerifier {
    pub(crate) fn new(
        policy: HostKeyPolicy,
        configured_path: Option<PathBuf>,
        host: impl Into<String>,
        port: u16,
    ) -> Result<Self, SshError> {
        let path = match policy {
            HostKeyPolicy::No | HostKeyPolicy::Off => None,
            HostKeyPolicy::Yes | HostKeyPolicy::AcceptNew => {
                Some(configured_path.unwrap_or_else(default_known_hosts_path))
            }
        };
        let entries = path
            .as_deref()
            .map(load_known_hosts)
            .transpose()?
            .unwrap_or_default();

        Ok(Self {
            policy,
            host: host.into(),
            port,
            aliases: Vec::new(),
            path,
            entries,
        })
    }

    pub(crate) fn with_alias(mut self, alias: impl Into<String>) -> Self {
        let alias = alias.into();
        if alias != self.host && !self.aliases.contains(&alias) {
            self.aliases.push(alias);
        }
        self
    }

    /// Evaluate the offered key against the loaded `known_hosts` snapshot.
    /// Pure: no I/O, safe to call on an async worker.
    pub(crate) fn decide_public_key(&self, key: &PublicKey) -> HostKeyDecision {
        let encoded = match key.to_openssh() {
            Ok(encoded) => encoded,
            Err(error) => {
                log::error!("could not encode offered SSH host key: {error}");
                return HostKeyDecision::Reject;
            }
        };
        let Some(key) = key_material(&encoded) else {
            log::error!("could not encode offered SSH host key");
            return HostKeyDecision::Reject;
        };
        self.decide_key(&key)
    }

    fn decide_key(&self, key: &str) -> HostKeyDecision {
        match self.policy {
            HostKeyPolicy::No | HostKeyPolicy::Off => HostKeyDecision::Accept,
            HostKeyPolicy::Yes | HostKeyPolicy::AcceptNew => {
                let matches: Vec<_> = self
                    .entries
                    .iter()
                    .filter(|entry| entry.matches(&self.host_names()))
                    .collect();
                if matches
                    .iter()
                    .any(|entry| entry.is_revoked() && entry.key == key)
                {
                    log::error!("host key for {} is explicitly revoked", self.host);
                    return HostKeyDecision::Reject;
                }

                let trusted: Vec<_> = matches
                    .into_iter()
                    .filter(|entry| entry.is_trusted())
                    .collect();
                if trusted.iter().any(|entry| entry.key == key) {
                    return HostKeyDecision::Accept;
                }

                match self.policy {
                    HostKeyPolicy::Yes => {
                        log::error!("no trusted host key for {}", self.host);
                        HostKeyDecision::Reject
                    }
                    HostKeyPolicy::AcceptNew if !trusted.is_empty() => {
                        log::error!("host key changed for {}", self.host);
                        HostKeyDecision::Reject
                    }
                    HostKeyPolicy::AcceptNew => match &self.path {
                        Some(_) => HostKeyDecision::Persist(key.to_string()),
                        None => HostKeyDecision::Accept,
                    },
                    HostKeyPolicy::No | HostKeyPolicy::Off => HostKeyDecision::Accept,
                }
            }
        }
    }

    /// Everything [`persist_first_contact`] needs, offloaded by the caller.
    /// `Some` exactly when a known_hosts path applies (`yes` / `accept-new`).
    pub(crate) fn persist_context(&self) -> Option<(PathBuf, String, u16, Vec<String>)> {
        self.path
            .clone()
            .map(|path| (path, self.host.clone(), self.port, self.host_names()))
    }

    /// Record a just-persisted first-contact key so later checks within this
    /// handshake treat it as trusted.
    pub(crate) fn note_persisted(&mut self, key: &str) {
        self.entries.push(KnownHostEntry {
            patterns: vec![host_pattern(&self.host, self.port)],
            key: key.to_string(),
            marker: None,
        });
    }

    /// The configured host name (for diagnostics).
    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    // Lookup keys are the configured names only; OpenSSH `CheckHostIP`-style
    // pins on the resolved IP address are intentionally not consulted.
    fn host_names(&self) -> Vec<String> {
        std::iter::once(&self.host)
            .chain(self.aliases.iter())
            .map(|host| host_pattern(host, self.port))
            .collect()
    }
}

fn default_known_hosts_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|path| path.join(".ssh").join("known_hosts"))
        .unwrap_or_else(|| PathBuf::from(".ssh/known_hosts"))
}

fn load_known_hosts(path: &Path) -> Result<Vec<KnownHostEntry>, SshError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(SshError::Transport(format!(
                "could not read known_hosts {}: {error}",
                path.display()
            )));
        }
    };

    contents
        .lines()
        .enumerate()
        .filter_map(|(line_no, line)| parse_known_host_line(line, line_no + 1, path))
        .collect()
}

fn parse_known_host_line(
    line: &str,
    line_no: usize,
    path: &Path,
) -> Option<Result<KnownHostEntry, SshError>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let mut fields = line.split_whitespace();
    let first = fields.next()?;
    let (marker, hosts) = if first.starts_with('@') {
        (Some(first.to_string()), fields.next())
    } else {
        (None, Some(first))
    };
    let Some(hosts) = hosts else {
        return Some(Err(malformed_known_hosts(path, line_no)));
    };
    let (Some(kind), Some(data)) = (fields.next(), fields.next()) else {
        return Some(Err(malformed_known_hosts(path, line_no)));
    };
    let key = format!("{kind} {data}");
    if PublicKey::from_openssh(&key).is_err() {
        return Some(Err(malformed_known_hosts(path, line_no)));
    }

    Some(Ok(KnownHostEntry {
        patterns: hosts.split(',').map(str::to_string).collect(),
        key,
        marker,
    }))
}

fn malformed_known_hosts(path: &Path, line_no: usize) -> SshError {
    SshError::Transport(format!(
        "malformed known_hosts entry in {} at line {line_no}",
        path.display()
    ))
}

/// Append a first-contact key while holding the process-global write lock,
/// re-reading the file **under that lock** first: two concurrent first
/// contacts for the same known_hosts name (e.g. two targets whose ssh-config
/// `HostName` resolves to one machine in a single fan-out run) must not both
/// persist keys. Without the re-check, a MITM key presented to one connection
/// would be permanently recorded next to the real one and trusted by future
/// `yes`-policy runs.
///
/// Returns `Ok(true)` when the key is persisted (or an identical trusted
/// entry was recorded concurrently), `Ok(false)` when a *different* trusted
/// key won the race, and `Err` on filesystem failure.
///
/// Runs synchronously on purpose — offload via `spawn_blocking`.
pub(crate) fn persist_first_contact(
    path: &Path,
    host: &str,
    port: u16,
    host_names: &[String],
    key: &str,
) -> Result<bool, SshError> {
    let lock = KNOWN_HOSTS_WRITE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let current = load_known_hosts(path)?;
    let trusted: Vec<_> = current
        .iter()
        .filter(|entry| entry.matches(host_names) && entry.is_trusted())
        .collect();
    if !trusted.is_empty() {
        // No longer first contact: accept only the already-recorded key.
        return Ok(trusted.iter().any(|entry| entry.key == key));
    }
    append_known_host_locked(path, host, port, key).map_err(|error| {
        SshError::Transport(format!(
            "could not persist host key to {}: {error}",
            path.display()
        ))
    })?;
    Ok(true)
}

fn append_known_host_locked(path: &Path, host: &str, port: u16, key: &str) -> std::io::Result<()> {
    let existed = path.exists();
    let needs_newline = fs::read(path)
        .ok()
        .and_then(|contents| contents.last().copied())
        .is_some_and(|last| last != b'\n');

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    if needs_newline {
        file.write_all(b"\n")?;
    }
    writeln!(file, "{} {key}", host_pattern(host, port))?;
    file.flush()?;

    #[cfg(unix)]
    if !existed {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn host_pattern(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    }
}

fn key_material(key: &str) -> Option<String> {
    let mut fields = key.split_whitespace();
    Some(format!("{} {}", fields.next()?, fields.next()?))
}

/// Match one `HashKnownHosts` pattern (`|1|salt|hash`, both base64) against a
/// lookup name: `HMAC-SHA1(salt, name) == hash` (OpenSSH `hostfile.c`).
/// Malformed salt/hash fields simply do not match (mirrors OpenSSH, which
/// skips unparsable hashed hosts instead of erroring the whole file).
fn hashed_pattern_matches(salt_and_hash: &str, name: &str) -> bool {
    use hmac::{Hmac, KeyInit, Mac};
    use sha1::Sha1;

    let Some((salt, hash)) = salt_and_hash.split_once('|') else {
        return false;
    };
    let (Some(salt), Some(hash)) = (base64_decode(salt), base64_decode(hash)) else {
        return false;
    };
    let Ok(mut mac) = <Hmac<Sha1> as KeyInit>::new_from_slice(&salt) else {
        return false;
    };
    mac.update(name.as_bytes());
    mac.verify_slice(&hash).is_ok()
}

/// Strict standard-alphabet base64 (the only form OpenSSH emits in hashed
/// `known_hosts` fields). Hand-rolled to keep the dependency footprint at
/// `hmac` + `sha1`; rejects non-alphabet bytes and impossible lengths.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    fn sextet(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some(u32::from(byte - b'A')),
            b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let input = input.trim_end_matches('=').as_bytes();
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    for chunk in input.chunks(4) {
        if chunk.len() == 1 {
            return None; // a lone sextet cannot encode a whole byte
        }
        let mut accumulator = 0u32;
        for &byte in chunk {
            accumulator = (accumulator << 6) | sextet(byte)?;
        }
        accumulator <<= 6 * (4 - chunk.len()) as u32;
        let bytes = accumulator.to_be_bytes();
        output.extend_from_slice(&bytes[1..chunk.len()]);
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use repose_core::config::HostKeyPolicy;
    use tempfile::tempdir;

    use super::{HostKeyDecision, HostKeyVerifier, persist_first_contact};

    const KEY_ONE: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti";
    const KEY_TWO: &str = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQCmjkeMm8k3JkNrf16eb5pG4bc77B6Mt3VN4saltsRV8vASpyWa/PlBgdaeldOaNJ5NK0gqU3KyiUNzHbdcc8572e7IUBDJS/rlaWARiSL4aos2VbNX0k56Z5zYp9m/bq5m9/mlb+PQkNBjIhimgpYNiq2TwBiYeA6tLb79cPtHA0cX5BLk/a5oUpLsiR4kI/f+Q98vVDKasKXXVh5YLkLobrruDB6er2A9fOcIUF0O4JCRLh/Dc161gE3fQrYTMQenbppZzfxrZfQ8YwLPvKjnqm+XRX+pbTtaJuj0EgTSzUK+EZxoSw8CNwiZpxrjwecTMVQ8w/srQmh4ABGuTqk0wP8HcI7hg+fpBv7kiejh5X/Oehxt+Puu85u9GVXb1a0av/vhJvUCBcuISvCA/z1wVJ0xdLhb1/ZiTDdTzyNbZQ0OQijzK+e1SlkNhp+3eGVZu3pNZvnTppwIXv3wg6kV1HodkWGgh1ayY7Buc52Z8okDYqvJat5CzOj5OaQNr/k=";

    #[test]
    fn yes_accepts_only_an_existing_matching_key() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("host {KEY_ONE}\n")).unwrap();
        let verifier = HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "host", 22).unwrap();

        assert_eq!(verifier.decide_key(KEY_ONE), HostKeyDecision::Accept);
        assert_eq!(verifier.decide_key(KEY_TWO), HostKeyDecision::Reject);
    }

    #[test]
    fn accept_new_persists_first_contact_and_refuses_changed_key() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("old-entry {KEY_ONE}")).unwrap();
        let mut verifier =
            HostKeyVerifier::new(HostKeyPolicy::AcceptNew, Some(path.clone()), "host", 2222)
                .unwrap();

        let HostKeyDecision::Persist(key) = verifier.decide_key(KEY_ONE) else {
            panic!("first contact must decide to persist");
        };
        let (persist_path, host, port, names) = verifier.persist_context().unwrap();
        assert!(persist_first_contact(&persist_path, &host, port, &names, &key).unwrap());
        verifier.note_persisted(&key);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!("old-entry {KEY_ONE}\n[host]:2222 {KEY_ONE}\n")
        );
        assert_eq!(verifier.decide_key(KEY_TWO), HostKeyDecision::Reject);
    }

    #[test]
    fn concurrent_first_contacts_cannot_both_persist() {
        // Both verifiers snapshot an empty known_hosts (each sees "first
        // contact"); the loser's persist must fail the re-check under the
        // write lock instead of recording a second, different key.
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        let first =
            HostKeyVerifier::new(HostKeyPolicy::AcceptNew, Some(path.clone()), "host", 22).unwrap();
        let second =
            HostKeyVerifier::new(HostKeyPolicy::AcceptNew, Some(path.clone()), "host", 22).unwrap();

        let HostKeyDecision::Persist(key_one) = first.decide_key(KEY_ONE) else {
            panic!("first contact must decide to persist");
        };
        let HostKeyDecision::Persist(key_two) = second.decide_key(KEY_TWO) else {
            panic!("first contact must decide to persist");
        };
        let (path_one, host_one, port_one, names_one) = first.persist_context().unwrap();
        let (path_two, host_two, port_two, names_two) = second.persist_context().unwrap();
        assert!(
            persist_first_contact(&path_one, &host_one, port_one, &names_one, &key_one).unwrap()
        );
        assert!(
            !persist_first_contact(&path_two, &host_two, port_two, &names_two, &key_two).unwrap(),
            "a different key that lost the race must be rejected"
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!("host {KEY_ONE}\n"),
            "only the winning key is recorded"
        );
    }

    #[test]
    fn concurrent_first_contact_with_the_same_key_is_idempotent() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        let verifier =
            HostKeyVerifier::new(HostKeyPolicy::AcceptNew, Some(path.clone()), "host", 22).unwrap();
        let (persist_path, host, port, names) = verifier.persist_context().unwrap();
        assert!(persist_first_contact(&persist_path, &host, port, &names, KEY_ONE).unwrap());
        assert!(
            persist_first_contact(&persist_path, &host, port, &names, KEY_ONE).unwrap(),
            "the same key recorded concurrently stays accepted"
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!("host {KEY_ONE}\n"),
            "no duplicate entry is appended"
        );
    }

    #[test]
    fn revoked_key_is_refused_even_without_a_trusted_pin() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("@revoked host {KEY_ONE}\n")).unwrap();
        let verifier =
            HostKeyVerifier::new(HostKeyPolicy::AcceptNew, Some(path), "host", 22).unwrap();

        assert_eq!(verifier.decide_key(KEY_ONE), HostKeyDecision::Reject);
    }

    #[test]
    fn no_and_off_do_not_read_known_hosts() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, "not a valid known-hosts line\n").unwrap();

        for policy in [HostKeyPolicy::No, HostKeyPolicy::Off] {
            let verifier = HostKeyVerifier::new(policy, Some(path.clone()), "host", 22).unwrap();
            assert_eq!(verifier.decide_key(KEY_ONE), HostKeyDecision::Accept);
        }
    }

    #[test]
    fn malformed_known_hosts_is_rejected() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, "host malformed-key\n").unwrap();

        assert!(HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "host", 22).is_err());
    }

    #[test]
    fn host_patterns_support_wildcards_and_negation() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("*.example,!bad.example {KEY_ONE}\n")).unwrap();

        let good =
            HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path.clone()), "ok.example", 22).unwrap();
        let bad = HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "bad.example", 22).unwrap();
        assert_eq!(good.decide_key(KEY_ONE), HostKeyDecision::Accept);
        assert_eq!(bad.decide_key(KEY_ONE), HostKeyDecision::Reject);
    }

    #[test]
    fn alias_pin_is_honoured_for_a_resolved_hostname() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("alias {KEY_ONE}\n")).unwrap();

        let verifier = HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "resolved", 22)
            .unwrap()
            .with_alias("alias");
        assert_eq!(verifier.decide_key(KEY_ONE), HostKeyDecision::Accept);
    }

    // HMAC-SHA1 vectors computed independently (Python `hmac` stdlib) with
    // salt = bytes 0x00..0x13; the Rust implementation must reproduce them.
    const HASHED_SALT: &str = "AAECAwQFBgcICQoLDA0ODxAREhM=";
    const HASHED_HOST: &str = "6tbkAfZDulNpJ/cQ7aCK0Bbfp+g="; // "host"
    const HASHED_HOST_2222: &str = "mwVUb/Ss+IB46YfMPRtwfxIeTCM="; // "[host]:2222"

    #[test]
    fn hashed_entry_matches_the_pinned_host() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("|1|{HASHED_SALT}|{HASHED_HOST} {KEY_ONE}\n")).unwrap();
        let verifier = HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "host", 22).unwrap();

        assert_eq!(verifier.decide_key(KEY_ONE), HostKeyDecision::Accept);
        assert_eq!(verifier.decide_key(KEY_TWO), HostKeyDecision::Reject);
    }

    #[test]
    fn hashed_entry_hashes_the_bracketed_form_for_non_default_ports() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(
            &path,
            format!("|1|{HASHED_SALT}|{HASHED_HOST_2222} {KEY_ONE}\n"),
        )
        .unwrap();

        let on_2222 =
            HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path.clone()), "host", 2222).unwrap();
        let on_22 = HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "host", 22).unwrap();
        assert_eq!(on_2222.decide_key(KEY_ONE), HostKeyDecision::Accept);
        assert_eq!(on_22.decide_key(KEY_ONE), HostKeyDecision::Reject);
    }

    #[test]
    fn accept_new_refuses_a_changed_key_behind_a_hashed_entry() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        let contents = format!("|1|{HASHED_SALT}|{HASHED_HOST} {KEY_ONE}\n");
        fs::write(&path, &contents).unwrap();
        let verifier =
            HostKeyVerifier::new(HostKeyPolicy::AcceptNew, Some(path.clone()), "host", 22).unwrap();

        // A hashed pin is not first contact: the changed key must be refused
        // and nothing may be appended to known_hosts.
        assert_eq!(verifier.decide_key(KEY_TWO), HostKeyDecision::Reject);
        assert_eq!(fs::read_to_string(&path).unwrap(), contents);
        assert_eq!(verifier.decide_key(KEY_ONE), HostKeyDecision::Accept);
    }

    #[test]
    fn mixed_plain_and_hashed_entries_each_match_their_host() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(
            &path,
            format!("other.example {KEY_TWO}\n|1|{HASHED_SALT}|{HASHED_HOST} {KEY_ONE}\n"),
        )
        .unwrap();

        let hashed =
            HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path.clone()), "host", 22).unwrap();
        assert_eq!(hashed.decide_key(KEY_ONE), HostKeyDecision::Accept);
        assert_eq!(hashed.decide_key(KEY_TWO), HostKeyDecision::Reject);

        let plain =
            HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "other.example", 22).unwrap();
        assert_eq!(plain.decide_key(KEY_TWO), HostKeyDecision::Accept);
        assert_eq!(plain.decide_key(KEY_ONE), HostKeyDecision::Reject);
    }

    #[test]
    fn base64_decoder_handles_openssh_material_and_rejects_junk() {
        let expected: Vec<u8> = (0..20).collect();
        assert_eq!(super::base64_decode(HASHED_SALT), Some(expected));
        assert_eq!(super::base64_decode("aGk="), Some(b"hi".to_vec()));
        assert_eq!(super::base64_decode("////"), Some(vec![255, 255, 255]));
        assert_eq!(super::base64_decode(""), Some(Vec::new()));
        assert_eq!(super::base64_decode("!!!!"), None);
        assert_eq!(super::base64_decode("A"), None);
    }

    #[test]
    fn malformed_hashed_pattern_does_not_match_or_panic() {
        assert!(!super::hashed_pattern_matches("nosplit", "host"));
        assert!(!super::hashed_pattern_matches("bad|bad", "host"));
        assert!(!super::hashed_pattern_matches("|", "host"));
    }

    #[test]
    fn non_default_port_requires_the_bracketed_host_pattern() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("host {KEY_ONE}\n")).unwrap();
        let verifier = HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "host", 2222).unwrap();

        assert_eq!(verifier.decide_key(KEY_ONE), HostKeyDecision::Reject);
    }
}
