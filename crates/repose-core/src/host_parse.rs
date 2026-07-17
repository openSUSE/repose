//! Parse `-t` host strings: `[user@]host[:port]` (Python `repose.host`).

use thiserror::Error;

/// Default SSH user (Python `ParseHosts`).
pub const DEFAULT_USER: &str = "root";
/// Default SSH port.
pub const DEFAULT_PORT: u16 = 22;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSpec {
    /// Map key: `hostname` or `hostname:port` when port ≠ 22.
    pub key: String,
    pub hostname: String,
    pub port: u16,
    pub username: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HostParseError {
    #[error("Target host: Wrong port specification on Host: {0}")]
    PortNotInt(String),
    #[error("Target host: empty hostname")]
    EmptyHost,
}

/// Parse `[user@]host[:port]` without creating a Target.
pub fn parse_host(arg: &str) -> Result<HostSpec, HostParseError> {
    // Mirror urllib.parse.urlparse("//" + arg) behaviour for host/user/port.
    let s = arg.trim();
    if s.is_empty() {
        return Err(HostParseError::EmptyHost);
    }

    let (user_part, host_part) = if let Some((user, hostport)) = s.split_once('@') {
        // urlparse("//alice@example.com") → username alice (first @).
        (Some(user), hostport)
    } else {
        (None, s)
    };

    // IPv6 in brackets not supported in Python path (urlparse //host) — skip.
    // host:port — if last colon and port is numeric.
    let (hostname, port) = if let Some((h, p)) = host_part.rsplit_once(':') {
        // Distinguish hostname:port from bare IPv6 (no brackets) — Python
        // ValueError on non-int port.
        if p.is_empty() {
            return Err(HostParseError::PortNotInt(h.to_string()));
        }
        match p.parse::<u16>() {
            Ok(port) => {
                if h.is_empty() {
                    return Err(HostParseError::EmptyHost);
                }
                (h.to_string(), port)
            }
            Err(_) => {
                // non-numeric port segment
                return Err(HostParseError::PortNotInt(h.to_string()));
            }
        }
    } else {
        if host_part.is_empty() {
            return Err(HostParseError::EmptyHost);
        }
        (host_part.to_string(), DEFAULT_PORT)
    };

    let username = user_part
        .filter(|u| !u.is_empty())
        .unwrap_or(DEFAULT_USER)
        .to_string();

    let key = if port == DEFAULT_PORT {
        hostname.clone()
    } else {
        format!("{hostname}:{port}")
    };

    Ok(HostSpec {
        key,
        hostname,
        port,
        username,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn matches_oracle() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/oracle/hostparse/hosts.json");
        let raw = std::fs::read_to_string(path).unwrap();
        let cases: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        for case in cases {
            let input = case["input"].as_str().unwrap();
            let ok = case["ok"].as_bool().unwrap();
            match parse_host(input) {
                Ok(h) if ok => {
                    assert_eq!(h.key, case["key"].as_str().unwrap(), "{input}");
                    assert_eq!(h.hostname, case["hostname"].as_str().unwrap(), "{input}");
                    assert_eq!(h.port as i64, case["port"].as_i64().unwrap(), "{input}");
                    assert_eq!(h.username, case["username"].as_str().unwrap(), "{input}");
                }
                Err(_) if !ok => {}
                other => panic!("mismatch for {input}: {other:?}"),
            }
        }
    }

    #[test]
    fn basic_defaults() {
        let h = parse_host("example.com").unwrap();
        assert_eq!(h.username, "root");
        assert_eq!(h.port, 22);
        assert_eq!(h.key, "example.com");
    }
}
