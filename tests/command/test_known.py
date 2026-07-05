from argparse import Namespace
from unittest.mock import MagicMock

import pytest

import repose.command._command
import repose.command.known
from repose.command.known import KnownProducts
from repose.template import TemplateError


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


def test_known_products_empty_template_lists_nothing(monkeypatch, mock_args, tmp_path):
    """An empty products.yml yields a clean, empty listing (exit 0).

    Regression: safe-YAML loads an empty file as ``None``; before
    ``load_template`` normalized it to ``{}``, ``template.keys()``
    raised ``AttributeError`` and the command died with a traceback.
    Uses the real ``load_template`` on a real empty file on purpose.
    """
    empty = tmp_path / "products.yml"
    empty.write_text("")
    mock_args.config = empty
    monkeypatch.setattr(repose.command._command, "HostGroup", MagicMock())

    known_command = KnownProducts(mock_args)
    known_command.display = MagicMock()

    assert known_command.run() == 0
    known_command.display.list_known_products.assert_called_once_with([])


def test_known_products_comment_only_template_lists_nothing(
    monkeypatch, mock_args, tmp_path
):
    """A comment-only products.yml also yields a clean, empty listing.

    Same regression as the empty-file case: safe-YAML loads a
    comment-only document as ``None``.
    """
    comments = tmp_path / "products.yml"
    comments.write_text("# no products defined yet\n")
    mock_args.config = comments
    monkeypatch.setattr(repose.command._command, "HostGroup", MagicMock())

    known_command = KnownProducts(mock_args)
    known_command.display = MagicMock()

    assert known_command.run() == 0
    known_command.display.list_known_products.assert_called_once_with([])


def test_known_products_non_mapping_template_raises_template_error(
    monkeypatch, mock_args, tmp_path
):
    """A top-level-sequence products.yml raises ``TemplateError``.

    The command layer propagates it untouched so ``repose.cli._dispatch``
    can translate it into the one-line config error + exit 2 (covered
    end to end by ``test_friendly_message_on_non_mapping_config`` in
    ``tests/test_cli.py``).
    """
    listy = tmp_path / "products.yml"
    listy.write_text("- SLES\n- openSUSE-Leap\n")
    mock_args.config = listy
    monkeypatch.setattr(repose.command._command, "HostGroup", MagicMock())

    known_command = KnownProducts(mock_args)
    known_command.display = MagicMock()

    with pytest.raises(TemplateError, match="must be a YAML mapping, got list"):
        known_command.run()
    known_command.display.list_known_products.assert_not_called()


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
