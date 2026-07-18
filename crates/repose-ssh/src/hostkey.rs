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
            let (negated, pattern) = pattern
                .strip_prefix('!')
                .map_or((false, pattern.as_str()), |pattern| (true, pattern));
            if names.iter().any(|name| glob_matches(pattern, name)) {
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

    fn is_trusted(&self) -> bool {
        self.marker.is_none()
    }
}

/// Host-key policy evaluator used by [`crate::session::RusshSession`].
///
/// `accept-new` uses trust-on-first-use: it appends a key only when no trusted
/// entry exists for the host, and rejects a changed or explicitly revoked key.
pub(crate) struct HostKeyVerifier {
    policy: HostKeyPolicy,
    host: String,
    port: u16,
    aliases: Vec<String>,
    path: Option<PathBuf>,
    entries: Vec<KnownHostEntry>,
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

    pub(crate) fn verify_public_key(&mut self, key: &PublicKey) -> bool {
        let encoded = match key.to_openssh() {
            Ok(encoded) => encoded,
            Err(error) => {
                log::error!("could not encode offered SSH host key: {error}");
                return false;
            }
        };
        let Some(key) = key_material(&encoded) else {
            log::error!("could not encode offered SSH host key");
            return false;
        };
        self.verify_key(&key)
    }

    fn verify_key(&mut self, key: &str) -> bool {
        match self.policy {
            HostKeyPolicy::No | HostKeyPolicy::Off => true,
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
                    return false;
                }

                let trusted: Vec<_> = matches
                    .into_iter()
                    .filter(|entry| entry.is_trusted())
                    .collect();
                if trusted.iter().any(|entry| entry.key == key) {
                    return true;
                }

                match self.policy {
                    HostKeyPolicy::Yes => {
                        log::error!("no trusted host key for {}", self.host);
                        false
                    }
                    HostKeyPolicy::AcceptNew if !trusted.is_empty() => {
                        log::error!("host key changed for {}", self.host);
                        false
                    }
                    HostKeyPolicy::AcceptNew => self.accept_new(key),
                    HostKeyPolicy::No | HostKeyPolicy::Off => true,
                }
            }
        }
    }

    fn accept_new(&mut self, key: &str) -> bool {
        let Some(path) = &self.path else {
            return true;
        };
        if let Err(error) = append_known_host(path, &self.host, self.port, key) {
            log::warn!(
                "accept-new: could not persist host key for {} to {}: {}",
                self.host,
                path.display(),
                error
            );
            return true;
        }

        self.entries.push(KnownHostEntry {
            patterns: vec![host_pattern(&self.host, self.port)],
            key: key.to_string(),
            marker: None,
        });
        true
    }

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

fn append_known_host(path: &Path, host: &str, port: u16, key: &str) -> std::io::Result<()> {
    let lock = KNOWN_HOSTS_WRITE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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

fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern: Vec<_> = pattern.chars().collect();
    let value: Vec<_> = value.chars().collect();
    glob_matches_chars(&pattern, &value)
}

fn glob_matches_chars(pattern: &[char], value: &[char]) -> bool {
    match pattern {
        [] => value.is_empty(),
        ['*', rest @ ..] => {
            glob_matches_chars(rest, value)
                || (!value.is_empty() && glob_matches_chars(pattern, &value[1..]))
        }
        ['?', rest @ ..] => !value.is_empty() && glob_matches_chars(rest, &value[1..]),
        [first, rest @ ..] => {
            value.first().is_some_and(|value| value == first)
                && glob_matches_chars(rest, &value[1..])
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use repose_core::config::HostKeyPolicy;
    use tempfile::tempdir;

    use super::HostKeyVerifier;

    const KEY_ONE: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti";
    const KEY_TWO: &str = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQCmjkeMm8k3JkNrf16eb5pG4bc77B6Mt3VN4saltsRV8vASpyWa/PlBgdaeldOaNJ5NK0gqU3KyiUNzHbdcc8572e7IUBDJS/rlaWARiSL4aos2VbNX0k56Z5zYp9m/bq5m9/mlb+PQkNBjIhimgpYNiq2TwBiYeA6tLb79cPtHA0cX5BLk/a5oUpLsiR4kI/f+Q98vVDKasKXXVh5YLkLobrruDB6er2A9fOcIUF0O4JCRLh/Dc161gE3fQrYTMQenbppZzfxrZfQ8YwLPvKjnqm+XRX+pbTtaJuj0EgTSzUK+EZxoSw8CNwiZpxrjwecTMVQ8w/srQmh4ABGuTqk0wP8HcI7hg+fpBv7kiejh5X/Oehxt+Puu85u9GVXb1a0av/vhJvUCBcuISvCA/z1wVJ0xdLhb1/ZiTDdTzyNbZQ0OQijzK+e1SlkNhp+3eGVZu3pNZvnTppwIXv3wg6kV1HodkWGgh1ayY7Buc52Z8okDYqvJat5CzOj5OaQNr/k=";

    #[test]
    fn yes_accepts_only_an_existing_matching_key() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("host {KEY_ONE}\n")).unwrap();
        let mut verifier =
            HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "host", 22).unwrap();

        assert!(verifier.verify_key(KEY_ONE));
        assert!(!verifier.verify_key(KEY_TWO));
    }

    #[test]
    fn accept_new_persists_first_contact_and_refuses_changed_key() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("old-entry {KEY_ONE}")).unwrap();
        let mut verifier =
            HostKeyVerifier::new(HostKeyPolicy::AcceptNew, Some(path.clone()), "host", 2222)
                .unwrap();

        assert!(verifier.verify_key(KEY_ONE));
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!("old-entry {KEY_ONE}\n[host]:2222 {KEY_ONE}\n")
        );
        assert!(!verifier.verify_key(KEY_TWO));
    }

    #[test]
    fn revoked_key_is_refused_even_without_a_trusted_pin() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("@revoked host {KEY_ONE}\n")).unwrap();
        let mut verifier =
            HostKeyVerifier::new(HostKeyPolicy::AcceptNew, Some(path), "host", 22).unwrap();

        assert!(!verifier.verify_key(KEY_ONE));
    }

    #[test]
    fn no_and_off_do_not_read_known_hosts() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, "not a valid known-hosts line\n").unwrap();

        for policy in [HostKeyPolicy::No, HostKeyPolicy::Off] {
            let mut verifier =
                HostKeyVerifier::new(policy, Some(path.clone()), "host", 22).unwrap();
            assert!(verifier.verify_key(KEY_ONE));
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

        let mut good =
            HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path.clone()), "ok.example", 22).unwrap();
        let mut bad =
            HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "bad.example", 22).unwrap();
        assert!(good.verify_key(KEY_ONE));
        assert!(!bad.verify_key(KEY_ONE));
    }

    #[test]
    fn alias_pin_is_honoured_for_a_resolved_hostname() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("alias {KEY_ONE}\n")).unwrap();

        let mut verifier = HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "resolved", 22)
            .unwrap()
            .with_alias("alias");
        assert!(verifier.verify_key(KEY_ONE));
    }

    #[test]
    fn non_default_port_requires_the_bracketed_host_pattern() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("known_hosts");
        fs::write(&path, format!("host {KEY_ONE}\n")).unwrap();
        let mut verifier =
            HostKeyVerifier::new(HostKeyPolicy::Yes, Some(path), "host", 2222).unwrap();

        assert!(!verifier.verify_key(KEY_ONE));
    }
}
