"""Regression tests: runtime values must be shell-quoted in zypper cmds.

The command templates (``addcmd``, ``rrcmd``, ``ipdcmd``/``ipdtcmd``,
``rrpcmd``/``rrpdtcmd``) are ``str.format``-filled with repo names,
URLs, aliases, and product ids, and the result runs through the remote
login shell. A value containing whitespace or a quote used to pass
through verbatim, corrupting token boundaries (or worse, injecting into
the shell). Every test here feeds a value with a space *and* a single
quote through a command built the way the sibling command tests build
them (the real constructor with a mock ``Namespace``) and asserts, via
a ``shlex.split`` round-trip, that the command line reaching the target
keeps the value as one token.

Coverage is per format site: for every command both the sync and the
async worker are driven, and where the dry-run branch formats its own
command line (clear, reset) or a transactional host picks a different
template (install, uninstall) those twins are pinned too. Reverting the
quoting at any single site makes at least one test here fail (verified
by per-site mutation runs).
"""

import concurrent.futures
import shlex
from unittest.mock import AsyncMock, MagicMock

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.add import Add
from repose.command.clear import Clear
from repose.command.install import Install
from repose.command.remove import Remove
from repose.command.reset import Reset
from repose.command.uninstall import Uninstall
from repose.types.repa import Repa

EVIL_NAME = "evil repo's name"
EVIL_URL = "http://mirror.example.com/dist path/?foo=1&bar=2"
EVIL_ALIAS = "evil alias's"
EVIL_PRODUCT = "prod uct's"

ADD_TOKENS = ["zypper", "-n", "ar", "-ckn", EVIL_NAME, EVIL_URL, EVIL_NAME]
ADD_TOKENS_REFRESH = ["zypper", "-n", "ar", "-cfkn", EVIL_NAME, EVIL_URL, EVIL_NAME]
RR_ALIAS_TOKENS = ["zypper", "-n", "rr", EVIL_ALIAS]
IN_TOKENS = ["zypper", "-n", "in", "-t", "product", "-l", "-f", EVIL_PRODUCT]
IN_T_TOKENS = [
    "transactional-update",
    "-n",
    "pkg",
    "in",
    "-t",
    "product",
    "-l",
    "-f",
    EVIL_PRODUCT,
]
RM_TOKENS = ["zypper", "-n", "rm", "-t", "product", EVIL_PRODUCT]
RM_T_TOKENS = [
    "transactional-update",
    "-n",
    "pkg",
    "rm",
    "-t",
    "product",
    "-l",
    "-f",
    EVIL_PRODUCT,
]


class MockRepo:
    def __init__(self, name, url, refresh=False):
        self.name = name
        self.url = url
        self.refresh = refresh


class MockRawRepo:
    def __init__(self, alias):
        self.alias = alias


class MockProduct:
    def __init__(self, name, version):
        self.name = name
        self.version = version


def _ok_out():
    return [["cmd", "", "", 0, 0]]


def _noop_update(host: str, msg: str) -> None:
    """Progress-updater stub matching the ``UpdateFn`` signature."""


def _patch_hostgroup(monkeypatch, target, hosts=("user@host1",)):
    """Sync-backend scaffolding shared with the sibling command tests.

    ``ImmediateExecutor`` in place of the thread pool plus a
    ``HostGroup`` mock that resolves every host to ``target``.
    """
    hg = MagicMock()
    hg.keys.return_value = list(hosts)
    hg.__getitem__.return_value = target
    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command, "HostGroup", MagicMock(return_value=hg)
    )
    return hg


def _async_target():
    """A target whose ``run`` is awaitable, for direct ``_arun_one`` calls.

    Mirrors the async tests in ``test_reset.py``: build the command via
    the real constructor with ``ssh_backend="asyncssh"``, then swap in a
    dict of per-host mocks for ``targets``.
    """
    target = MagicMock()
    target.out = _ok_out()
    target.run = AsyncMock()
    return target


# ---------------------------------------------------------------------------
# add: ``addcmd`` in the sync ``_add`` and async ``_aadd`` workers
# ---------------------------------------------------------------------------


