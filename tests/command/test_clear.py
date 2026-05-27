import concurrent.futures
from argparse import Namespace
from unittest.mock import MagicMock

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.clear import Clear


class MockRawRepo:
    def __init__(self, alias):
        self.alias = alias


@pytest.fixture
def mock_args():
    return Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=None,
        config="dummy_config",
        yaml=False,
    )


def test_clear_command_run(monkeypatch, mock_args, mock_ssh_client):
    # Mocks
    mock_target = MagicMock()
    mock_target.raw_repos = [MockRawRepo("repo1"), MockRawRepo("repo2")]

    mock_host_group_instance = MagicMock()
    mock_host_group_instance.keys.return_value = ["user@host1"]
    mock_host_group_instance.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_host_group_instance),
    )

    # Run
    clear_command = Clear(mock_args)
    assert clear_command.run() == 0

    # Assertions
    mock_host_group_instance.read_repos.assert_called_once()

    run_call = mock_target.run.call_args[0][0]
    assert run_call.startswith("zypper -n rr")
    assert "repo1" in run_call
    assert "repo2" in run_call

    mock_host_group_instance.close.assert_called_once()


def test_clear_command_dryrun_skips_run(monkeypatch, capsys, mock_ssh_client):
    args = Namespace(
        dry=True,
        target=[{"user@host1": MagicMock()}],
        repa=None,
        config="dummy_config",
        yaml=False,
    )
    mock_target = MagicMock()
    mock_target.raw_repos = [MockRawRepo("repo1")]

    mock_hg = MagicMock()
    mock_hg.keys.return_value = ["user@host1"]
    mock_hg.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_hg),
    )

    assert Clear(args).run() == 0

    mock_target.run.assert_not_called()
    out = capsys.readouterr().out
    assert "user@host1" in out
    assert "repo1" in out


def test_clear_command_empty_repos(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=None,
        config="dummy_config",
        yaml=False,
    )
    mock_target = MagicMock()
    mock_target.raw_repos = []  # nothing to clear

    mock_hg = MagicMock()
    mock_hg.keys.return_value = ["user@host1"]
    mock_hg.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_hg),
    )

    assert Clear(args).run() == 0

    # rr command still issued, just with empty repos list
    cmd = mock_target.run.call_args[0][0]
    assert cmd.startswith("zypper -n rr")


# ---------------------------------------------------------------------------
# Exit code propagation (PR 06)
# ---------------------------------------------------------------------------


def _setup_clear_hosts(monkeypatch, hosts):
    """Build a HostGroup mock with one target per host.

    ``hosts`` is a dict mapping ``host -> target.run side_effect``
    (either ``None`` for success or an exception to raise).
    """
    targets = {}
    for host, run_se in hosts.items():
        t = MagicMock()
        t.raw_repos = [MockRawRepo(f"{host}-repo")]
        if run_se is not None:
            t.run.side_effect = run_se
        targets[host] = t

    hg = MagicMock()
    hg.keys.return_value = list(hosts.keys())
    hg.__getitem__.side_effect = lambda k: targets[k]

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=hg),
    )
    return targets, hg


def test_clear_run_returns_0_when_all_succeed(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        repa=None,
        config="dummy_config",
        yaml=False,
    )
    _setup_clear_hosts(monkeypatch, {"h1": None, "h2": None})

    assert Clear(args).run() == 0


def test_clear_run_returns_1_on_partial_failure(monkeypatch, mock_ssh_client):
    """If ``target.run`` raises on one host, that host's _run future
    holds an exception → partial failure."""
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        repa=None,
        config="dummy_config",
        yaml=False,
    )
    _setup_clear_hosts(monkeypatch, {"h1": None, "h2": RuntimeError("boom")})

    assert Clear(args).run() == 1


def test_clear_run_returns_2_when_all_fail(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        repa=None,
        config="dummy_config",
        yaml=False,
    )
    _setup_clear_hosts(
        monkeypatch,
        {"h1": RuntimeError("boom1"), "h2": RuntimeError("boom2")},
    )

    assert Clear(args).run() == 2
