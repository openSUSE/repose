import concurrent.futures
from argparse import Namespace
from unittest.mock import MagicMock

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.remove import Remove
from repose.types.repa import Repa


class MockProduct:
    def __init__(self, name, version):
        self.name = name
        self.version = version


@pytest.fixture
def mock_args_and_repa():
    """Fixture for command arguments."""
    repa_instance = Repa("SLES:::repo1")
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[repa_instance],
        config="dummy_config",
        yaml=False,
    )
    return args, repa_instance


def test_remove_command_run(monkeypatch, mock_args_and_repa, mock_ssh_client):
    mock_args, _ = mock_args_and_repa

    # Setup Mocks
    mock_target = MagicMock()
    mock_target.products.flatten.return_value = [MockProduct("SLES", "15-SP4")]
    mock_target.repos.keys.return_value = [
        "SLES:15-SP4::repo1",
        "SLES:15-SP4::repo2",
        "other:repo",
    ]

    mock_host_group_instance = MagicMock()
    mock_host_group_instance.keys.return_value = ["user@host1"]
    mock_host_group_instance.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_host_group_instance),
    )

    # Instantiate and Run
    remove_command = Remove(mock_args)
    remove_command.run()

    # Assertions
    mock_host_group_instance.read_repos.assert_called_once()
    mock_host_group_instance.parse_repos.assert_called_once()

    expected_cmd = remove_command.rrcmd.format(repos="SLES:15-SP4::repo1")
    mock_target.run.assert_called_once_with(expected_cmd)

    mock_host_group_instance.close.assert_called_once()