def test_add_sync_quotes_name_and_url(monkeypatch, make_args, mock_ssh_client):
    args = make_args(repa=[Repa("dummy-repa")], no_probe=True)
    target = MagicMock()
    target.products.get_base.return_value = "base"
    target.out = _ok_out()
    _patch_hostgroup(monkeypatch, target)
    repoq = MagicMock()
    repoq.solve_repa.return_value = {"product": [MockRepo(EVIL_NAME, EVIL_URL)]}
    monkeypatch.setattr(Add, "repoq", repoq)

    assert Add(args).run() == 0

    (produced,) = [c.args[0] for c in target.run.call_args_list]
    assert shlex.split(produced) == ADD_TOKENS


async def test_add_async_quotes_name_and_url(monkeypatch, make_args):
    args = make_args(repa=[Repa("dummy-repa")], no_probe=True, ssh_backend="asyncssh")
    repoq = MagicMock()
    repoq.solve_repa.return_value = {
        "product": [MockRepo(EVIL_NAME, EVIL_URL, refresh=True)]
    }
    monkeypatch.setattr(Add, "repoq", repoq)
    add = Add(args)
    target = _async_target()
    target.products.get_base.return_value = "base"
    add.targets = {"user@host1": target}

    assert await add._arun_one("user@host1", _noop_update) is True

    (produced,) = [c.args[0] for c in target.run.await_args_list]
    assert shlex.split(produced) == ADD_TOKENS_REFRESH


# ---------------------------------------------------------------------------
# clear: ``rrcmd`` — dry-run and real execution each format their own line
# ---------------------------------------------------------------------------


@pytest.mark.parametrize("dry", [True, False], ids=["dry", "real"])
def test_clear_sync_quotes_alias(monkeypatch, make_args, mock_ssh_client, dry):
    args = make_args(dry=dry)
    target = MagicMock()
    target.raw_repos = [MockRawRepo(EVIL_ALIAS)]
    _patch_hostgroup(monkeypatch, target)

    clear = Clear(args)
    clear.console = MagicMock()
    assert clear.run() == 0

    if dry:
        produced = clear.console.dry.call_args.args[1]
        target.run.assert_not_called()
    else:
        (produced,) = [c.args[0] for c in target.run.call_args_list]
    assert shlex.split(produced) == RR_ALIAS_TOKENS


@pytest.mark.parametrize("dry", [True, False], ids=["dry", "real"])
async def test_clear_async_quotes_alias(make_args, dry):
    args = make_args(dry=dry, ssh_backend="asyncssh")
    clear = Clear(args)
    clear.console = MagicMock()
    target = _async_target()
    target.raw_repos = [MockRawRepo(EVIL_ALIAS)]
    clear.targets = {"user@host1": target}

    assert await clear._arun_one("user@host1", _noop_update) is True

    if dry:
        produced = clear.console.dry.call_args.args[1]
        target.run.assert_not_awaited()
    else:
        (produced,) = [c.args[0] for c in target.run.await_args_list]
    assert shlex.split(produced) == RR_ALIAS_TOKENS


# ---------------------------------------------------------------------------
# install: ``addcmd`` + ``ipdcmd``/``ipdtcmd`` in both workers
# ---------------------------------------------------------------------------


def test_install_sync_quotes_repo_and_products(monkeypatch, make_args, mock_ssh_client):
    args = make_args(repa=[Repa("dummy-repa")], no_probe=True)
    target = MagicMock()
    target.products.get_base.return_value = "base"
    target.products.is_transactional.return_value = False
    target.out = _ok_out()
    _patch_hostgroup(monkeypatch, target)
    repoq = MagicMock()
    repoq.solve_repa.return_value = {EVIL_PRODUCT: [MockRepo(EVIL_NAME, EVIL_URL)]}
    monkeypatch.setattr(Install, "repoq", repoq)

    assert Install(args).run() == 0

    issued = [c.args[0] for c in target.run.call_args_list]
    assert shlex.split(issued[0]) == ADD_TOKENS
    assert shlex.split(issued[-1]) == IN_TOKENS


