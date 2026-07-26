//! POSIX shell quoting: every argument interpolated into a remote command
//! must reach a refhost's `/bin/sh` as the literal bytes intended.
//!
//! Vectors under `tests/vectors/shell/` pin the exact quoting rule and the
//! command templates below and are the merge gate for any remote command
//! interpolation (design R2 / PR2).

/// Characters safe to leave unquoted: ASCII alphanumerics plus
/// `_@%+=:,./-`; anything else forces single-quoting.
const fn is_safe_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-')
}

/// Quote `s` for a POSIX shell. Pinned by `tests/vectors/shell/quote.json`.
#[must_use]
fn quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars().all(is_safe_char) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// Join arguments with spaces after quoting each. Pinned by
/// `tests/vectors/shell/join.json`.
#[must_use]
pub(crate) fn join(parts: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    parts
        .into_iter()
        .map(|p| quote(p.as_ref()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Remote command templates: the literal commands run on a refhost's
/// `/bin/sh`, pinned by `tests/vectors/shell/command_templates.json`.
pub(crate) mod cmd {
    use super::join;

    /// `zypper -n ar {params} {name} {url} {name}` with shell-quoted interpolations.
    #[must_use]
    pub(crate) fn zypper_ar(refresh: bool, name: &str, url: &str) -> String {
        let params = if refresh { "-cfkn" } else { "-ckn" };
        join(["zypper", "-n", "ar", params, name, url, name])
    }

    #[must_use]
    pub(crate) fn zypper_rr(aliases: &[&str]) -> String {
        let mut parts: Vec<&str> = vec!["zypper", "-n", "rr"];
        parts.extend_from_slice(aliases);
        join(parts)
    }

    #[must_use]
    pub(crate) fn zypper_in_products(products: &[&str]) -> String {
        let mut parts: Vec<&str> = vec!["zypper", "-n", "in", "-t", "product", "-l", "-f"];
        parts.extend_from_slice(products);
        join(parts)
    }

    #[must_use]
    pub(crate) fn transactional_in_products(products: &[&str]) -> String {
        let mut parts: Vec<&str> = vec![
            "transactional-update",
            "-n",
            "pkg",
            "in",
            "-t",
            "product",
            "-l",
            "-f",
        ];
        parts.extend_from_slice(products);
        join(parts)
    }

    #[must_use]
    pub(crate) fn zypper_rm_products(products: &[&str]) -> String {
        let mut parts: Vec<&str> = vec!["zypper", "-n", "rm", "-t", "product"];
        parts.extend_from_slice(products);
        join(parts)
    }

    #[must_use]
    pub(crate) fn transactional_rm_products(products: &[&str]) -> String {
        let mut parts: Vec<&str> = vec![
            "transactional-update",
            "-n",
            "pkg",
            "rm",
            "-t",
            "product",
            "-l",
            "-f",
        ];
        parts.extend_from_slice(products);
        join(parts)
    }

    pub(crate) const REFCMD: &str = "zypper -n --gpg-auto-import-keys ref -f";
    pub(crate) const REFTCMD: &str =
        "transactional-update -n run zypper -n --gpg-auto-import-keys ref -f";
    pub(crate) const REBOOT: &str = "systemctl reboot";
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn vector(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/vectors/shell")
            .join(name)
    }

    #[test]
    fn quote_matches_vector() {
        let raw = std::fs::read_to_string(vector("quote.json")).unwrap();
        let cases: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        for case in cases {
            let input = case["input"].as_str().unwrap();
            let expected = case["quoted"].as_str().unwrap();
            assert_eq!(quote(input), expected, "quote({input:?})");
        }
    }

    #[test]
    fn join_matches_vector() {
        let raw = std::fs::read_to_string(vector("join.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let parts: Vec<&str> = v["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        let expected = v["joined"].as_str().unwrap();
        assert_eq!(join(parts), expected);
    }

    #[test]
    fn command_templates_match_vector() {
        let raw = std::fs::read_to_string(vector("command_templates.json")).unwrap();
        let expected: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let evil_name = "evil repo's name";
        let evil_url = "http://mirror.example.com/dist path/?foo=1&bar=2";
        let evil_alias = "evil alias's";
        let evil_product = "prod uct's";

        assert_eq!(
            cmd::zypper_ar(false, evil_name, evil_url),
            expected["add_ckn"].as_str().unwrap()
        );
        assert_eq!(
            cmd::zypper_ar(true, evil_name, evil_url),
            expected["add_cfkn"].as_str().unwrap()
        );
        assert_eq!(
            cmd::zypper_rr(&[evil_alias]),
            expected["rr"].as_str().unwrap()
        );
        assert_eq!(
            cmd::zypper_in_products(&[evil_product]),
            expected["in"].as_str().unwrap()
        );
        assert_eq!(
            cmd::transactional_in_products(&[evil_product]),
            expected["in_t"].as_str().unwrap()
        );
        assert_eq!(
            cmd::zypper_rm_products(&[evil_product]),
            expected["rm"].as_str().unwrap()
        );
        assert_eq!(
            cmd::transactional_rm_products(&[evil_product]),
            expected["rm_t"].as_str().unwrap()
        );
    }
}
