from argparse import Namespace
from unittest.mock import MagicMock

import pytest

import repose.command._command
from repose.command.list import ListProducts, ListRepos


@pytest.fixture
def mock_args():
    return Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=None,
        config="dummy_config",
        yaml=False,
    )


def test_list_repos_command(monkeypatch, mock_args, mock_ssh_client):
    # Mocks
    mock_host_group_instance = MagicMock()
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_host_group_instance),
    )

    # Run
    list_repos_command = ListRepos(mock_args)
    list_repos_command.display = MagicMock()
    list_repos_command.run()

    # Assertions
    mock_host_group_instance.read_repos.assert_called_once()
    mock_host_group_instance.report_repos.assert_called_once_with(
        list_repos_command.display.list_update_repos
    )
    mock_host_group_instance.close.assert_called_once()


def test_list_products_command(monkeypatch, mock_args, mock_ssh_client):
    # Mocks
    mock_host_group_instance = MagicMock()
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_host_group_instance),
    )

    # Run
    list_products_command = ListProducts(mock_args)
    list_products_command.display = MagicMock()
    list_products_command.run()

    # Assertions
    mock_host_group_instance.read_products.assert_called_once()
    mock_host_group_instance.report_products.assert_called_once_with(
        list_products_command.display.list_products
    )
    mock_host_group_instance.close.assert_called_once()


def test_list_products_command_yaml(monkeypatch, mock_args, mock_ssh_client):
    # Mocks
    mock_args.yaml = True
    mock_host_group_instance = MagicMock()
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_host_group_instance),
    )

    # Run
    list_products_command = ListProducts(mock_args)
    list_products_command.display = MagicMock()
    list_products_command.run()

    # Assertions
    mock_host_group_instance.read_products.assert_called_once()
    mock_host_group_instance.report_products_yaml.assert_called_once_with(
        list_products_command.display.list_products_yaml
    )
    mock_host_group_instance.close.assert_called_once()
