# repose

Manipulate repositories in QAM refhosts

## Introduction
**Repose** is a tools for querying and manipulation of repositories in SUSE QA Maintenance reference machines.

*Repose* allows for manipulation of repositories in refhosts requiring only a running sshd and zypper installed on them

### Installation

```
zypper ar -f http://download.suse.de/ibs/QA:/Maintenance/$DISTRO/ qam-infra
zypper -n in repose
```

After install, `man repose` shows the full command reference.

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

Then in a new shell, tab-complete subcommands and flags:

```
repose <TAB>           # add remove reset install clear uninstall ...
```

Regenerate the committed completions (and man pages) from the CLI with:

```
cargo run -p repose-cli --features gen --bin repose-gen -- repose-cli
```

## Internal Functionality

Repose reports or modifies the package repositories in one or more refhosts
based on installed products (/etc/products.d/), repository configuration (/etc
/zypp/repos.d), and user input; commands are sent via ssh.

Three steps are conducted by repose:
1. refhost is queried
2. product info is provided back to repose
3. repose executes zypper commands on refhost

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
repose install -t root@slmicro.example qa        # install + reboot + verify
repose --no-reboot install -t root@slmicro.example qa   # stage only
```

## Getting Help

oFor repose itself as well as for its commands you can use:

Options:
-h        Display this message
--help    Display full help

Using parameter –-help will open up a man page.

## General Usage
Usage of repose is pretty straightforward.

repose COMMAND options [-h] -t HOST REPA

Commands:
    add               add specified repository to target
    remove            remove repository from target
    reset             reset target repositories to only installed products repositories
    install           add specified repository to target and install product
    clear             clear all repositories from target
    uninstall         remove specified repository from target and uninstall product
    list-products     list products on target
    list-repos        list repositories on target
    known-products    list known products by 'repose'

‘’HOST’’ is supposed to be added in format `root@fubar.suse.cz`. You can add multiple hosts
‘’REPA’’ is REpository PAttern. You can use multiple patterns.
You can also add specific versions after colon.
For example:
SLES 12 SP2: SLES:12-SP2
You can find more at /etc/repose/products.yml

### Live progress

When stdout is a terminal, repose draws a per-host status table that
updates as each refhost moves through its work (e.g. *resolving repos*,
*adding 3 repo(s)*, *done*). The overlay drops back to plain log lines
automatically when output is piped, `--format=json` is used, or
`--quiet` is set, so scripts and structured-output consumers see a
clean stream.

## Repository URL Probing

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

## Most Common Usage Examples
Setup of repositories on refhost:

```
repose reset -t fubar.suse.cz
repose install -t fubar.suse.cz qa
```

Adding SDK repository to SLE of any version:

```
repose add -t fubar.suse.cz sle-sdk
```

Adding specificaly SDK repository of SLE 12 SP2:

```
repose add -t fubar.suse.cz sle-sdk:12-SP2
```

Adding multiple add-ons on multiple machines:

```
repose add -t fubar.suse.cz -t snafu.suse.cz qa sle-sdk
```

Additional modules: sle-module-toolchain - sle-module-public-cloud - sle-module-legacy - sle-module-hpc - sle-module-containers - sle-module-
adv-systems-management - sle-live-patching - sle-bsk - sle-ha - sle-we - sle-web-scripting

Show products in yaml format needed for refhost.yaml genetor:

```
repose list-products --yaml -t foobar.suse.cz
```

## Output Control

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

### Examples

```
repose add -n --format=json -t fubar.suse.cz sle-sdk | jq .
repose list-products --format=json -t fubar.suse.cz | jq 'select(.kind=="base")'
repose known-products --format=json | jq -r '.name'
```

Note: per-host run output (previously emitted via the logger at info/warning
level) now goes through this sink on stdout. The `--quiet` flag still
silences logger messages but no longer hides per-host output; redirect
stdout or use `--format=json` with filtering to suppress it.

## SSH Host-Key Policy

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

## SSH Backend

Repose uses a single SSH stack ([russh](https://crates.io/crates/russh)); there
is no `--ssh-backend` flag. It honours the same `--strict-host-key-checking`,
`--known-hosts`, and `~/.ssh/config` directives described above.

Authentication tries the ssh-agent first (every agent identity is offered),
then `IdentityFile` keys from `~/.ssh/config`. Unlike `ssh(1)`, the
`IdentitiesOnly` directive is not honoured: agent identities are offered even
when `IdentitiesOnly yes` is set for a host.

## Building

Repose is a Rust workspace under `crates/`. Build the binary with:

```
cargo build --release -p repose-cli --manifest-path crates/Cargo.toml
# binary at crates/target/release/repose
```

## License

This project is licensed under the GPLv3 license, see LICENSE file for
details.

