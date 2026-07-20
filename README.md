# repose

Manipulate zypper repositories and products on QAM reference hosts, over SSH.

## Introduction

**Repose** queries and manipulates the package repositories and installed
products of SUSE QA Maintenance reference machines. It needs nothing on the
refhost beyond a running `sshd` and `zypper` — all decisions are made locally
and the resulting `zypper` (or `transactional-update`) commands are sent over
SSH.

Given one or more hosts and a set of repository patterns, repose figures out
which repositories the host's installed products require and adds, removes,
resets, or clears them accordingly. It can also install or uninstall whole
products, including the transactional-update reboot cycle on immutable hosts.

## Installation

```
zypper ar -f http://download.suse.de/ibs/QA:/Maintenance/$DISTRO/ qam-infra
zypper -n in repose
```

After install, `man repose` shows the full command reference, and every
subcommand accepts `-h`/`--help`.

## How it works

Repose derives repository changes from three inputs — the products installed
on the host (`/etc/products.d/`), its current repository configuration
(`/etc/zypp/repos.d/`), and the repository template in
`/etc/repose/products.yml` — then applies them in three steps:

1. the refhost is queried over SSH;
2. its product/repository state is sent back to repose;
3. repose runs the resulting `zypper` commands on the refhost.

## Quick start

Set up the repositories for a refhost and install the `qa` product:

```
repose reset -t fubar.suse.cz
repose install -t fubar.suse.cz qa
```

Add the SDK repository to whatever SLE version the host runs:

```
repose add -t fubar.suse.cz sle-sdk
```

Add the SDK repository for a specific version (append the version after a colon):

```
repose add -t fubar.suse.cz sle-sdk:12-SP2
```

Add several add-ons across several hosts in one run:

```
repose add -t fubar.suse.cz -t snafu.suse.cz qa sle-sdk
```

Emit a YAML host spec for the `refhosts.yml` generator:

```
repose list-products --yaml -t foobar.suse.cz
```

Preview the commands without touching the host (dry run):

```
repose -n add -t fubar.suse.cz sle-sdk
```

## Commands

```
repose [GLOBAL OPTIONS] COMMAND [OPTIONS] -t HOST [REPA ...]
```

| Command          | Description                                                            |
| ---------------- | --------------------------------------------------------------------- |
| `add`            | add the specified repositories to the target                          |
| `remove`         | remove repositories from the target                                   |
| `reset`          | reset the target to only its installed products' repositories         |
| `clear`          | clear all repositories from the target                                |
| `install`        | add repositories to the target and install the product               |
| `uninstall`      | remove repositories from the target and uninstall the product         |
| `list-products`  | list the products installed on the target                             |
| `list-repos`     | list the repositories configured on the target                        |
| `known-products` | list the products repose knows about (from `products.yml`)            |

Frequently used global options (run `repose --help` for the full list):

| Option                              | Description                                            |
| ----------------------------------- | ------------------------------------------------------ |
| `-n`, `--print`                     | print the commands that would run, then exit (dry run) |
| `-c`, `--config PATH`               | repose configuration (default `/etc/repose/products.yml`) |
| `-d`, `--debug`                     | enable debug logging                                   |
| `-q`, `--quiet`                     | suppress repose's own log messages                     |
| `--no-color`                        | disable ANSI color (honors `NO_COLOR`)                 |
| `--format text\|json`               | output format (default `text`)                         |
| `--strict-host-key-checking MODE`   | SSH host-key policy (see below)                        |
| `--known-hosts PATH`                | known_hosts file (default `~/.ssh/known_hosts`)        |
| `-V`, `--version`                   | print the version and exit                             |

### Targets and repository patterns

- **HOST** is an SSH target such as `root@fubar.suse.cz`, passed with `-t`.
  Repeat `-t` to operate on multiple hosts concurrently.
- **REPA** is a *REpository PAttern* — a positional argument naming a
  repository/add-on to act on. Pass several to act on several. Append a
  version after a colon to pin it, e.g. `SLES:12-SP2`.

