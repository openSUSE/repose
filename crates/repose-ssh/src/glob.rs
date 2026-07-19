//! OpenSSH-style glob matching shared by `known_hosts` patterns
//! ([`crate::hostkey`]) and `ssh_config` `Host` patterns
//! ([`crate::openssh_config`]). `*` matches any run of characters,
//! `?` matches exactly one; there are no character classes.

pub(crate) fn glob_matches(pattern: &str, value: &str) -> bool {
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
    use super::glob_matches;

    #[test]
    fn literal_star_and_question_mark() {
        assert!(glob_matches("host", "host"));
        assert!(!glob_matches("host", "host2"));
        assert!(glob_matches("*.example", "a.example"));
        assert!(!glob_matches("*.example", "a.example.org"));
        assert!(glob_matches("h?st", "host"));
        assert!(!glob_matches("h?st", "hst"));
        assert!(glob_matches("*", ""));
    }
}
