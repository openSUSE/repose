//! Parse `-t` host strings: `[user@]host[:port]`.

use thiserror::Error;

/// Default SSH user.
const DEFAULT_USER: &str = "root";
/// Default SSH port.
const DEFAULT_PORT: u16 = 22;

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
    // `[user@]host[:port]` is split like a URL authority component.
    let s = arg.trim();
    if s.is_empty() {
        return Err(HostParseError::EmptyHost);
    }

    let (user_part, host_part) = if let Some((user, hostport)) = s.split_once('@') {
        // The FIRST `@` separates the user: `alice@example.com` → `alice`.
        (Some(user), hostport)
    } else {
        (None, s)
    };

    // Bracketed IPv6 hosts are not supported — skip.
    // host:port — if last colon and port is numeric.
    //
    // The hostname is lowercased; the username and port are left untouched.
    // The PortNotInt error message also carries the lowercased hostname.
    let (hostname, port) = if let Some((h, p)) = host_part.rsplit_once(':') {
        // The LAST colon splits host from port, and a non-numeric segment
        // after it errors as a bad port. Unbracketed IPv6 has no way to opt
        // out of that split, so it either errors here or — when its final
        // group happens to be numeric — is misread as host plus port.
        let h_lower = h.to_lowercase();
        if p.is_empty() {
            return Err(HostParseError::PortNotInt(h_lower));
        }
        match p.parse::<u16>() {
            Ok(port) => {
                if h.is_empty() {
                    return Err(HostParseError::EmptyHost);
                }
                (h_lower, port)
            }
            Err(_) => {
                // non-numeric port segment
                return Err(HostParseError::PortNotInt(h_lower));
            }
        }
    } else {
        if host_part.is_empty() {
            return Err(HostParseError::EmptyHost);
        }
        (host_part.to_lowercase(), DEFAULT_PORT)
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
    fn matches_vector() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/vectors/hostparse/hosts.json");
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

    #[test]
    fn hostname_lowercased_user_and_port_untouched() {
        // Only the hostname is case-normalized: `Root@EXAMPLE.com:2222`
        // yields hostname `example.com`, username `Root` (unchanged), and
        // port 2222.
        let h = parse_host("Root@EXAMPLE.com:2222").unwrap();
        assert_eq!(h.hostname, "example.com");
        assert_eq!(h.username, "Root");
        assert_eq!(h.port, 2222);
        assert_eq!(h.key, "example.com:2222");

        // A non-numeric port segment errors, and the error message carries
        // the lowercased hostname.
        assert_eq!(
            parse_host("UPPER.host:abc"),
            Err(HostParseError::PortNotInt("upper.host".into()))
        );
    }
}
