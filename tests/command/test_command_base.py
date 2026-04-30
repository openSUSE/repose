"""Tests for ``repose.command._command.Command`` base class."""

from unittest.mock import MagicMock
from urllib.error import HTTPError, URLError

import pytest

import repose.command._command as cmdmod
from repose.command._command import Command


class _Concrete(Command):
    command = True

    def run(self):
        return 0


def _make(monkeypatch, args_overrides=None, host_group=None):
    """Build a concrete Command instance with mocked HostGroup."""
    if host_group is None:
        host_group = MagicMock()
        host_group.keys.return_value = []
    monkeypatch.setattr(cmdmod, "HostGroup", MagicMock(return_value=host_group))

    from argparse import Namespace

    defaults = {
        "dry": False,
        "target": [{"h1": MagicMock()}],
        "config": "p",
        "yaml": False,
        "repa": [],
    }
    if args_overrides:
        defaults.update(args_overrides)
    return _Concrete(Namespace(**defaults)), host_group


# ---------------------------------------------------------------------------
# check_url
# ---------------------------------------------------------------------------


def test_check_url_returns_true_when_url_opens(monkeypatch):
    monkeypatch.setattr(cmdmod, "urlopen", lambda url: MagicMock())
    assert Command.check_url("http://example.com/") is True


def test_check_url_returns_false_when_both_urls_fail(monkeypatch):
    def _raise(url):
        raise URLError("nope")

    monkeypatch.setattr(cmdmod, "urlopen", _raise)
    assert Command.check_url("http://example.com/") is False


def test_check_url_handles_http_error(monkeypatch):
    def _raise(url):
        raise HTTPError(url, 404, "not found", {}, None)

    monkeypatch.setattr(cmdmod, "urlopen", _raise)
    assert Command.check_url("http://example.com/") is False


# ---------------------------------------------------------------------------
# Filtering of unconnected targets
# ---------------------------------------------------------------------------


def test_unconnected_targets_are_dropped(monkeypatch):
    # HostGroup behaves like a dict with two hosts; one is "connected"
    # (truthy mock) and the other is falsy (not connected).
    connected = MagicMock()
    connected.__bool__ = lambda self: True
    disconnected = MagicMock()
    disconnected.__bool__ = lambda self: False

    hg = MagicMock()
    hg.keys.return_value = ["a", "b"]
    hg.__getitem__.side_effect = lambda k: {
        "a": connected,
        "b": disconnected,
    }[k]
    deletions = []
    hg.__delitem__.side_effect = lambda k: deletions.append(k)

    cmd, _ = _make(monkeypatch, host_group=hg)
    assert "b" in deletions
    assert "a" not in deletions


# ---------------------------------------------------------------------------
# _report_target
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "exitcode,expected_stream",
    [
        (0, "stdout"),  # logger.info on stdout
        (4, "stdout"),  # logger.warning on stdout
        (1, "stderr"),  # logger.warning on stderr
        (-1, "stderr"),
    ],
)
def test_report_target_routes_by_exitcode(monkeypatch, exitcode, expected_stream):
    """_report_target picks the right output stream depending on exitcode."""
    target = MagicMock()
    # out is list of [cmd, stdout, stderr, exitcode, runtime]
    target.out = [["cmd", "out-line", "err-line", exitcode, 0]]

    hg = MagicMock()
    hg.keys.return_value = ["host"]
    hg.__getitem__.return_value = target

    cmd, _ = _make(monkeypatch, host_group=hg)
    # Should not raise
    cmd._report_target("host")


def test_init_repoq_loads_template(monkeypatch, tmp_path):
    yml = tmp_path / "products.yml"
    yml.write_text("SLES:\n  '15':\n    default_repos:\n      - update\n")

    cmd, _ = _make(
        monkeypatch,
        args_overrides={"config": yml},
    )
    repoq = cmd._init_repoq()
    assert "SLES" in repoq.template
