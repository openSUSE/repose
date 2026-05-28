import concurrent.futures
from argparse import Namespace
from unittest.mock import MagicMock

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.reset import Reset
from repose.messages import UnsuportedProductMessage


class MockRepo:
    def __init__(self, name, url, refresh=False):
        self.name = name
        self.url = url
        self.refresh = refresh


class MockRawRepo:
    def __init__(self, alias):
        self.alias = alias


class MockProduct:
    def __init__(self, name, version):
        self.name = name
        self.version = version


def _ok_out():
    return [["cmd", "", "", 0, 0]]


def _fail_out():
    return [["cmd", "", "boom", 1, 0]]


@pytest.fixture
def mock_args():
    """Fixture for command arguments."""
    return Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        config="dummy_config",
        repa=None,
        yaml=False,
    )


def test_reset_command_run(monkeypatch, mock_args, mock_ssh_client):
    # Setup Mocks
    mock_repoq = MagicMock()
    mock_repoq.solve_product.return_value = {
        "product": [
            MockRepo("prod-repo1", "http://prod-repo1.url", refresh=True),
        ]
    }

    mock_target = MagicMock()
    mock_target.products = [MockProduct("SLES", "15-SP4")]
    mock_target.raw_repos = [
        MockRawRepo("existing-repo1"),
        MockRawRepo("existing-repo2"),
    ]
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
    monkeypatch.setattr(Reset, "repoq", mock_repoq)
    monkeypatch.setattr(Reset, "check_url", MagicMock(return_value=True))

    # Instantiate and Run
    reset_command = Reset(mock_args)
    assert reset_command.run() == 0

    # Assertions
    mock_host_group_instance.read_products.assert_called_once()
    mock_host_group_instance.read_repos.assert_called_once()

    run_calls = mock_target.run.call_args_list
    assert len(run_calls) == 2

    rr_call_args, _ = run_calls[0]
    rr_cmd = rr_call_args[0]
    assert rr_cmd.startswith("zypper -n rr")
    assert "existing-repo1" in rr_cmd
    assert "existing-repo2" in rr_cmd

    ar_call_args, _ = run_calls[1]
    expected_ar_cmd = reset_command.addcmd.format(
        name="prod-repo1", url="http://prod-repo1.url", params="-cfkn"
    )
    assert ar_call_args[0] == expected_ar_cmd

    mock_host_group_instance.close.assert_called_once()


def _setup_reset(
    monkeypatch,
    args,
    repoq_solution=None,
    raw_repos=None,
    products=None,
    check_url=True,
    solve_side_effect=None,
    out=None,
):
    mock_repoq = MagicMock()
    if solve_side_effect is not None:
        mock_repoq.solve_product.side_effect = solve_side_effect
    else:
        mock_repoq.solve_product.return_value = repoq_solution

    mock_target = MagicMock()
    mock_target.products = products or [MockProduct("SLES", "15-SP4")]
    mock_target.raw_repos = raw_repos or [MockRawRepo("existing-repo1")]
    mock_target.out = out if out is not None else _ok_out()

    mock_hg = MagicMock()
    mock_hg.keys.return_value = ["user@host1"]
    mock_hg.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_hg),
    )
    monkeypatch.setattr(Reset, "repoq", mock_repoq)
    monkeypatch.setattr(Reset, "check_url", MagicMock(return_value=check_url))
    return mock_target, mock_hg, mock_repoq


def test_reset_dryrun_does_not_run(monkeypatch, mock_args, capsys, mock_ssh_client):
    mock_args.dry = True
    target, _, _ = _setup_reset(
        monkeypatch,
        mock_args,
        repoq_solution={"product": [MockRepo("repo1", "http://r1", refresh=True)]},
    )

    assert Reset(mock_args).run() == 0

    target.run.assert_not_called()
    out = capsys.readouterr().out
    assert "user@host1" in out
    assert "existing-repo1" in out


def test_reset_unsupported_product_logs_error(
    monkeypatch, mock_args, caplog, mock_ssh_client
):
    UnknownProd = type("P", (), {"name": "X", "version": "1"})
    target, _, _ = _setup_reset(
        monkeypatch,
        mock_args,
        solve_side_effect=UnsuportedProductMessage(UnknownProd()),
    )

    with caplog.at_level("ERROR", logger="repose.command.reset"):
        # UnsuportedProductMessage on the only host → exit 2.
        assert Reset(mock_args).run() == 2

    assert any("Refhost" in r.message for r in caplog.records)
    # _add() raised before reaching the run() block — no commands executed.
    target.run.assert_not_called()


def test_reset_check_url_false_skips_add(monkeypatch, mock_args, mock_ssh_client):
    target, _, _ = _setup_reset(
        monkeypatch,
        mock_args,
        repoq_solution={"product": [MockRepo("r1", "http://bad", refresh=False)]},
        check_url=False,
    )

    # rr fires (and succeeds via _ok_out), no ar attempted → exit 0.
    assert Reset(mock_args).run() == 0

    issued = [c.args[0] for c in target.run.call_args_list]
    # rr executed but no ar (filtered out by check_url=False)
    assert any(c.startswith("zypper -n rr") for c in issued)
    assert not any(c.startswith("zypper -n ar") for c in issued)


# ---------------------------------------------------------------------------
# Exit code propagation (PR 06)
# ---------------------------------------------------------------------------


def _setup_reset_multi(monkeypatch, hosts):
    mock_repoq = MagicMock()
    mock_repoq.solve_product.return_value = {
        "product": [MockRepo("repo1", "http://repo1.url", refresh=False)]
    }

    targets = {}
    for host, out in hosts.items():
        t = MagicMock()
        t.products = [MockProduct("SLES", "15-SP4")]
        t.raw_repos = [MockRawRepo(f"{host}-existing")]
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
    monkeypatch.setattr(Reset, "repoq", mock_repoq)
    monkeypatch.setattr(Reset, "check_url", MagicMock(return_value=True))
    return targets, hg


def test_reset_run_returns_0_when_all_succeed(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        config="dummy_config",
        repa=None,
        yaml=False,
    )
    _setup_reset_multi(monkeypatch, {"h1": _ok_out(), "h2": _ok_out()})

    assert Reset(args).run() == 0


def test_reset_run_returns_1_on_partial_failure(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        config="dummy_config",
        repa=None,
        yaml=False,
    )
    _setup_reset_multi(monkeypatch, {"h1": _ok_out(), "h2": _fail_out()})

    assert Reset(args).run() == 1


def test_reset_run_returns_2_when_all_fail(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        config="dummy_config",
        repa=None,
        yaml=False,
    )
    _setup_reset_multi(monkeypatch, {"h1": _fail_out(), "h2": _fail_out()})

    assert Reset(args).run() == 2
