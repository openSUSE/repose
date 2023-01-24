[![CodeQL](https://github.com/openSUSE/repose/actions/workflows/codeql-analysis.yml/badge.svg)](https://github.com/openSUSE/repose/actions/workflows/codeql-analysis.yml)
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

## Internal Functionality

Repose reports or modifies the package repositories in one or more refhosts
based on installed products (/etc/products.d/), repository configuration (/etc
/zypp/repos.d), and user input; commands are sent via ssh.

Three steps are conducted by repose:
1. refhost is queried
2. product info is provided back to repose
3. repose executes zypper commands on refhost

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

## License

This project is licensed under the GPLv3 license, see LICENSE file for
details.

