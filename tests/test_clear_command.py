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
    clear_command.run()

    # Assertions
    mock_host_group_instance.read_repos.assert_called_once()

    run_call = mock_target.run.call_args[0][0]
    assert run_call.startswith("zypper -n rr")
    assert "repo1" in run_call
    assert "repo2" in run_call

    mock_host_group_instance.close.assert_called_once()
