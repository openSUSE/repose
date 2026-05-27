import concurrent.futures
from argparse import Namespace
from unittest.mock import MagicMock, call

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.uninstall import Uninstall
from repose.types.repa import Repa


class MockRepo:
    def __init__(self, name):
        self.name = name


class MockProduct:
    def __init__(self, name, version):
        self.name = name
        self.version = version


def _ok_out():
    return [["cmd", "", "", 0, 0]]


def _fail_out():
    return [["cmd", "", "boom", 1, 0]]


@pytest.fixture
def mock_args_and_repa():
    repa_instance = Repa("SLES:15-SP4")
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[repa_instance],
        config="dummy_config",
        yaml=False,
    )
    return args, repa_instance


def test_uninstall_command_run(monkeypatch, mock_args_and_repa, mock_ssh_client):
    mock_args, repa_instance = mock_args_and_repa

    # Mocks
    mock_target = MagicMock()
    mock_target.products.flatten.return_value = [MockProduct("SLES", "15-SP4")]
    mock_target.repos = {
        "SLES:15-SP4::repo1": MockRepo(name="SLES"),
        "SLES:15-SP4::repo2": MockRepo(name="SLES"),
        "other:repo": MockRepo(name="other"),
    }
    mock_target.out = _ok_out()

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
    uninstall_command = Uninstall(mock_args)
    assert uninstall_command.run() == 0

    # Assertions
    mock_host_group_instance.read_repos.assert_called_once()
    mock_host_group_instance.parse_repos.assert_called_once()

    # Check that repa.repo was set to None
    assert repa_instance.repo is None

    rm_cmd = uninstall_command.rrpcmd.format(products="SLES")

    run_calls = mock_target.run.call_args_list
    # The order of the repository names in the rr command is not guaranteed.
    first_call_args, _ = run_calls[0]
    first_cmd = first_call_args[0]
    assert first_cmd.startswith("zypper -n rr")
    assert "SLES:15-SP4::repo1" in first_cmd
    assert "SLES:15-SP4::repo2" in first_cmd

    # The second call should be the rm command
    assert run_calls[1] == call(rm_cmd)

    mock_host_group_instance.close.assert_called_once()


def _setup_uninstall(monkeypatch, args, products, repos, out=None, hosts=None):
    if hosts is None:
        hosts = ["user@host1"]
    mock_target = MagicMock()
    mock_target.products.flatten.return_value = products
    mock_target.repos = repos
    mock_target.out = out if out is not None else _ok_out()
    mock_hg = MagicMock()
    mock_hg.keys.return_value = hosts
    mock_hg.__getitem__.return_value = mock_target
    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_hg),
    )
    return mock_target, mock_hg


def test_uninstall_dryrun_does_not_run(monkeypatch, capsys, mock_ssh_client):
    args = Namespace(
        dry=True,
        target=[{"user@host1": MagicMock()}],
        repa=[Repa("SLES:15-SP4")],
        config="dummy",
        yaml=False,
    )
    target, _ = _setup_uninstall(
        monkeypatch,
        args,
        [MockProduct("SLES", "15-SP4")],
        {"SLES:15-SP4::repo1": MockRepo("SLES")},
    )

    assert Uninstall(args).run() == 0

    target.run.assert_not_called()
    out = capsys.readouterr().out
    assert "user@host1" in out


def test_uninstall_no_patterns_logs(monkeypatch, caplog, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[Repa("OTHER:99")],
        config="dummy",
        yaml=False,
    )
    target, _ = _setup_uninstall(
        monkeypatch,
        args,
        [MockProduct("SLES", "15-SP4")],
        {"SLES:15-SP4::repo1": MockRepo("SLES")},
    )

    with caplog.at_level("INFO", logger="repose.command.uninstall"):
        # No matching pattern → INFO no-op → exit 0.
        assert Uninstall(args).run() == 0

    target.run.assert_not_called()
    assert any("no products for remove" in r.message for r in caplog.records)


def test_uninstall_no_matching_repos_runs_only_pdcmd(monkeypatch, mock_ssh_client):
    """Patterns match but no repos in dict → rrcmd skipped, only pdcmd runs."""
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[Repa("SLES:15-SP4")],
        config="dummy",
        yaml=False,
    )
    target, _ = _setup_uninstall(
        monkeypatch,
        args,
        [MockProduct("SLES", "15-SP4")],
        {},  # no repositories at all
    )

    assert Uninstall(args).run() == 0

    # Only one command issued: rrpcmd (no rrcmd because no rdict)
    assert target.run.call_count == 1
    assert "rm -t product" in target.run.call_args[0][0]


def test_uninstall_sl_micro_uses_transactional(monkeypatch, caplog, mock_ssh_client):
    """SL-Micro uninstalls must dispatch via ``transactional-update``.

    Patterns are formatted ``<product>:<version>::<repo>`` so SL-Micro
    detection has to match on the product component, not the whole pattern.
    """
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[Repa("SL-Micro:6.0")],
        config="dummy",
        yaml=False,
    )
    target, _ = _setup_uninstall(
        monkeypatch,
        args,
        [MockProduct("SL-Micro", "6.0")],
        {"SL-Micro:6.0::repo1": MockRepo("SL-Micro")},
    )

    with caplog.at_level("INFO", logger="repose.command.uninstall"):
        assert Uninstall(args).run() == 0

    issued = [c.args[0] for c in target.run.call_args_list]
    # Both rr (for the matched repo) and the transactional rm should fire.
    assert any(cmd.startswith("zypper -n rr") for cmd in issued)
    assert any(
        "transactional-update pkg rm -t product" in cmd and "SL-Micro" in cmd
        for cmd in issued
    )
    # And the reboot reminder must be emitted.
    assert any("Reboot" in r.message for r in caplog.records)


# ---------------------------------------------------------------------------
# Exit code propagation (PR 06)
# ---------------------------------------------------------------------------


def _setup_uninstall_multi(monkeypatch, hosts):
    targets = {}
    for host, out in hosts.items():
        t = MagicMock()
        t.products.flatten.return_value = [MockProduct("SLES", "15-SP4")]
        t.repos = {"SLES:15-SP4::repo1": MockRepo("SLES")}
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
    return targets, hg


def test_uninstall_run_returns_0_when_all_succeed(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        repa=[Repa("SLES:15-SP4")],
        config="dummy",
        yaml=False,
    )
    _setup_uninstall_multi(monkeypatch, {"h1": _ok_out(), "h2": _ok_out()})

    assert Uninstall(args).run() == 0


def test_uninstall_run_returns_1_on_partial_failure(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        repa=[Repa("SLES:15-SP4")],
        config="dummy",
        yaml=False,
    )
    _setup_uninstall_multi(monkeypatch, {"h1": _ok_out(), "h2": _fail_out()})

    assert Uninstall(args).run() == 1


def test_uninstall_run_returns_2_when_all_fail(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        repa=[Repa("SLES:15-SP4")],
        config="dummy",
        yaml=False,
    )
    _setup_uninstall_multi(monkeypatch, {"h1": _fail_out(), "h2": _fail_out()})

    assert Uninstall(args).run() == 2
