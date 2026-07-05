import concurrent.futures
from argparse import Namespace
from unittest.mock import AsyncMock, MagicMock

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


def test_clear_command_dryrun_json_format(monkeypatch, capsys, mock_ssh_client):
    """End-to-end: --format=json emits a parseable JSON event per dry-run line."""
    import json

    args = Namespace(
        dry=True,
        target=[{"user@host1": MagicMock()}],
        repa=None,
        config="dummy_config",
        yaml=False,
        format="json",
        no_color=False,
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

    lines = [line for line in capsys.readouterr().out.splitlines() if line.strip()]
    assert lines, "expected at least one JSON event line"
    payload = json.loads(lines[0])
    assert payload["event"] == "dry"
    assert payload["host"] == "user@host1"
    assert "repo1" in payload["cmd"]


def test_clear_command_empty_repos_skips_rr(monkeypatch, caplog, mock_ssh_client):
    """A host without repositories is an INFO-level no-op.

    Issuing the command anyway would run a bare ``zypper -n rr`` (no
    argument), which zypper rejects with a non-zero exit — so no command
    must be run and the host must still be reported ok.
    """
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

    with caplog.at_level("INFO", logger="repose.command.clear"):
        assert Clear(args).run() == 0

    mock_target.run.assert_not_called()
    assert any("No repositories to clear" in r.message for r in caplog.records)


def test_clear_command_mixed_hosts_skips_only_empty(monkeypatch, mock_ssh_client):
    """Only the repo-less host is skipped; the other still gets ``rr``."""
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        repa=None,
        config="dummy_config",
        yaml=False,
    )
    t1 = MagicMock()
    t1.raw_repos = [MockRawRepo("h1-repo")]
    t2 = MagicMock()
    t2.raw_repos = []
    targets = {"h1": t1, "h2": t2}

    mock_hg = MagicMock()
    mock_hg.keys.return_value = ["h1", "h2"]
    mock_hg.__getitem__.side_effect = lambda k: targets[k]

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_hg),
    )

    assert Clear(args).run() == 0

    assert t1.run.call_args[0][0] == "zypper -n rr h1-repo"
    t2.run.assert_not_called()


async def test_clear_arun_one_empty_repos_skips_rr(mock_args, caplog):
    """Async ``_arun_one`` must mirror the sync no-repos no-op guard."""
    mock_args.ssh_backend = "asyncssh"
    clear = Clear(mock_args)

    target = MagicMock()
    target.raw_repos = []
    target.run = AsyncMock()

    clear.targets = {"user@host1": target}

    with caplog.at_level("INFO", logger="repose.command.clear"):
        ok = await clear._arun_one("user@host1", lambda host, msg: None)

    assert ok is True
    target.run.assert_not_awaited()
    assert any("No repositories to clear" in r.message for r in caplog.records)


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