def test_install_sync_transactional_quotes_products(
    monkeypatch, make_args, mock_ssh_client
):
    args = make_args(repa=[Repa("dummy-repa")], no_probe=True, no_reboot=True)
    target = MagicMock()
    target.products.get_base.return_value = "base"
    target.products.is_transactional.return_value = True
    target.out = _ok_out()
    _patch_hostgroup(monkeypatch, target)
    repoq = MagicMock()
    repoq.solve_repa.return_value = {EVIL_PRODUCT: []}
    monkeypatch.setattr(Install, "repoq", repoq)

    assert Install(args).run() == 0

    issued = [c.args[0] for c in target.run.call_args_list]
    assert shlex.split(issued[-1]) == IN_T_TOKENS


async def test_install_async_quotes_repo_and_products(monkeypatch, make_args):
    args = make_args(repa=[Repa("dummy-repa")], no_probe=True, ssh_backend="asyncssh")
    repoq = MagicMock()
    repoq.solve_repa.return_value = {
        EVIL_PRODUCT: [MockRepo(EVIL_NAME, EVIL_URL, refresh=True)]
    }
    monkeypatch.setattr(Install, "repoq", repoq)
    install = Install(args)
    target = _async_target()
    target.products.get_base.return_value = "base"
    target.products.is_transactional.return_value = False
    install.targets = {"user@host1": target}

    assert await install._arun_one("user@host1", _noop_update) is True

    issued = [c.args[0] for c in target.run.await_args_list]
    assert shlex.split(issued[0]) == ADD_TOKENS_REFRESH
    assert shlex.split(issued[-1]) == IN_TOKENS


async def test_install_async_transactional_quotes_products(monkeypatch, make_args):
    args = make_args(
        repa=[Repa("dummy-repa")],
        no_probe=True,
        no_reboot=True,
        ssh_backend="asyncssh",
    )
    repoq = MagicMock()
    repoq.solve_repa.return_value = {EVIL_PRODUCT: []}
    monkeypatch.setattr(Install, "repoq", repoq)
    install = Install(args)
    target = _async_target()
    target.products.get_base.return_value = "base"
    target.products.is_transactional.return_value = True
    install.targets = {"user@host1": target}

    assert await install._arun_one("user@host1", _noop_update) is True

    issued = [c.args[0] for c in target.run.await_args_list]
    assert shlex.split(issued[-1]) == IN_T_TOKENS


# ---------------------------------------------------------------------------
# remove: ``rrcmd`` in the sync ``_run`` and async ``_arun_one`` workers
# ---------------------------------------------------------------------------

REMOVE_ALIAS = f"SLES:15-SP4::{EVIL_ALIAS}"


def test_remove_sync_quotes_alias(monkeypatch, make_args, mock_ssh_client):
    args = make_args(repa=[Repa(f"SLES:15-SP4::{EVIL_ALIAS}")])
    target = MagicMock()
    target.products.flatten.return_value = [MockProduct("SLES", "15-SP4")]
    target.repos.keys.return_value = [REMOVE_ALIAS]
    target.out = _ok_out()
    _patch_hostgroup(monkeypatch, target)

    assert Remove(args).run() == 0

    (produced,) = [c.args[0] for c in target.run.call_args_list]
    assert shlex.split(produced) == ["zypper", "-n", "rr", REMOVE_ALIAS]


async def test_remove_async_quotes_alias(make_args):
    args = make_args(repa=[Repa(f"SLES:15-SP4::{EVIL_ALIAS}")], ssh_backend="asyncssh")
    remove = Remove(args)
    target = _async_target()
    target.products.flatten.return_value = [MockProduct("SLES", "15-SP4")]
    target.repos.keys.return_value = [REMOVE_ALIAS]
    remove.targets = {"user@host1": target}

    assert await remove._arun_one("user@host1", _noop_update) is True

    (produced,) = [c.args[0] for c in target.run.await_args_list]
    assert shlex.split(produced) == ["zypper", "-n", "rr", REMOVE_ALIAS]


# ---------------------------------------------------------------------------
# reset: ``addcmd`` in ``_add``/``_aadd`` plus the four ``rrcmd`` sites in
# its own ``_run``/``_arun_one`` overrides (dry-run and real, sync and async)
# ---------------------------------------------------------------------------


