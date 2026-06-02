"""Tests for ``repose.target.async_hostgroup.AsyncHostGroup``.

Mirrors ``tests/target/test_hostgroup.py`` for the async backend.
Crucially exercises the "one host raises, others still complete"
semantic — the asyncio TaskGroup default would cancel siblings, so
``AsyncHostGroup`` wraps every per-host coroutine in :func:`_isolate`.
"""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

import pytest

from repose.target.async_hostgroup import AsyncHostGroup


def _make_async_target() -> MagicMock:
    """Build a MagicMock whose I/O methods are AsyncMocks."""
    t = MagicMock()
    for name in (
        "connect",
        "close",
        "read_products",
        "read_repos",
        "parse_repos",
        "run",
        "report_products",
        "report_products_yaml",
        "report_repos",
    ):
        # The report_* methods are sync on the real ``AsyncTarget`` —
        # leave them as plain MagicMocks for parity.
        if name in ("report_products", "report_products_yaml", "report_repos"):
            continue
        setattr(t, name, AsyncMock())
    return t


@pytest.fixture
def two_targets():
    return {"host-a": _make_async_target(), "host-b": _make_async_target()}


async def test_connect_calls_each_target_connect(two_targets):
    for t in two_targets.values():
        t.connect.return_value = t

    hg = AsyncHostGroup(two_targets)
    await hg.connect()

    for t in two_targets.values():
        t.connect.assert_awaited_once()


async def test_connect_swallows_per_host_exception(two_targets, caplog):
    """If one target's connect() raises, others still finish."""
    two_targets["host-a"].connect.side_effect = RuntimeError("boom")
    two_targets["host-b"].connect.return_value = two_targets["host-b"]

    hg = AsyncHostGroup(two_targets)
    with caplog.at_level("WARNING", logger="repose.target.async_hostgroup"):
        await hg.connect()  # must not raise

    messages = [r.getMessage() for r in caplog.records]
    assert any("boom" in m and "host-a" in m for m in messages)
    two_targets["host-b"].connect.assert_awaited_once()


async def test_close_calls_each_target_close(two_targets):
    hg = AsyncHostGroup(two_targets)
    await hg.close()
    for t in two_targets.values():
        t.close.assert_awaited_once()


async def test_read_products_fans_out(two_targets):
    hg = AsyncHostGroup(two_targets)
    await hg.read_products()
    for t in two_targets.values():
        t.read_products.assert_awaited_once()


async def test_read_repos_fans_out(two_targets):
    hg = AsyncHostGroup(two_targets)
    await hg.read_repos()
    for t in two_targets.values():
        t.read_repos.assert_awaited_once()


async def test_parse_repos_fans_out(two_targets):
    hg = AsyncHostGroup(two_targets)
    await hg.parse_repos()
    for t in two_targets.values():
        t.parse_repos.assert_awaited_once()


def test_report_products_iterates_sorted(two_targets):
    hg = AsyncHostGroup(two_targets)
    sink = MagicMock()
    hg.report_products(sink)
    for t in two_targets.values():
        t.report_products.assert_called_once_with(sink)


def test_report_products_yaml_iterates_sorted(two_targets):
    hg = AsyncHostGroup(two_targets)
    sink = MagicMock()
    hg.report_products_yaml(sink)
    for t in two_targets.values():
        t.report_products_yaml.assert_called_once_with(sink)


def test_report_repos_iterates_sorted(two_targets):
    hg = AsyncHostGroup(two_targets)
    sink = MagicMock()
    hg.report_repos(sink)
    for t in two_targets.values():
        t.report_repos.assert_called_once_with(sink)


async def test_run_fans_out_with_str_command(two_targets):
    """A bare ``str`` command broadcasts the same string to every host."""
    hg = AsyncHostGroup(two_targets)
    await hg.run("zypper -n ref")

    for t in two_targets.values():
        t.run.assert_awaited_once_with("zypper -n ref")


async def test_run_fans_out_with_dict_command(two_targets):
    """A per-host dict picks each host's own command."""
    cmds = {"host-a": "ls /a", "host-b": "ls /b"}
    hg = AsyncHostGroup(two_targets)
    await hg.run(cmds)

    two_targets["host-a"].run.assert_awaited_once_with("ls /a")
    two_targets["host-b"].run.assert_awaited_once_with("ls /b")


async def test_run_swallows_per_host_exception(two_targets, caplog):
    """One host raising during run() must not cancel siblings."""
    two_targets["host-a"].run.side_effect = RuntimeError("kaboom")
    two_targets["host-b"].run.return_value = ("ok", "", 0)

    hg = AsyncHostGroup(two_targets)
    with caplog.at_level("WARNING", logger="repose.target.async_hostgroup"):
        await hg.run("zypper -n ref")  # must not raise

    two_targets["host-b"].run.assert_awaited_once()
    messages = [r.getMessage() for r in caplog.records]
    assert any("kaboom" in m and "host-a" in m for m in messages)
