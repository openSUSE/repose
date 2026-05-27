"""Tests for ``repose.utils``."""

import io

import repose.utils


def test_timestamp_returns_string_of_int(monkeypatch):
    monkeypatch.setattr(repose.utils.time, "time", lambda: 1234567.89)
    assert repose.utils.timestamp() == "1234567"


def test_color_helpers_have_ansi_codes_when_color_always(monkeypatch):
    monkeypatch.setenv("COLOR", "always")
    monkeypatch.delenv("NO_COLOR", raising=False)
    assert "\033[1;32m" in repose.utils.green("x")
    assert "\033[1;31m" in repose.utils.red("x")
    assert "\033[1;33m" in repose.utils.yellow("x")
    assert "\033[1;34m" in repose.utils.blue("x")


def test_color_helpers_disabled_when_color_never(monkeypatch):
    monkeypatch.setenv("COLOR", "never")
    monkeypatch.delenv("NO_COLOR", raising=False)
    assert repose.utils.green("x") == "x"
    assert repose.utils.red("y") == "y"
    assert repose.utils.yellow("z") == "z"
    assert repose.utils.blue("w") == "w"


def test_no_color_env_disables_colors(monkeypatch):
    monkeypatch.delenv("COLOR", raising=False)
    monkeypatch.setenv("NO_COLOR", "1")
    assert repose.utils.green("x") == "x"
    assert repose.utils.blue("x") == "x"


def test_color_always_beats_no_color(monkeypatch):
    monkeypatch.setenv("COLOR", "always")
    monkeypatch.setenv("NO_COLOR", "1")
    assert "\033[1;34m" in repose.utils.blue("x")


def test_non_tty_stdout_disables_colors(monkeypatch):
    monkeypatch.delenv("COLOR", raising=False)
    monkeypatch.delenv("NO_COLOR", raising=False)
    monkeypatch.setattr(repose.utils.sys, "stdout", io.StringIO())
    assert repose.utils.green("x") == "x"
    assert repose.utils.blue("x") == "x"


def test_tty_stdout_enables_colors(monkeypatch):
    monkeypatch.delenv("COLOR", raising=False)
    monkeypatch.delenv("NO_COLOR", raising=False)

    class FakeTTY(io.StringIO):
        def isatty(self) -> bool:
            return True

    monkeypatch.setattr(repose.utils.sys, "stdout", FakeTTY())
    assert "\033[1;34m" in repose.utils.blue("x")
