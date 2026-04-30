"""Tests for ``repose.utils``."""

import importlib

import repose.utils


def test_timestamp_returns_string_of_int(monkeypatch):
    monkeypatch.setattr(repose.utils.time, "time", lambda: 1234567.89)
    assert repose.utils.timestamp() == "1234567"


def test_color_helpers_default_have_ansi_codes():
    # Module is imported with COLOR=always (default)
    assert "\033[1;32m" in repose.utils.green("x")
    assert "\033[1;31m" in repose.utils.red("x")
    assert "\033[1;33m" in repose.utils.yellow("x")
    assert "\033[1;34m" in repose.utils.blue("x")


def test_color_helpers_disabled_when_color_env_overridden(monkeypatch):
    monkeypatch.setenv("COLOR", "never")
    # Reload the module so the COLOR check re-runs
    reloaded = importlib.reload(repose.utils)
    try:
        assert reloaded.green("x") == "x"
        assert reloaded.red("y") == "y"
        assert reloaded.yellow("z") == "z"
        assert reloaded.blue("w") == "w"
    finally:
        # Restore original module state for other tests
        monkeypatch.delenv("COLOR")
        importlib.reload(repose.utils)
