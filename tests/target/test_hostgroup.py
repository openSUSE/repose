"""Tests for ``repose.target.hostgroup.HostGroup``."""

from unittest.mock import MagicMock

import pytest

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


def test_connect_swallows_per_host_exception(two_targets, capsys):
    """If one target's connect() raises, others still finish."""
    two_targets["host-a"].connect.side_effect = RuntimeError("boom")
    two_targets["host-b"].connect.return_value = two_targets["host-b"]

    hg = HostGroup(two_targets)
    hg.connect()  # should not raise

    captured = capsys.readouterr()
    assert "boom" in captured.out
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


def test_run_delegates_to_runcommand(monkeypatch, two_targets):
    captured = {}

    class FakeRC:
        def __init__(self, data, cmd):
            captured["data"] = data
            captured["cmd"] = cmd

        def run(self):
            return "ok"

    monkeypatch.setattr("repose.target.hostgroup.RunCommand", FakeRC)
    hg = HostGroup(two_targets)
    assert hg.run("zypper -n ref") == "ok"
    assert captured["cmd"] == "zypper -n ref"
    assert set(captured["data"].keys()) == {"host-a", "host-b"}
