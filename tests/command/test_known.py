from argparse import Namespace
from unittest.mock import MagicMock

import pytest

import repose.command._command
import repose.command.known
from repose.command.known import KnownProducts


@pytest.fixture
def mock_args():
    return Namespace(
        dry=False,
        target=[],  # Must be iterable
        repa=None,
        config="dummy_config",
        yaml=False,
    )


def test_known_products_command(monkeypatch, mock_args):
    # Mocks
    mock_load_template = MagicMock(
        return_value={"product-c": {}, "product-a": {}, "product-b": {}}
    )
    monkeypatch.setattr(repose.command.known, "load_template", mock_load_template)
    monkeypatch.setattr(repose.command._command, "HostGroup", MagicMock())

    # Run
    known_command = KnownProducts(mock_args)
    known_command.display = MagicMock()
    known_command.run()

    # Assertions
    mock_load_template.assert_called_once()
    known_command.display.list_known_products.assert_called_once_with(
        ["product-a", "product-b", "product-c"]
    )


def test_known_products_format_json_end_to_end(monkeypatch, mock_args, capsys):
    """--format=json emits one known_product event per template key."""
    import json

    mock_args.format = "json"
    monkeypatch.setattr(
        repose.command.known,
        "load_template",
        MagicMock(return_value={"product-c": {}, "product-a": {}, "product-b": {}}),
    )
    monkeypatch.setattr(repose.command._command, "HostGroup", MagicMock())

    assert KnownProducts(mock_args).run() == 0

    lines = [line for line in capsys.readouterr().out.splitlines() if line.strip()]
    events = [json.loads(line) for line in lines]
    assert [e["event"] for e in events] == ["known_product"] * 3
    assert [e["name"] for e in events] == ["product-a", "product-b", "product-c"]
