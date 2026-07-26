//! OpenSSH-style glob matching shared by `known_hosts` patterns
//! ([`crate::hostkey`]) and `ssh_config` `Host` patterns
//! ([`crate::openssh_config`]). `*` matches any run of characters,
//! `?` matches exactly one; there are no character classes. Matching is
//! case-insensitive, as OpenSSH's `match_hostname` is — host names are not
//! case-sensitive, and treating them as if they were turns a `known_hosts`
//! entry written in another case into a miss, which under `accept-new`
//! silently re-pins the host instead of refusing it.

/// Iterative two-pointer match (linear-ish, no recursion): on a mismatch after
/// a `*`, backtrack the value by one position past the last star instead of
/// exploring every split recursively — pathological patterns like `"****...a"`
/// therefore cannot cause exponential blowup.
pub(crate) fn glob_matches(pattern: &str, value: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let v: Vec<char> = value.to_lowercase().chars().collect();
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
        assert!(glob_matches(&(pattern + "*"), &(value + "z")));
        assert!(start.elapsed() < std::time::Duration::from_millis(200));
    }

    /// A `known_hosts` entry written in another case must still match, or
    /// `accept-new` treats the host as unseen and re-pins it. OpenSSH's
    /// `match_hostname` lowercases both sides for the same reason.
    #[test]
    fn matching_ignores_case_on_both_sides() {
        assert!(glob_matches("HOST.EXAMPLE", "host.example"));
        assert!(glob_matches("host.example", "HOST.EXAMPLE"));
        assert!(glob_matches("[2001:DB8::1]:2222", "[2001:db8::1]:2222"));
        assert!(glob_matches("*.EXAMPLE", "a.example"));
        assert!(!glob_matches("other.example", "host.example"));
    }
}
