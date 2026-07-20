# Security Policy

## Reporting a vulnerability

Please report suspected security vulnerabilities privately via GitHub's
[**Report a vulnerability**](https://github.com/openSUSE/repose/security/advisories/new)
(Security → Advisories). Do not open a public issue for a security report.

For SUSE-internal QAM infrastructure impact, also notify the SUSE QA
Maintenance team.

## Scope

`repose` connects to reference hosts over SSH and runs `zypper` /
`transactional-update` on them. Security-relevant areas include SSH host-key
verification (`~/.ssh/known_hosts`, including hashed entries), remote command
construction (shell quoting), and repository/URL handling. The dependency tree
is gated in CI by `cargo-deny` (RUSTSEC advisories, license and source policy),
and the code is scanned by CodeQL.

## Supported versions

The latest release from `master` is supported. `repose` is a single-package
replacement (no parallel-installed older line).
