"""Tests for ``repose.target.hostgroup.HostGroup``."""

from unittest.mock import MagicMock

import pytest
from conftest import ImmediateExecutor

from repose.target.hostgroup import HostGroup


@pytest.fixture
def two_targets():
    a = MagicMock()
    b = MagicMock()
    return {"host-a": a, "host-b": b}


def test_connect_calls_each_target_connect(two_targets):
    for t in two_targets.values():
        t.connect.return_value = t  # mimic Target.connect returning self

    hg = HostGroup(two_targets)
    hg.connect()

    for t in two_targets.values():
        t.connect.assert_called_once()


def test_connect_swallows_per_host_exception(two_targets, caplog):
    """If one target's connect() raises, others still finish."""
    two_targets["host-a"].connect.side_effect = RuntimeError("boom")
    two_targets["host-b"].connect.return_value = two_targets["host-b"]

    hg = HostGroup(two_targets)
    with caplog.at_level("WARNING", logger="repose.target.hostgroup"):
        hg.connect()  # should not raise

    messages = [r.getMessage() for r in caplog.records]
    assert any("boom" in m and "host-a" in m for m in messages)
    two_targets["host-b"].connect.assert_called_once()


def test_close_calls_each_target_close(two_targets):
    hg = HostGroup(two_targets)
    hg.close()
    for t in two_targets.values():
        t.close.assert_called_once()


def test_read_products_fans_out(two_targets):
    hg = HostGroup(two_targets)
    hg.read_products()
    for t in two_targets.values():
        t.read_products.assert_called_once()


def test_read_repos_fans_out(two_targets):
    hg = HostGroup(two_targets)
    hg.read_repos()
    for t in two_targets.values():
        t.read_repos.assert_called_once()


def test_parse_repos_fans_out(two_targets):
    hg = HostGroup(two_targets)
    hg.parse_repos()
    for t in two_targets.values():
        t.parse_repos.assert_called_once()


def test_report_products_iterates_sorted(two_targets):
    hg = HostGroup(two_targets)
    sink = MagicMock()
    hg.report_products(sink)

    for t in two_targets.values():
        t.report_products.assert_called_once_with(sink)


def test_report_products_yaml_iterates_sorted(two_targets):
    hg = HostGroup(two_targets)
    sink = MagicMock()
    hg.report_products_yaml(sink)
    for t in two_targets.values():
        t.report_products_yaml.assert_called_once_with(sink)


def test_report_repos_iterates_sorted(two_targets):
    hg = HostGroup(two_targets)
    sink = MagicMock()
    hg.report_repos(sink)
    for t in two_targets.values():
        t.report_repos.assert_called_once_with(sink)


def test_run_fans_out_with_str_command(monkeypatch, two_targets):
    """A bare ``str`` command broadcasts the same string to every host."""
    monkeypatch.setattr(
        "repose.target.hostgroup.concurrent.futures.ThreadPoolExecutor",
        ImmediateExecutor,
    )

    hg = HostGroup(two_targets)
    hg.run("zypper -n ref")

    for t in two_targets.values():
        t.run.assert_called_once_with("zypper -n ref")


def test_run_fans_out_with_dict_command(monkeypatch, two_targets):
    """A ``dict`` command dispatches per-host commands."""
    monkeypatch.setattr(
        "repose.target.hostgroup.concurrent.futures.ThreadPoolExecutor",
        ImmediateExecutor,
    )

    hg = HostGroup(two_targets)
    hg.run({"host-a": "cmd-a", "host-b": "cmd-b"})

    two_targets["host-a"].run.assert_called_once_with("cmd-a")
    two_targets["host-b"].run.assert_called_once_with("cmd-b")
