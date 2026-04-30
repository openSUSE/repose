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


def test_main_invokes_subcommand_func(monkeypatch):
    sentinel_func = MagicMock(return_value=0)

    def fake_get_parser():
        import argparse

        p = argparse.ArgumentParser(prog="repose")
        p.add_argument("--debug", action="store_true")
        p.add_argument("--quiet", action="store_true")
        sub = p.add_subparsers()
        s = sub.add_parser("foo")
        s.set_defaults(func=sentinel_func)
        return p

    monkeypatch.setattr(main_mod, "get_parser", fake_get_parser)
    monkeypatch.setattr(sys, "argv", ["repose", "foo"])

    with pytest.raises(SystemExit) as exc:
        main_mod.main()

    sentinel_func.assert_called_once()
    assert exc.value.code == 0


def test_main_debug_sets_debug_level(monkeypatch):
    captured = {}

    def fake_get_parser():
        import argparse

        p = argparse.ArgumentParser(prog="repose")
        p.add_argument("--debug", action="store_true")
        p.add_argument("--quiet", action="store_true")
        sub = p.add_subparsers()
        s = sub.add_parser("foo")
        s.set_defaults(func=lambda a: 0)
        return p

    def fake_create_logger(name):
        log = logging.getLogger(name)
        captured["logger"] = log
        return log

    monkeypatch.setattr(main_mod, "get_parser", fake_get_parser)
    monkeypatch.setattr(main_mod, "create_logger", fake_create_logger)
    monkeypatch.setattr(sys, "argv", ["repose", "--debug", "foo"])

    with pytest.raises(SystemExit):
        main_mod.main()

    assert captured["logger"].level == logging.DEBUG


def test_main_quiet_sets_warning_level(monkeypatch):
    captured = {}

    def fake_get_parser():
        import argparse

        p = argparse.ArgumentParser(prog="repose")
        p.add_argument("--debug", action="store_true")
        p.add_argument("--quiet", action="store_true")
        sub = p.add_subparsers()
        s = sub.add_parser("foo")
        s.set_defaults(func=lambda a: 0)
        return p

    def fake_create_logger(name):
        log = logging.getLogger(name)
        captured["logger"] = log
        return log

    monkeypatch.setattr(main_mod, "get_parser", fake_get_parser)
    monkeypatch.setattr(main_mod, "create_logger", fake_create_logger)
    monkeypatch.setattr(sys, "argv", ["repose", "--quiet", "foo"])

    with pytest.raises(SystemExit):
        main_mod.main()

    assert captured["logger"].level == logging.WARNING


def test_main_propagates_func_exit_code(monkeypatch):
    def fake_get_parser():
        import argparse

        p = argparse.ArgumentParser(prog="repose")
        p.add_argument("--debug", action="store_true")
        p.add_argument("--quiet", action="store_true")
        sub = p.add_subparsers()
        s = sub.add_parser("foo")
        s.set_defaults(func=lambda a: 7)
        return p

    monkeypatch.setattr(main_mod, "get_parser", fake_get_parser)
    monkeypatch.setattr(sys, "argv", ["repose", "foo"])

    with pytest.raises(SystemExit) as exc:
        main_mod.main()
    assert exc.value.code == 7
