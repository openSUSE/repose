//! Parse `-t` host strings: `[user@]host[:port]`.
//!
//! An IPv6 target is written bracketed, `[2001:db8::1]` or
//! `[2001:db8::1]:2222`, exactly as it appears in `known_hosts`. A bare
//! unbracketed literal (`2001:db8::1`) is accepted at the default port,
//! since a colon cannot occur in a hostname and the form is therefore
//! unambiguous. A zone index (`fe80::1%eth0`) is not supported.

use std::net::Ipv6Addr;

use thiserror::Error;

/// Default SSH user.
const DEFAULT_USER: &str = "root";
/// Default SSH port.
const DEFAULT_PORT: u16 = 22;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSpec {
    /// Map key: `hostname`, or `hostname:port` when port ≠ 22. An IPv6
    /// address is bracketed at every port (`[::1]`, `[::1]:2222`) so the
    /// port stays recoverable; see the key construction in [`parse_host`].
    pub key: String,
    /// Never bracketed, even for IPv6 — the connect tuple and `known_hosts`
    /// both want the bare address.
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
    #[error(
        "Target host: {0} is not a valid IPv6 address; brackets are only for IPv6 literals, as in [2001:db8::1]:2222"
    )]
    InvalidIpv6(String),
}

/// The RFC 5952 form of `s` if it is an IPv6 literal, else `None`.
///
/// Canonicalizing is what makes the `known_hosts` pin work. OpenSSH runs a
/// numeric target through `getnameinfo(NI_NUMERICHOST)` before it looks the
/// host up or records it, so `2001:db8:0:0:0:0:0:1` is stored as
/// `2001:db8::1`. Keeping the text as typed would give one address as many
/// identities as it has spellings, and under the default `accept-new` a
/// missed identity is not a refusal — it looks like first contact, so any
/// key the network offers gets trusted and appended.
fn canonical_ipv6(s: &str) -> Option<String> {
    s.parse::<Ipv6Addr>().ok().map(|a| a.to_string())
}

/// Parse the port segment, reporting `host` in the error the way the rest of
/// this module does (lowercased, without any brackets).
fn parse_port(p: &str, host: &str) -> Result<u16, HostParseError> {
    p.parse::<u16>()
        .map_err(|_| HostParseError::PortNotInt(host.to_string()))
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

    // The hostname is lowercased; the username and port are left untouched.
    // The PortNotInt error message also carries the lowercased hostname.
    let (hostname, port) = if let Some(rest) = host_part.strip_prefix('[') {
        // `[addr]` or `[addr]:port`. The brackets must close, and must wrap
        // an IPv6 literal — they are not a general quoting mechanism.
        let Some((inner, after)) = rest.split_once(']') else {
            return Err(HostParseError::InvalidIpv6(host_part.to_lowercase()));
        };
        if inner.is_empty() {
            return Err(HostParseError::EmptyHost);
        }
        let Some(addr) = canonical_ipv6(inner) else {
            return Err(HostParseError::InvalidIpv6(inner.to_lowercase()));
        };
        let port = match after {
            "" => DEFAULT_PORT,
            _ => {
                let Some(p) = after.strip_prefix(':') else {
                    return Err(HostParseError::InvalidIpv6(host_part.to_lowercase()));
                };
                parse_port(p, &addr)?
            }
        };
        (addr, port)
    } else if let Some(addr) = canonical_ipv6(host_part) {
        // Unbracketed literal: unambiguous, but leaves no room for a port.
        (addr, DEFAULT_PORT)
    } else if let Some((h, p)) = host_part.rsplit_once(':') {
        // The LAST colon splits host from port. A hostname cannot itself
        // contain a colon, so a colon left in `h` means this was a malformed
        // or unbracketed IPv6 target, not a host/port pair — refusing it is
        // what stops such an input being silently read as some other host.
        if h.contains(':') {
            return Err(HostParseError::InvalidIpv6(host_part.to_lowercase()));
        }
        if h.is_empty() {
            return Err(HostParseError::EmptyHost);
        }
        let h_lower = h.to_lowercase();
        let port = parse_port(p, &h_lower)?;
        (h_lower, port)
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

    // An IPv6 address is bracketed in the key at EVERY port, including the
    // default. `split_key` recovers host and port by splitting on the last
    // colon and accepting a numeric tail, so a bare `::1` would decode as
    // host `:` port 1; a bracketed key always ends in `]` and can never hit
    // that branch. `hostname` stays bare — the connect tuple and
    // `known_hosts` both need it unbracketed.
    let key = if canonical_ipv6(&hostname).is_some() {
        if port == DEFAULT_PORT {
            format!("[{hostname}]")
        } else {
            format!("[{hostname}]:{port}")
        }
    } else if port == DEFAULT_PORT {
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

    /// The load-bearing split: `key` is bracketed so `split_key` can recover
    /// the port, while `hostname` stays bare because the connect tuple and
    /// `known_hosts`'s `host_pattern` both reject a bracketed address.
    /// Changing either half in isolation breaks connection or host-key
    /// verification.
    #[test]
    fn ipv6_key_is_bracketed_and_hostname_is_not() {
        for (input, key, port) in [
            ("::1", "[::1]", 22),
            ("[::1]", "[::1]", 22),
            ("[::1]:2222", "[::1]:2222", 2222),
        ] {
            let h = parse_host(input).unwrap();
            assert_eq!(h.key, key, "{input}");
            assert_eq!(h.hostname, "::1", "{input} hostname must stay bare");
            assert_eq!(h.port, port, "{input}");
        }
    }

    /// Every spelling of one address must collapse to one identity, because
    /// the `known_hosts` pin is keyed on it and a miss under the default
    /// `accept-new` policy is not a refusal — it reads as first contact and
    /// trusts whatever key the network offered. OpenSSH canonicalizes the
    /// same way (`ssh -G 2001:db8:0:0:0:0:0:1` reports `2001:db8::1`).
    #[test]
    fn every_spelling_of_an_address_yields_one_identity() {
        for input in [
            "[2001:DB8:0:0:0:0:0:1]:2222",
            "[2001:db8::1]:2222",
            "[2001:0db8:0000:0000:0000:0000:0000:0001]:2222",
        ] {
            let h = parse_host(input).unwrap();
            assert_eq!(h.hostname, "2001:db8::1", "{input}");
            assert_eq!(h.key, "[2001:db8::1]:2222", "{input}");
        }
        assert_eq!(parse_host("0:0:0:0:0:0:0:1").unwrap().hostname, "::1");
        assert_eq!(
            parse_host("::ffff:7f00:1").unwrap().hostname,
            "::ffff:127.0.0.1"
        );
    }

    /// The guard that closes the silent-misparse class: anything still
    /// holding a colon after the port is split off was never a hostname, so
    /// it is refused rather than read as some other host. Before this,
    /// `2001:db8::1` resolved to host `2001:db8:` on port 1.
    #[test]
    fn ambiguous_colon_forms_are_refused_not_misread() {
        for input in [
            "fe80::1%eth0",
            "2001:db8::gg",
            "foo:bar:80",
            "[::1",
            "[example.com]:22",
        ] {
            assert!(
                matches!(parse_host(input), Err(HostParseError::InvalidIpv6(_))),
                "{input} must be refused, not silently reinterpreted"
            );
        }
    }
}
