"""Tests for ``repose.target.Target``."""

from unittest.mock import MagicMock

import pytest

from repose.connection import CommandTimeout
from repose.target import Target
from repose.target.parsers import Product, Repository
from repose.types.system import System


@pytest.fixture
def make_target():
    """Build a Target with a fully-mocked Connection."""

    def _factory(**conn_attrs):
        conn = MagicMock()
        for k, v in conn_attrs.items():
            setattr(conn, k, v)
        target = Target("h", 22, "u", connector=lambda *a, **kw: conn)
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


def test_connect_success(make_target):
    target, conn = make_target()
    target.connect()
    conn.connect.assert_called_once()
    assert target.is_connected is True


def test_connect_failure_logged_not_raised(make_target, caplog):
    target, conn = make_target()
    conn.connect.side_effect = RuntimeError("nope")

    with caplog.at_level("CRITICAL", logger="repose.target"):
        result = target.connect()

    assert result is target
    assert target.is_connected is False
    assert any("connecting to h:22" in r.message for r in caplog.records)


def test_connect_skipped_when_already_connected(make_target):
    target, conn = make_target()
    target.is_connected = True
    target.connect()
    conn.connect.assert_not_called()


def test_close_resets_connected_flag(make_target):
    target, conn = make_target()
    target.is_connected = True
    target.close()
    conn.close.assert_called_once()
    assert target.is_connected is False


def test_run_happy_path(make_target):
    target, conn = make_target()
    conn.run.return_value = ("out", "err", 0)

    result = target.run("ls")

    assert result == ("out", "err", 0)
    assert len(target.out) == 1
    assert target.out[0][0] == "ls"
    assert target.out[0][3] == 0  # exitcode


def test_run_command_timeout_records_minus_one(make_target, caplog):
    """``CommandTimeout`` is logged as critical and recorded with
    exitcode -1 plus empty stdout/stderr in ``self.out``."""
    target, conn = make_target()
    conn.run.side_effect = CommandTimeout("ls")

    with caplog.at_level("CRITICAL", logger="repose.target"):
        result = target.run("ls")

    assert result == ("", "", -1)
    assert target.out[-1][0] == "ls"
    assert target.out[-1][3] == -1
    assert any("timed out" in r.message for r in caplog.records)


def test_run_assertion_error_returns_none(make_target):
    target, conn = make_target()
    conn.run.side_effect = AssertionError()

    assert target.run("ls") is None
    # Early return — nothing appended.
    assert target.out == []


def test_run_generic_exception_records_minus_one(make_target, caplog):
    """Any other exception is logged at ERROR level and recorded with
    exitcode -1 plus empty stdout/stderr in ``self.out``."""
    target, conn = make_target()
    conn.run.side_effect = RuntimeError("boom")

    with caplog.at_level("ERROR", logger="repose.target"):
        result = target.run("ls")

    assert result == ("", "", -1)
    assert target.out[-1][3] == -1
    assert any("failed to run" in r.message for r in caplog.records)


def test_read_products_calls_parse_system(make_target, monkeypatch):
    target, conn = make_target()
    target.is_connected = True

    fake_system = System(Product("SLES", "15-SP3", "x86_64"))
    monkeypatch.setattr("repose.target.parse_system", lambda c: fake_system)

    target.read_products()
    assert target.products == fake_system


def test_read_products_connects_first_if_needed(make_target, monkeypatch):
    target, conn = make_target()
    monkeypatch.setattr(
        "repose.target.parse_system",
        lambda c: System(Product("X", "1", "noarch")),
    )

    target.read_products()
    conn.connect.assert_called_once()


@pytest.mark.parametrize("exitcode", [0, 106, 6])
def test_read_repos_acceptable_exitcodes(make_target, monkeypatch, exitcode):
    target, conn = make_target()
    target.is_connected = True
    conn.run.return_value = ("<xml/>", "", exitcode)
    parsed = {Repository("a", "n", "u", True)}
    monkeypatch.setattr("repose.target.parse_repositories", lambda x: parsed)

    target.read_repos()
    assert target.raw_repos == parsed


def test_read_repos_bad_exitcode_raises(make_target, caplog):
    target, conn = make_target()
    target.is_connected = True
    conn.run.return_value = ("", "err", 1)

    with caplog.at_level("ERROR", logger="repose.target"):
        with pytest.raises(ValueError, match="Can't read repositories"):
            target.read_repos()


def test_read_repos_skipped_when_not_connected(make_target):
    target, _ = make_target()
    # is_connected = False by default
    target.read_repos()
    assert target.raw_repos is None


def test_parse_repos_orchestrates_dependent_calls(make_target, monkeypatch):
    target, conn = make_target()
    target.is_connected = True
    fake_system = System(Product("SLES", "15-SP3", "x86_64"))
    monkeypatch.setattr("repose.target.parse_system", lambda c: fake_system)
    parsed = {Repository("a", "SLES:15-SP3::r", "u", True)}
    conn.run.return_value = ("<xml/>", "", 0)
    monkeypatch.setattr("repose.target.parse_repositories", lambda x: parsed)

    target.parse_repos()
    assert target.repos is not None
    assert "a" in target.repos


def test_report_helpers_invoke_sink(make_target):
    target, _ = make_target()
    target.products = "PRODS"
    target.raw_repos = ["R1"]

    sink = MagicMock()
    target.report_products(sink)
    sink.assert_called_with("h", 22, "PRODS")

    sink2 = MagicMock()
    target.report_products_yaml(sink2)
    sink2.assert_called_with("h", "PRODS")

    sink3 = MagicMock()
    target.report_repos(sink3)
    sink3.assert_called_with("h", 22, ["R1"])
