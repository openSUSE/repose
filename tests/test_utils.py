"""Tests for ``repose.utils``."""

import io
from unittest.mock import MagicMock
from urllib.error import HTTPError, URLError

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


# ---------------------------------------------------------------------------
# check_repo_url
# ---------------------------------------------------------------------------


def test_check_repo_url_returns_true_when_primary_opens(monkeypatch):
    """Canonical ``repodata/repomd.xml`` answers -> True, no fallback."""
    called: list[str] = []

    def _ok(url, timeout):
        called.append(url)
        return MagicMock()

    monkeypatch.setattr(repose.utils, "urlopen", _ok)
    assert repose.utils.check_repo_url("http://example.com/") is True
    assert called == ["http://example.com/repodata/repomd.xml"]


def test_check_repo_url_falls_back_to_suse_layout(monkeypatch):
    """Primary 404s, ``suse/repodata/repomd.xml`` answers -> True."""
    called: list[str] = []

    def _selective(url, timeout):
        called.append(url)
        if "suse/repodata/repomd.xml" in url:
            return MagicMock()
        raise URLError("not at root")

    monkeypatch.setattr(repose.utils, "urlopen", _selective)
    assert repose.utils.check_repo_url("http://example.com/") is True
    assert called == [
        "http://example.com/repodata/repomd.xml",
        "http://example.com/suse/repodata/repomd.xml",
    ]


def test_check_repo_url_returns_false_when_both_fail_urlerror(monkeypatch):
    def _raise(url, timeout):
        raise URLError("nope")

    monkeypatch.setattr(repose.utils, "urlopen", _raise)
    assert repose.utils.check_repo_url("http://example.com/") is False


def test_check_repo_url_returns_false_on_http_error(monkeypatch):
    def _raise(url, timeout):
        raise HTTPError(url, 404, "not found", {}, None)

    monkeypatch.setattr(repose.utils, "urlopen", _raise)
    assert repose.utils.check_repo_url("http://example.com/") is False


def test_check_repo_url_returns_false_on_timeout(monkeypatch):
    """A ``TimeoutError`` raised by ``urlopen`` is caught, not propagated."""

    def _raise(url, timeout):
        raise TimeoutError("slow")

    monkeypatch.setattr(repose.utils, "urlopen", _raise)
    assert repose.utils.check_repo_url("http://example.com/") is False


def test_check_repo_url_returns_false_on_oserror(monkeypatch):
    """OSError (e.g. socket refused) is caught, not propagated."""

    def _raise(url, timeout):
        raise OSError("connection refused")

    monkeypatch.setattr(repose.utils, "urlopen", _raise)
    assert repose.utils.check_repo_url("http://example.com/") is False


def test_check_repo_url_default_timeout_is_5_seconds(monkeypatch):
    seen: list[float] = []

    def _capture(url, timeout):
        seen.append(timeout)
        return MagicMock()

    monkeypatch.setattr(repose.utils, "urlopen", _capture)
    repose.utils.check_repo_url("http://example.com/")
    assert seen == [5.0]


def test_check_repo_url_custom_timeout_is_forwarded(monkeypatch):
    seen: list[float] = []

    def _capture(url, timeout):
        seen.append(timeout)
        raise URLError("first fails to force second attempt")

    monkeypatch.setattr(repose.utils, "urlopen", _capture)
    repose.utils.check_repo_url("http://example.com/", timeout=0.5)
    # Both probes attempted, both saw the custom timeout.
    assert seen == [0.5, 0.5]
