import concurrent.futures
from argparse import Namespace
from unittest.mock import MagicMock

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.reset import Reset


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

    mock_host_group_instance = MagicMock()
    mock_host_group_instance.keys.return_value = ["user@host1"]
    mock_host_group_instance.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_host_group_instance),
    )
    monkeypatch.setattr(Reset, "_init_repoq", MagicMock(return_value=mock_repoq))
    monkeypatch.setattr(Reset, "check_url", MagicMock(return_value=True))

    # Instantiate and Run
    reset_command = Reset(mock_args)
    reset_command.run()

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
