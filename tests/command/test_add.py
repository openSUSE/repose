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
    monkeypatch.setattr(Add, "_init_repoq", MagicMock(return_value=mock_repoq))
    monkeypatch.setattr(Add, "check_url", MagicMock(return_value=True))

    # Instantiate and Run
    add_command = Add(mock_args)
    add_command.run()

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
