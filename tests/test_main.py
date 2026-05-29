"""Tests for the ``repose.main`` console-script shim.

``repose.main.main`` is now a 3-line shim that delegates to
:data:`repose.cli.app`. The rich behavioural surface (parsing, exit
codes, friendly error handling, debug/quiet logger level, dispatch
through ``Command.registry``) is covered by :mod:`tests.test_cli`.
This module only asserts the shim wiring is intact, which protects
downstream packaging that still imports ``from repose.main import main``.
"""

import logging
import sys
from unittest.mock import MagicMock

import pytest

import repose.main as main_mod


@pytest.fixture(autouse=True)
def _reset_logger():
    """Detach handlers from the named 'repose' logger between tests."""
    logger = logging.getLogger("repose")
    saved = list(logger.handlers)
    yield
    logger.handlers = saved


def test_main_delegates_to_typer_app(monkeypatch):
    """``main()`` calls ``repose.cli.app`` exactly once."""
    fake_app = MagicMock()
    monkeypatch.setattr(main_mod, "app", fake_app)
    main_mod.main()
    fake_app.assert_called_once_with()


def test_main_prints_usage_when_no_subcommand(monkeypatch, capsys):
    """Bare ``repose`` renders the Typer help on stdout and exits 0."""
    monkeypatch.setattr(sys, "argv", ["repose"])
    with pytest.raises(SystemExit) as exc:
        main_mod.main()
    assert exc.value.code == 0
    out = capsys.readouterr().out
    assert "usage" in out.lower()
    # Every subcommand surfaces in the help block.
    for cmd in (
        "add",
        "remove",
        "reset",
        "install",
        "clear",
        "uninstall",
        "list-products",
        "list-repos",
        "known-products",
    ):
        assert cmd in out