The known patterns are defined in the configuration file
(`/etc/repose/products.yml`); `repose known-products` lists them. Common
add-on modules include:

```
sle-module-toolchain      sle-module-public-cloud   sle-module-legacy
sle-module-hpc            sle-module-containers      sle-live-patching
sle-module-adv-systems-management   sle-bsk   sle-ha   sle-we   sle-web-scripting
```

## Transactional systems (SL Micro)

Repose auto-detects **transactional / immutable** hosts (SL Micro, SLE
Micro, MicroOS), where the root filesystem is a read-only snapshot. On
such hosts:

- **Repository changes** (`add`, `remove`, `reset`, `clear`) use plain
  `zypper` — `/etc/zypp/repos.d` is on a writable overlay, so nothing
  special is needed.
- **Product install/remove** (`install`, `uninstall`) is routed through
  **`transactional-update`** instead of `zypper`, because it modifies the
  read-only `/usr`. This is decided by the *host*, not the product — e.g.
  `repose install -t slmicro qa` installs `qa` transactionally.
- After a transactional package change, repose **reboots** the host into
  the new snapshot, **reconnects** (with retries/backoff), and
  **verifies** the product is actually installed (or gone, for
  `uninstall`) before reporting success.

Detection and routing are automatic — no flag is needed to enable them.
Pass `--no-reboot` to `install`/`uninstall` to **stage** the change
without rebooting (a reminder is logged); the snapshot only becomes
active after the next reboot. `--no-reboot` is a no-op on
non-transactional hosts.

```
repose install -t root@slmicro.example qa               # install + reboot + verify
repose install --no-reboot -t root@slmicro.example qa   # stage only
```

## Repository URL probing

Before `add`, `reset`, and `install` apply repository changes, repose
probes each candidate repository URL in parallel to verify it is
reachable, dropping repositories whose URLs fail to respond. Probes run
against the system trust store, so internal CAs are honored. Two flags
on these commands tune the behaviour:

- `--probe-timeout SECONDS`: seconds to wait per repository URL probe
  (default: `5`).
- `--no-probe`: skip the liveness probes entirely and request every
  candidate repository unchanged.

```
repose add --probe-timeout 10 -t fubar.suse.cz sle-sdk
repose reset --no-probe -t fubar.suse.cz
```

## Live progress

When stdout is a terminal, repose draws a per-host status table that
updates as each refhost moves through its work (e.g. *resolving repos*,
*adding 3 repo(s)*, *done*). The overlay drops back to plain log lines
automatically when output is piped, `--format=json` is used, or
`--quiet` is set, so scripts and structured-output consumers see a
clean stream.

## Output control

Repose routes all user-facing output (dry-run command previews and per-host
run output) through a single sink. Two global flags govern its shape:

