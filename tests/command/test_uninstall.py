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
    uninstall_command.run()

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
