import concurrent.futures
from argparse import Namespace
from unittest.mock import MagicMock, call

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.install import Install
from repose.types.repa import Repa


class MockRepo:
    def __init__(self, name, url, refresh=False):
        self.name = name
        self.url = url
        self.refresh = refresh


@pytest.fixture
def mock_args_and_repa():
    repa_instance = Repa("dummy-repa")
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[repa_instance],
        config="dummy_config",
        yaml=False,
    )
    return args, repa_instance


def test_install_command_run(monkeypatch, mock_args_and_repa, mock_ssh_client):
    mock_args, repa_instance = mock_args_and_repa

    # Mocks
    mock_repoq = MagicMock()
    mock_repoq.solve_repa.return_value = {
        "product-to-install": [MockRepo("repo1", "http://repo1.url", refresh=True)]
    }

    mock_target = MagicMock()
    mock_target.products.get_base.return_value = "dummy_base"
    mock_host_group_instance = MagicMock()
    mock_host_group_instance.keys.return_value = ["user@host1"]
    mock_host_group_instance.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_host_group_instance),
    )
    monkeypatch.setattr(Install, "_init_repoq", MagicMock(return_value=mock_repoq))

    # Run
    install_command = Install(mock_args)
    install_command.run()

    # Assertions
    mock_host_group_instance.read_products.assert_called_once()
    mock_host_group_instance.read_repos.assert_called_once()
    mock_repoq.solve_repa.assert_called_once_with(repa_instance, "dummy_base")

    expected_ar_cmd = install_command.addcmd.format(
        name="repo1", url="http://repo1.url", params="-cfkn"
    )
    expected_in_cmd = install_command.ipdcmd.format(products="product-to-install")

    mock_target.run.assert_has_calls(
        [
            call(expected_ar_cmd),
            call(install_command.refcmd),
            call(expected_in_cmd),
        ],
        any_order=False,
    )

    mock_host_group_instance.close.assert_called_once()


def _setup_install(monkeypatch, args, repoq_solution):
    mock_repoq = MagicMock()
    mock_repoq.solve_repa.return_value = repoq_solution

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
    monkeypatch.setattr(Install, "_init_repoq", MagicMock(return_value=mock_repoq))
    return mock_target, mock_hg, mock_repoq


def test_install_command_dryrun_skips_run(
    monkeypatch, mock_args_and_repa, capsys, mock_ssh_client
):
    args, _ = mock_args_and_repa
    args.dry = True

    target, _, _ = _setup_install(
        monkeypatch,
        args,
        {"product": [MockRepo("repo1", "http://repo1.url", refresh=True)]},
    )

    Install(args).run()

    target.run.assert_not_called()
    out = capsys.readouterr().out
    assert "repo1" in out
    assert "user@host1" in out


def test_install_command_sl_micro_uses_transactional(
    monkeypatch, mock_args_and_repa, caplog, mock_ssh_client
):
    args, _ = mock_args_and_repa

    target, _, _ = _setup_install(
        monkeypatch,
        args,
        {"SL-Micro": [MockRepo("repo1", "http://repo1.url", refresh=True)]},
    )

    with caplog.at_level("INFO", logger="repose.command.install"):
        Install(args).run()

    # Find the install command issued
    issued = [c.args[0] for c in target.run.call_args_list]
    assert any("transactional-update" in cmd for cmd in issued)
    assert any("Reboot" in r.message for r in caplog.records)


def test_install_command_no_products_logs_error(
    monkeypatch, mock_args_and_repa, caplog, mock_ssh_client
):
    args, _ = mock_args_and_repa

    target, _, repoq = _setup_install(
        monkeypatch,
        args,
        {},  # empty solution
    )

    with caplog.at_level("ERROR", logger="repose.command.install"):
        Install(args).run()

    assert any("No products to install" in r.message for r in caplog.records)


def test_install_command_solve_repa_value_error_logged(
    monkeypatch, mock_args_and_repa, caplog, mock_ssh_client
):
    args, _ = mock_args_and_repa

    mock_repoq = MagicMock()
    mock_repoq.solve_repa.side_effect = ValueError("Unknow product: X")

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
    monkeypatch.setattr(Install, "_init_repoq", MagicMock(return_value=mock_repoq))

    with caplog.at_level("ERROR", logger="repose.command.install"):
        Install(args).run()

    assert any("Unknow product" in r.message for r in caplog.records)
