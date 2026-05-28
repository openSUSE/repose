"""Tests for ``repose.main.main``."""

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


def test_main_prints_usage_when_no_subcommand(monkeypatch, capsys):
    monkeypatch.setattr(sys, "argv", ["repose"])
    with pytest.raises(SystemExit) as exc:
        main_mod.main()
    assert exc.value.code == 0
    out = capsys.readouterr().out
    assert "usage" in out.lower()


def _fake_namespace(*, func, debug=False, quiet=False):
    """Build a minimal argparse-style Namespace for monkeypatching parse."""
    import argparse

    return argparse.Namespace(func=func, debug=debug, quiet=quiet)


def test_main_invokes_subcommand_func(monkeypatch):
    sentinel_func = MagicMock(return_value=0)

    monkeypatch.setattr(
        main_mod, "parse", lambda argv: _fake_namespace(func=sentinel_func)
    )
    monkeypatch.setattr(sys, "argv", ["repose", "foo"])

    with pytest.raises(SystemExit) as exc:
        main_mod.main()

    sentinel_func.assert_called_once()
    assert exc.value.code == 0


def test_main_debug_sets_debug_level(monkeypatch):
    captured = {}

    monkeypatch.setattr(
        main_mod,
        "parse",
        lambda argv: _fake_namespace(func=lambda a: 0, debug=True),
    )

    def fake_create_logger(name):
        log = logging.getLogger(name)
        captured["logger"] = log
        return log

    monkeypatch.setattr(main_mod, "create_logger", fake_create_logger)
    monkeypatch.setattr(sys, "argv", ["repose", "--debug", "foo"])

    with pytest.raises(SystemExit):
        main_mod.main()

    assert captured["logger"].level == logging.DEBUG


def test_main_quiet_sets_warning_level(monkeypatch):
    captured = {}

    monkeypatch.setattr(
        main_mod,
        "parse",
        lambda argv: _fake_namespace(func=lambda a: 0, quiet=True),
    )

    def fake_create_logger(name):
        log = logging.getLogger(name)
        captured["logger"] = log
        return log

    monkeypatch.setattr(main_mod, "create_logger", fake_create_logger)
    monkeypatch.setattr(sys, "argv", ["repose", "--quiet", "foo"])

    with pytest.raises(SystemExit):
        main_mod.main()

    assert captured["logger"].level == logging.WARNING


def test_main_propagates_func_exit_code(monkeypatch):
    monkeypatch.setattr(
        main_mod, "parse", lambda argv: _fake_namespace(func=lambda a: 7)
    )
    monkeypatch.setattr(sys, "argv", ["repose", "foo"])

    with pytest.raises(SystemExit) as exc:
        main_mod.main()
    assert exc.value.code == 7


def _raiser(exc):
    def _f(_args):
        raise exc

    return _f


def test_main_friendly_message_on_missing_config(monkeypatch, caplog):
    err = FileNotFoundError(2, "No such file or directory", "/etc/repose/products.yml")
    monkeypatch.setattr(
        main_mod, "parse", lambda argv: _fake_namespace(func=_raiser(err))
    )
    monkeypatch.setattr(sys, "argv", ["repose", "known-products"])

    with caplog.at_level(logging.ERROR, logger="repose"):
        with pytest.raises(SystemExit) as exc:
            main_mod.main()

    assert exc.value.code == 2
    assert "config file not found" in caplog.text
    assert "/etc/repose/products.yml" in caplog.text


def test_main_friendly_message_on_permission_error(monkeypatch, caplog):
    err = PermissionError(13, "Permission denied", "/etc/repose/products.yml")
    monkeypatch.setattr(
        main_mod, "parse", lambda argv: _fake_namespace(func=_raiser(err))
    )
    monkeypatch.setattr(sys, "argv", ["repose", "known-products"])

    with caplog.at_level(logging.ERROR, logger="repose"):
        with pytest.raises(SystemExit) as exc:
            main_mod.main()

    assert exc.value.code == 2
    assert "permission denied" in caplog.text.lower()


def test_main_friendly_message_on_yaml_error(monkeypatch, caplog):
    from ruamel.yaml import YAMLError

    monkeypatch.setattr(
        main_mod,
        "parse",
        lambda argv: _fake_namespace(func=_raiser(YAMLError("bad yaml"))),
    )
    monkeypatch.setattr(sys, "argv", ["repose", "known-products"])

    with caplog.at_level(logging.ERROR, logger="repose"):
        with pytest.raises(SystemExit) as exc:
            main_mod.main()

    assert exc.value.code == 2
    assert "invalid yaml" in caplog.text.lower()


def test_main_debug_propagates_traceback(monkeypatch):
    """With ``--debug``, the original exception is re-raised intact."""
    err = FileNotFoundError(2, "No such file or directory", "/nope")
    monkeypatch.setattr(
        main_mod,
        "parse",
        lambda argv: _fake_namespace(func=_raiser(err), debug=True),
    )
    monkeypatch.setattr(sys, "argv", ["repose", "--debug", "known-products"])

    with pytest.raises(FileNotFoundError):
        main_mod.main()


def test_main_keyboard_interrupt_returns_130(monkeypatch, caplog):
    monkeypatch.setattr(
        main_mod,
        "parse",
        lambda argv: _fake_namespace(func=_raiser(KeyboardInterrupt())),
    )
    monkeypatch.setattr(sys, "argv", ["repose", "known-products"])

    with caplog.at_level(logging.ERROR, logger="repose"):
        with pytest.raises(SystemExit) as exc:
            main_mod.main()

    assert exc.value.code == 130