@pytest.mark.parametrize("dry", [True, False], ids=["dry", "real"])
def test_reset_sync_quotes_alias_and_repo(monkeypatch, make_args, mock_ssh_client, dry):
    args = make_args(dry=dry, repa=None, no_probe=True)
    target = MagicMock()
    target.raw_repos = [MockRawRepo(EVIL_ALIAS)]
    target.out = _ok_out()
    _patch_hostgroup(monkeypatch, target)
    repoq = MagicMock()
    repoq.solve_product.return_value = {"product": [MockRepo(EVIL_NAME, EVIL_URL)]}
    monkeypatch.setattr(Reset, "repoq", repoq)

    reset = Reset(args)
    reset.console = MagicMock()
    assert reset.run() == 0

    if dry:
        issued = [c.args[1] for c in reset.console.dry.call_args_list]
        target.run.assert_not_called()
    else:
        issued = [c.args[0] for c in target.run.call_args_list]
    assert shlex.split(issued[0]) == RR_ALIAS_TOKENS
    assert shlex.split(issued[1]) == ADD_TOKENS


@pytest.mark.parametrize("dry", [True, False], ids=["dry", "real"])
async def test_reset_async_quotes_alias_and_repo(monkeypatch, make_args, dry):
    args = make_args(dry=dry, repa=None, no_probe=True, ssh_backend="asyncssh")
    repoq = MagicMock()
    repoq.solve_product.return_value = {"product": [MockRepo(EVIL_NAME, EVIL_URL)]}
    monkeypatch.setattr(Reset, "repoq", repoq)
    reset = Reset(args)
    reset.console = MagicMock()
    target = _async_target()
    target.raw_repos = [MockRawRepo(EVIL_ALIAS)]
    reset.targets = {"user@host1": target}

    assert await reset._arun_one("user@host1", _noop_update) is True

    if dry:
        issued = [c.args[1] for c in reset.console.dry.call_args_list]
        target.run.assert_not_awaited()
    else:
        issued = [c.args[0] for c in target.run.await_args_list]
    assert shlex.split(issued[0]) == RR_ALIAS_TOKENS
    assert shlex.split(issued[1]) == ADD_TOKENS


# ---------------------------------------------------------------------------
# uninstall: ``rrcmd`` + the products argument shared by ``rrpcmd`` and the
# transactional ``rrpdtcmd`` in both workers
# ---------------------------------------------------------------------------

UNINSTALL_ALIAS = f"{EVIL_PRODUCT}:12::{EVIL_ALIAS}"


def test_uninstall_sync_quotes_alias_and_product(
    monkeypatch, make_args, mock_ssh_client
):
    args = make_args(repa=[Repa(f"{EVIL_PRODUCT}:12")])
    target = MagicMock()
    target.products.flatten.return_value = [MockProduct(EVIL_PRODUCT, "12")]
    target.products.is_transactional.return_value = False
    target.repos = {UNINSTALL_ALIAS: MockRepo(EVIL_PRODUCT, "http://unused")}
    target.out = _ok_out()
    _patch_hostgroup(monkeypatch, target)

    assert Uninstall(args).run() == 0

    issued = [c.args[0] for c in target.run.call_args_list]
    assert shlex.split(issued[0]) == ["zypper", "-n", "rr", UNINSTALL_ALIAS]
    assert shlex.split(issued[1]) == RM_TOKENS


async def test_uninstall_async_transactional_quotes_alias_and_product(make_args):
    args = make_args(
        repa=[Repa(f"{EVIL_PRODUCT}:12")],
        no_reboot=True,
        ssh_backend="asyncssh",
    )
    uninstall = Uninstall(args)
    target = _async_target()
    target.products.flatten.return_value = [MockProduct(EVIL_PRODUCT, "12")]
    target.products.is_transactional.return_value = True
    target.repos = {UNINSTALL_ALIAS: MockRepo(EVIL_PRODUCT, "http://unused")}
    uninstall.targets = {"user@host1": target}

    # ``repo`` is already None in the REPA, matching the orepa that
    # ``_arun`` passes through to the per-host worker.
    ok = await uninstall._arun_one("user@host1", _noop_update, uninstall.repa)
    assert ok is True

    issued = [c.args[0] for c in target.run.await_args_list]
    assert shlex.split(issued[0]) == ["zypper", "-n", "rr", UNINSTALL_ALIAS]
    assert shlex.split(issued[1]) == RM_T_TOKENS
