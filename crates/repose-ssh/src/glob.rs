//! OpenSSH-style glob matching shared by `known_hosts` patterns
//! ([`crate::hostkey`]) and `ssh_config` `Host` patterns
//! ([`crate::openssh_config`]). `*` matches any run of characters,
//! `?` matches exactly one; there are no character classes.

/// Iterative two-pointer match (linear-ish, no recursion): on a mismatch after
/// a `*`, backtrack the value by one position past the last star instead of
/// exploring every split recursively — pathological patterns like `"****...a"`
/// therefore cannot cause exponential blowup.
pub(crate) fn glob_matches(pattern: &str, value: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let v: Vec<char> = value.chars().collect();
    let (mut pi, mut vi) = (0usize, 0usize);
    // Position of the most recent `*` in the pattern and the value index the
    // current retry maps it to.
    let (mut star, mut retry) = (None::<usize>, 0usize);
    while vi < v.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == v[vi]) {
            pi += 1;
            vi += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            retry = vi;
            pi += 1;
        } else if let Some(s) = star {
            // Mismatch after a star: widen what the star swallows by one.
            pi = s + 1;
            retry += 1;
            vi = retry;
        } else {
            return false;
        }
    }
    // Only trailing stars may remain unconsumed.
    p[pi..].iter().all(|c| *c == '*')
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
        assert!(glob_matches("**", "anything"));
        assert!(glob_matches("a*b*c", "a-long-b-tail-c"));
        assert!(!glob_matches("a*b*c", "a-long-b-tail"));
        assert!(glob_matches("*?", "x"));
        assert!(!glob_matches("?*", ""));
    }

    #[test]
    fn pathological_star_runs_terminate_quickly() {
        // The old recursive matcher was exponential on adjacent `*`.
        let pattern = "*".repeat(40) + "z";
        let value = "a".repeat(200);
        let start = std::time::Instant::now();
        assert!(!glob_matches(&pattern, &value));
        assert!(glob_matches(&(pattern.clone() + "*"), &(value + "z")));
        assert!(start.elapsed() < std::time::Duration::from_millis(200));
    }
}
