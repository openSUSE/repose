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


# ---------------------------------------------------------------------------
# check_repo_url_async  (PR 14: httpx-based parallel probing)
# ---------------------------------------------------------------------------


import httpx  # noqa: E402

from unittest.mock import AsyncMock  # noqa: E402


def _async_client_returning(responses: list[object]):
    """Patch ``httpx.AsyncClient`` to yield a context manager whose
    ``.head`` / ``.get`` consume the given response queue."""
    client = MagicMock()
    queue = list(responses)

    async def _next(target, **kw):
        return queue.pop(0)

    client.head = _next
    client.get = _next
    cm = MagicMock()
    cm.__aenter__ = AsyncMock(return_value=client)
    cm.__aexit__ = AsyncMock(return_value=None)
    factory = MagicMock(return_value=cm)
    return factory


async def test_check_repo_url_async_returns_true_on_2xx(monkeypatch):
    ok = MagicMock(status_code=200)
    monkeypatch.setattr(httpx, "AsyncClient", _async_client_returning([ok]))
    assert (await repose.utils.check_repo_url_async("http://example.com/")) is True


async def test_check_repo_url_async_falls_back_to_suse_layout(monkeypatch):
    """First probe (repodata/) → 404; second (suse/repodata/) → 200."""
    bad = MagicMock(status_code=404)
    good = MagicMock(status_code=200)
    monkeypatch.setattr(httpx, "AsyncClient", _async_client_returning([bad, good]))
    assert (await repose.utils.check_repo_url_async("http://example.com/")) is True


async def test_check_repo_url_async_returns_false_when_both_4xx(monkeypatch):
    bad = MagicMock(status_code=404)
    monkeypatch.setattr(httpx, "AsyncClient", _async_client_returning([bad, bad]))
    assert (await repose.utils.check_repo_url_async("http://example.com/")) is False


async def test_check_repo_url_async_swallows_httpx_errors(monkeypatch):
    """A ``httpx.HTTPError`` (e.g. ConnectError) is treated as dead."""

    async def _raise(target, **kw):
        raise httpx.ConnectError("refused")

    client = MagicMock()
    client.head = _raise
    client.get = _raise
    cm = MagicMock()
    cm.__aenter__ = AsyncMock(return_value=client)
    cm.__aexit__ = AsyncMock(return_value=None)
    monkeypatch.setattr(httpx, "AsyncClient", MagicMock(return_value=cm))

    assert (await repose.utils.check_repo_url_async("http://example.com/")) is False


async def test_check_repo_url_async_retries_get_on_405(monkeypatch):
    """A 405 on HEAD must trigger a GET retry; a 200 on GET wins."""
    head_405 = MagicMock(status_code=405)
    get_200 = MagicMock(status_code=200)

    calls: list[str] = []

    async def _head(target, **kw):
        calls.append("head")
        return head_405

    async def _get(target, **kw):
        calls.append("get")
        return get_200

    client = MagicMock()
    client.head = _head
    client.get = _get
    cm = MagicMock()
    cm.__aenter__ = AsyncMock(return_value=client)
    cm.__aexit__ = AsyncMock(return_value=None)
    monkeypatch.setattr(httpx, "AsyncClient", MagicMock(return_value=cm))

    assert (await repose.utils.check_repo_url_async("http://example.com/")) is True
    assert calls == ["head", "get"]
