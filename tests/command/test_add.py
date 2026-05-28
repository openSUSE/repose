import concurrent.futures
from argparse import Namespace
from unittest.mock import MagicMock, call

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.add import Add
from repose.types.repa import Repa


class MockRepo:
    def __init__(self, name, url, refresh=False):
        self.name = name
        self.url = url
        self.refresh = refresh


def _ok_out():
    """Return a target.out list with a single zero-exitcode entry,
    so ``_report_target`` returns True."""
    return [["cmd", "", "", 0, 0]]


def _fail_out():
    """Return a target.out list with a non-zero exitcode so
    ``_report_target`` returns False."""
    return [["cmd", "", "boom", 1, 0]]


@pytest.fixture
def mock_args_and_repa():
    """Fixture for command arguments."""
    repa_instance = Repa("dummy-repa")
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[repa_instance],
        config="dummy_config",
        yaml=False,
    )
    return args, repa_instance


def test_add_command_run(monkeypatch, mock_args_and_repa, mock_ssh_client):
    mock_args, repa_instance = mock_args_and_repa

    # Setup Mocks
    mock_repoq = MagicMock()
    mock_repoq.solve_repa.return_value = {
        "product": [
            MockRepo("repo1", "http://repo1.url", refresh=True),
            MockRepo("repo2", "http://repo2.url"),
        ]
    }

    mock_target = MagicMock()
    mock_target.out = _ok_out()
    mock_host_group_instance = MagicMock()
    mock_host_group_instance.keys.return_value = ["user@host1"]
    mock_host_group_instance.__getitem__.return_value = mock_target
    mock_target.products.get_base.return_value = "dummy_base"

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_host_group_instance),
    )
    monkeypatch.setattr(Add, "repoq", mock_repoq)
    monkeypatch.setattr(Add, "check_url", MagicMock(return_value=True))

    # Instantiate and Run
    add_command = Add(mock_args)
    assert add_command.run() == 0

    # Assertions
    mock_host_group_instance.read_products.assert_called_once()
    mock_repoq.solve_repa.assert_called_once_with(repa_instance, "dummy_base")

    expected_cmd1 = add_command.addcmd.format(
        name="repo1", url="http://repo1.url", params="-cfkn"
    )
    expected_cmd2 = add_command.addcmd.format(
        name="repo2", url="http://repo2.url", params="-ckn"
    )

    mock_target.run.assert_has_calls(
        [
            call(expected_cmd1),
            call(expected_cmd2),
        ],
        any_order=True,
    )

    mock_host_group_instance.run.assert_called_once_with(add_command.refcmd)
    mock_host_group_instance.close.assert_called_once()


def test_add_command_dryrun_prints_and_skips_run(
    monkeypatch, mock_args_and_repa, capsys, mock_ssh_client
):
    args, _ = mock_args_and_repa
    args.dry = True

    mock_repoq = MagicMock()
    mock_repoq.solve_repa.return_value = {
        "product": [MockRepo("repo1", "http://repo1.url", refresh=True)]
    }
    mock_target = MagicMock()
    mock_target.products.get_base.return_value = "dummy_base"

    mock_hg = MagicMock()
    mock_hg.keys.return_value = ["user@host1"]
    mock_hg.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_hg),
    )
    monkeypatch.setattr(Add, "repoq", mock_repoq)
    monkeypatch.setattr(Add, "check_url", MagicMock(return_value=True))

    assert Add(args).run() == 0

    mock_target.run.assert_not_called()
    # refcmd skipped on dryrun
    mock_hg.run.assert_not_called()

    out = capsys.readouterr().out
    assert "repo1" in out
    assert "user@host1" in out


def test_add_command_solve_repa_value_error_logged(
    monkeypatch, mock_args_and_repa, caplog, mock_ssh_client
):
    args, _ = mock_args_and_repa

    mock_repoq = MagicMock()
    mock_repoq.solve_repa.side_effect = ValueError("Not known product: X")

    mock_target = MagicMock()
    mock_target.products.get_base.return_value = "dummy_base"

    mock_hg = MagicMock()
    mock_hg.keys.return_value = ["user@host1"]
    mock_hg.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_hg),
    )
    monkeypatch.setattr(Add, "repoq", mock_repoq)
    monkeypatch.setattr(Add, "check_url", MagicMock(return_value=True))

    with caplog.at_level("ERROR", logger="repose.command.add"):
        # solve_repa failure on the only host → all hosts failed → exit 2.
        assert Add(args).run() == 2

    assert any("Not known product" in r.message for r in caplog.records)
    mock_target.run.assert_not_called()


def test_add_command_skips_repo_when_check_url_false(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    args, _ = mock_args_and_repa

    mock_repoq = MagicMock()
    mock_repoq.solve_repa.return_value = {
        "product": [MockRepo("repo1", "http://bad.url", refresh=False)]
    }
    mock_target = MagicMock()
    mock_target.products.get_base.return_value = "dummy_base"
    mock_hg = MagicMock()
    mock_hg.keys.return_value = ["user@host1"]
    mock_hg.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_hg),
    )
    monkeypatch.setattr(Add, "repoq", mock_repoq)
    monkeypatch.setattr(Add, "check_url", MagicMock(return_value=False))

    # Repo filtered out by check_url → no cmds attempted, no failures → 0.
    assert Add(args).run() == 0

    # Repo got filtered out because URL check failed → no per-target add cmd
    mock_target.run.assert_not_called()


# ---------------------------------------------------------------------------
# Exit code propagation (PR 06)
# ---------------------------------------------------------------------------


def _setup_add_hosts(monkeypatch, hosts):
    """Build a HostGroup-like mock with one target per host and stub
    out the URL probe + repoq resolution to a single repo.

    ``hosts`` is a dict mapping ``host -> target.out`` so callers can
    decide which hosts succeed and which fail.
    """
    targets = {}
    for host, out in hosts.items():
        t = MagicMock()
        t.products.get_base.return_value = "dummy_base"
        t.out = out
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

    mock_repoq = MagicMock()
    mock_repoq.solve_repa.return_value = {
        "product": [MockRepo("repo1", "http://repo1.url", refresh=False)]
    }
    monkeypatch.setattr(Add, "repoq", mock_repoq)
    monkeypatch.setattr(Add, "check_url", MagicMock(return_value=True))
    return targets, hg


def test_add_run_returns_0_when_all_succeed(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    args, _ = mock_args_and_repa
    args.target = [{"h1": MagicMock(), "h2": MagicMock()}]
    _setup_add_hosts(monkeypatch, {"h1": _ok_out(), "h2": _ok_out()})

    assert Add(args).run() == 0


def test_add_run_returns_1_on_partial_failure(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    args, _ = mock_args_and_repa
    args.target = [{"h1": MagicMock(), "h2": MagicMock()}]
    _setup_add_hosts(monkeypatch, {"h1": _ok_out(), "h2": _fail_out()})

    assert Add(args).run() == 1


def test_add_run_returns_2_when_all_fail(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    args, _ = mock_args_and_repa
    args.target = [{"h1": MagicMock(), "h2": MagicMock()}]
    _setup_add_hosts(monkeypatch, {"h1": _fail_out(), "h2": _fail_out()})

    assert Add(args).run() == 2
