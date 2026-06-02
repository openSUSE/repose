"""Tests for ``repose.target.async_target.AsyncTarget``.

Mirrors ``tests/target/test_target.py`` for the async backend. The
``connector`` slot accepts any object satisfying the
``AsyncConnection`` shape, so a ``MagicMock`` with ``AsyncMock``
methods is plenty for these unit tests.
"""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

import pytest

from repose.aiossh import CommandTimeout
from repose.target.async_target import AsyncTarget
from repose.target.parsers import Product, Repository  # noqa: F401
from repose.types.system import System


@pytest.fixture
def make_target():
    """Build an AsyncTarget with a fully-mocked AsyncConnection."""

    def _factory(**conn_attrs):
        conn = MagicMock()
        # Default all I/O methods to AsyncMock so awaits resolve cleanly.
        for name in (
            "connect",
            "close",
            "run",
            "listdir",
            "readlink",
            "open",
        ):
            setattr(conn, name, AsyncMock())
        for k, v in conn_attrs.items():
            setattr(conn, k, v)
        target = AsyncTarget("h", 22, "u", connector=lambda *a, **kw: conn)
        return target, conn

    return _factory


def test_repr_includes_user_host_port(make_target):
    target, _ = make_target()
    text = repr(target)
    assert "u@h:22" in text
    assert "connected: False" in text


def test_bool_reflects_connection(make_target):
    target, _ = make_target()
    assert bool(target) is False
    target.is_connected = True
    assert bool(target) is True


async def test_connect_success(make_target):
    target, conn = make_target()
    await target.connect()
    conn.connect.assert_awaited_once()
    assert target.is_connected is True


async def test_connect_failure_logged_not_raised(make_target, caplog):
    target, conn = make_target()
    conn.connect.side_effect = RuntimeError("nope")

    with caplog.at_level("CRITICAL", logger="repose.target.async_target"):
        result = await target.connect()

    assert result is target
    assert target.is_connected is False
    assert any("connecting to h:22" in r.message.lower() for r in caplog.records)


async def test_connect_skipped_when_already_connected(make_target):
    target, conn = make_target()
    target.is_connected = True
    await target.connect()
    conn.connect.assert_not_awaited()


async def test_run_records_output_tuple(make_target):
    target, conn = make_target()
    conn.run.return_value = ("ok\n", "", 0)
    target.is_connected = True

    result = await target.run("echo ok")

    assert result == ("ok\n", "", 0)
    assert target.out[-1][0] == "echo ok"
    assert target.out[-1][1] == "ok\n"
    assert target.out[-1][3] == 0


async def test_run_handles_command_timeout(make_target, caplog):
    target, conn = make_target()
    conn.run.side_effect = CommandTimeout("sleep 1000")
    target.is_connected = True

    with caplog.at_level("CRITICAL", logger="repose.target.async_target"):
        result = await target.run("sleep 1000")

    assert result == ("", "", -1)
    assert target.out[-1] == ["sleep 1000", "", "", -1, 0]
    assert any("timed out" in r.message for r in caplog.records)


async def test_run_handles_assertion_error_returns_none(make_target):
    target, conn = make_target()
    conn.run.side_effect = AssertionError("zombie")

    result = await target.run("cmd")
    assert result is None
    # AssertionError path does not append to ``out``.
    assert target.out == []


async def test_run_handles_generic_exception(make_target, caplog):
    target, conn = make_target()
    conn.run.side_effect = RuntimeError("kaboom")

    with caplog.at_level("ERROR", logger="repose.target.async_target"):
        result = await target.run("cmd")

    assert result == ("", "", -1)
    assert any("failed to run command" in r.message for r in caplog.records)


async def test_read_products_caches_system(make_target, monkeypatch):
    """``read_products`` delegates to ``parse_system_async``."""
    target, conn = make_target()

    sentinel = System(Product("SLES", "15-SP5", "x86_64"))
    fake_parser = AsyncMock(return_value=sentinel)
    monkeypatch.setattr("repose.target.async_target.parse_system_async", fake_parser)

    await target.read_products()
    assert target.products is sentinel
    assert target.is_connected is True  # auto-connect kicked in
    fake_parser.assert_awaited_once_with(conn)


async def test_read_repos_short_circuits_when_not_connected(make_target, caplog):
    target, conn = make_target()
    # is_connected stays False.
    with caplog.at_level("DEBUG", logger="repose.target.async_target"):
        await target.read_repos()
    # No SSH call was attempted.
    conn.run.assert_not_awaited()


async def test_read_repos_parses_zypper_xml(make_target, sample_repos_xml):
    target, conn = make_target()
    target.is_connected = True
    conn.run.return_value = (sample_repos_xml, "", 0)

    await target.read_repos()

    assert target.raw_repos is not None
    aliases = {r.alias for r in target.raw_repos}
    assert {"repo-one", "repo-two"} <= aliases


async def test_read_repos_raises_on_unexpected_exitcode(make_target):
    target, conn = make_target()
    target.is_connected = True
    conn.run.return_value = ("", "boom", 5)

    with pytest.raises(ValueError):
        await target.read_repos()


async def test_close_marks_not_connected(make_target):
    target, conn = make_target()
    target.is_connected = True
    await target.close()
    conn.close.assert_awaited_once()
    assert target.is_connected is False


def test_report_helpers_remain_sync(make_target):
    target, _ = make_target()
    sink = MagicMock()
    target.report_products(sink)
    target.report_products_yaml(sink)
    target.report_repos(sink)
    assert sink.call_count == 3