- `--no-color`: disable ANSI color sequences. The
  [`NO_COLOR`](https://no-color.org) environment variable is also honored.
  By default, color is enabled only when stdout is a terminal. The legacy
  `COLOR=always|never` environment variable still overrides detection.
- `--format={text,json}`: select human-readable text (default) or
  newline-delimited JSON for scripts.

In JSON mode, every command emits newline-delimited JSON (one object per
line). Action commands (`add`, `install`, `remove`, `uninstall`, `clear`,
`reset`) emit event envelopes; query commands (`list-products`,
`list-repos`, `known-products`) emit payload events.

### Event envelopes (action commands)

| field   | type   | description                                              |
| ------- | ------ | -------------------------------------------------------- |
| `event` | string | `"dry"` \| `"report"` \| `"error"` \| `"info"`           |
| `level` | string | `"info"` \| `"warning"` \| `"error"`                     |
| `host`  | string | target host (omitted for unscoped `info` events)         |
| `cmd`   | string | the dry-run command (only for `event="dry"`)             |
| `line`  | string | a single output line (for `report`/`error`/`info`)       |
| `ok`    | bool   | per-host success flag (for `report`/`error`)             |

### Payload events (query commands)

`list-products --format=json` emits one `product` event per product per host
(`{event:"product", host, port, kind:"base"|"addon", name, version, arch}`).

`list-repos --format=json` emits one `repo` event per repository per host
(`{event:"repo", host, port, alias, name, url, state}`).

`known-products --format=json` emits one `known_product` event per known
product (`{event:"known_product", name}`).

`list-products --yaml --format=json` emits one `host_spec` event per host
carrying the same payload the YAML dumper produces (location, arch,
product, addons, name) — useful for machine consumers that want the
refhost.yml spec without the YAML envelope.

```
repose add -n --format=json -t fubar.suse.cz sle-sdk | jq .
repose list-products --format=json -t fubar.suse.cz | jq 'select(.kind=="base")'
repose known-products --format=json | jq -r '.name'
```

Note: per-host run output (previously emitted via the logger at info/warning
level) now goes through this sink on stdout. The `--quiet` flag still
silences logger messages but no longer hides per-host output; redirect
stdout or use `--format=json` with filtering to suppress it.

## SSH host-key policy

Repose talks to refhosts over SSH. The host-key behaviour follows OpenSSH's
`StrictHostKeyChecking` semantics and is configured with two global flags:

- `--strict-host-key-checking={yes,accept-new,no,off}` (default: `accept-new`)
- `--known-hosts PATH` (default: `~/.ssh/known_hosts`)

| Mode         | Unknown host (first contact)             | Changed host key             |
| ------------ | ---------------------------------------- | ---------------------------- |
| `yes`        | refuse (`BadHostKeyException`)           | refuse                       |
| `accept-new` | accept + record in known_hosts (default) | refuse                       |
| `no` / `off` | accept silently                          | accept silently (unsafe)     |

`accept-new` matches the OpenSSH default since 7.6 (2017): unknown hosts are
recorded on first contact, but a host whose key has *changed* since it was
first recorded is refused. This is a behaviour change from pre-1.12 releases,
which silently re-trusted any presented key (equivalent to `off`).

If you operate a QA pool where refhost keys legitimately rotate and you
cannot prune `known_hosts` between rotations, opt into the legacy behaviour
explicitly:

```
repose --strict-host-key-checking=off add -t fubar.suse.cz sle-sdk
```

For paranoid setups (each refhost key pre-recorded in a dedicated file),
pin both flags:

```
repose --strict-host-key-checking=yes \
       --known-hosts /etc/repose/known_hosts \
       add -t fubar.suse.cz sle-sdk
```

## SSH backend

Repose uses a single SSH stack ([russh](https://crates.io/crates/russh)); there
is no `--ssh-backend` flag. It honours the same `--strict-host-key-checking`,
`--known-hosts`, and `~/.ssh/config` directives described above.

Authentication tries the ssh-agent first (every agent identity is offered),
then `IdentityFile` keys from `~/.ssh/config`. Unlike `ssh(1)`, the
`IdentitiesOnly` directive is not honoured: agent identities are offered even
when `IdentitiesOnly yes` is set for a host.

## Shell completion

Repose ships pre-generated shell completions for bash, zsh, and fish
(`crates/repose-cli/completions/`, also installed by the package). Load the
one for your shell, for example:

```
# bash
source crates/repose-cli/completions/repose.bash
# zsh: put the `_repose` file on your $fpath
# fish: copy repose.fish into ~/.config/fish/completions/
```

Then, in a new shell, tab-complete subcommands and flags:

```
repose <TAB>           # add remove reset install clear uninstall ...
```

Regenerate the committed completions (and man pages) from the CLI with:

```
cargo run -p repose-cli --features gen --bin repose-gen -- crates/repose-cli
```

## Building

Repose is a root Rust workspace with crate sources under `crates/`. Build the
binary with:

```
cargo build --release -p repose-cli
# binary at target/release/repose
```

## License

This project is licensed under the GPLv3 license, see the LICENSE file for
details.
