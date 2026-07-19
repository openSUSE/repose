//! Minimal OpenSSH configuration lookup used by the russh connection.
//!
//! The supported directives are `Host`, `Hostname`, `Port`, `User`, multiple
//! `IdentityFile` entries, and `ProxyCommand`. `Match`, `Include`, and
//! `ProxyJump` are deliberately not interpreted; callers receive a warning so
//! an operator can replace a jump-only configuration with `ProxyCommand`.

use std::path::{Path, PathBuf};

/// Connection options selected for one host alias.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct OpenSshOptions {
    pub(crate) hostname: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) user: Option<String>,
    pub(crate) identity_files: Vec<PathBuf>,
    pub(crate) proxy_command: Option<String>,
}

impl OpenSshOptions {
    /// Load the user's `~/.ssh/config` and select options for `host`.
    ///
    /// Reads via `tokio::fs` so the lookup never blocks an async worker;
    /// [`crate::session::RusshSession`] caches the result per session.
    pub(crate) async fn lookup(host: &str) -> Self {
        let path = default_config_path();
        let contents = match tokio::fs::read_to_string(&path).await {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(error) => {
                log::warn!("could not read {}: {error}", path.display());
                return Self::default();
            }
        };
        Self::parse(&contents, host)
    }

    fn parse(contents: &str, host: &str) -> Self {
        let mut options = Self::default();
        let mut applies = false;

        for raw_line in contents.lines() {
            // OpenSSH only supports full-line comments (first non-blank
            // character is `#`); a mid-line `#` is part of the value
            // (e.g. `ProxyCommand ssh gw nc %h %p # not stripped`).
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((directive, value)) = split_directive(line) else {
                continue;
            };
            let directive = directive.to_ascii_lowercase();

            if directive == "host" {
                applies = host_patterns_match(value, host);
                continue;
            }
            if directive == "match" {
                log::warn!("OpenSSH Match blocks are not supported by the Rust SSH backend");
                applies = false;
                continue;
            }
            if directive == "include" {
                log::warn!("OpenSSH Include directives are not supported by the Rust SSH backend");
                continue;
            }
            if !applies {
                continue;
            }

            match directive.as_str() {
                "hostname" if options.hostname.is_none() => {
                    options.hostname = non_empty(value).map(str::to_string);
                }
                "port" if options.port.is_none() => match value.parse() {
                    Ok(port) => options.port = Some(port),
                    Err(_) => log::warn!("ignoring invalid OpenSSH Port value {value:?}"),
                },
                "user" if options.user.is_none() => {
                    options.user = non_empty(value).map(str::to_string);
                }
                "identityfile" => {
                    if let Some(path) = non_empty(value) {
                        options.identity_files.push(expand_home(path));
                    }
                }
                "proxycommand" if options.proxy_command.is_none() => {
                    options.proxy_command = non_empty(value).map(str::to_string);
                }
                "proxyjump" => {
                    log::warn!(
                        "OpenSSH ProxyJump is not supported; use an equivalent ProxyCommand"
                    );
                }
                _ => {}
            }
        }

        options
    }
}

fn default_config_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|path| path.join(".ssh").join("config"))
        .unwrap_or_else(|| PathBuf::from(".ssh/config"))
}

fn split_directive(line: &str) -> Option<(&str, &str)> {
    let split_at = line.find(|character: char| character.is_whitespace() || character == '=')?;
    let directive = &line[..split_at];
    let value = line[split_at..]
        .trim_start_matches(|character: char| character.is_whitespace() || character == '=')
        .trim();
    Some((directive, strip_quotes(value)))
}

fn strip_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn non_empty(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(value));
    }
    if let Some(path) = value.strip_prefix("~/") {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(path))
            .unwrap_or_else(|| PathBuf::from(value));
    }
    Path::new(value).to_path_buf()
}

fn host_patterns_match(patterns: &str, host: &str) -> bool {
    let mut positive_match = false;
    for pattern in patterns.split_whitespace() {
        let (negated, pattern) = pattern
            .strip_prefix('!')
            .map_or((false, pattern), |pattern| (true, pattern));
        if crate::glob::glob_matches(pattern, host) {
            if negated {
                return false;
            }
            positive_match = true;
        }
    }
    positive_match
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::OpenSshOptions;

    #[test]
    fn selects_required_directives_and_multiple_identity_files() {
        let options = OpenSshOptions::parse(
            "Host *\n  User global\nHost qa-*\n  HostName target.example\n  Port 2200\n  User qa\n  IdentityFile /keys/one\n  IdentityFile /keys/two\n  ProxyCommand nc %h %p\n",
            "qa-host",
        );

        assert_eq!(options.hostname.as_deref(), Some("target.example"));
        assert_eq!(options.port, Some(2200));
        assert_eq!(options.user.as_deref(), Some("global"));
        assert_eq!(
            options.identity_files,
            vec![PathBuf::from("/keys/one"), PathBuf::from("/keys/two")]
        );
        assert_eq!(options.proxy_command.as_deref(), Some("nc %h %p"));
    }

    #[test]
    fn host_patterns_and_negation_select_the_right_block() {
        let config = "Host *.example !blocked.example\n  Port 2222\n";
        assert_eq!(OpenSshOptions::parse(config, "ok.example").port, Some(2222));
        assert_eq!(OpenSshOptions::parse(config, "blocked.example").port, None);
    }

    #[test]
    fn first_scalar_value_wins_like_openssh() {
        let options = OpenSshOptions::parse(
            "Host *\n  HostName generic\n  Port 2200\nHost qa\n  HostName specific\n  Port 2201\n",
            "qa",
        );
        assert_eq!(options.hostname.as_deref(), Some("generic"));
        assert_eq!(options.port, Some(2200));
    }

    #[test]
    fn mid_line_hash_is_part_of_the_value() {
        let options = OpenSshOptions::parse(
            "# full-line comment\nHost qa\n   # indented comment\n  ProxyCommand nc %h %p # keep-me\n  IdentityFile /keys/with#hash\n",
            "qa",
        );
        assert_eq!(options.proxy_command.as_deref(), Some("nc %h %p # keep-me"));
        assert_eq!(
            options.identity_files,
            vec![PathBuf::from("/keys/with#hash")]
        );
    }

    #[test]
    fn supports_equals_syntax_and_quoted_values() {
        let options = OpenSshOptions::parse(
            "Host qa\n  User = 'operator'\n  ProxyCommand = \"nc %h %p\"\n",
            "qa",
        );
        assert_eq!(options.user.as_deref(), Some("operator"));
        assert_eq!(options.proxy_command.as_deref(), Some("nc %h %p"));
    }
}
