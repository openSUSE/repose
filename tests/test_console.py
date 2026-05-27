"""Tests for ``repose.console.Console``."""

import io
import json

import pytest

from repose.console import Console


ANSI_BLUE = "\033[1;34m"
ANSI_RED = "\033[1;31m"
ANSI_YELLOW = "\033[1;33m"


# ---------------------------------------------------------------------------
# Text-mode output
# ---------------------------------------------------------------------------


def test_text_dry_normalized_shape():
    stream = io.StringIO()
    c = Console(stream=stream, format="text", color="never")
    c.dry("user@host1", "zypper -n ar foo")

    assert stream.getvalue() == "user@host1 - zypper -n ar foo\n"


def test_text_dry_colored_when_color_always():
    stream = io.StringIO()
    c = Console(stream=stream, format="text", color="always")
    c.dry("user@host1", "cmd")

    out = stream.getvalue()
    assert ANSI_BLUE in out
    assert "user@host1" in out
    assert "cmd" in out


def test_text_dry_no_ansi_when_color_never():
    stream = io.StringIO()
    c = Console(stream=stream, format="text", color="never")
    c.dry("user@host1", "cmd")

    out = stream.getvalue()
    assert "\033[" not in out


def test_text_report_color_per_level_always():
    stream = io.StringIO()
    c = Console(stream=stream, format="text", color="always")
    c.report("h", "ok line", ok=True, level="info")
    c.report("h", "warn line", ok=True, level="warning")
    c.report("h", "err line", ok=False, level="error")

    out = stream.getvalue()
    assert ANSI_BLUE in out
    assert ANSI_YELLOW in out
    assert ANSI_RED in out


def test_text_info_emits_unscoped_line():
    stream = io.StringIO()
    c = Console(stream=stream, format="text", color="never")
    c.info("something happened")

    assert stream.getvalue() == "something happened\n"


def test_text_error_with_host_prefix():
    stream = io.StringIO()
    c = Console(stream=stream, format="text", color="never")
    c.error("h", "boom")

    assert stream.getvalue() == "h - boom\n"


# ---------------------------------------------------------------------------
# JSON-mode output
# ---------------------------------------------------------------------------


def test_json_dry_parseable():
    stream = io.StringIO()
    c = Console(stream=stream, format="json")
    c.dry("h", "cmd")

    payload = json.loads(stream.getvalue())
    assert payload == {
        "event": "dry",
        "level": "info",
        "host": "h",
        "cmd": "cmd",
    }


def test_json_report_includes_ok_and_level():
    stream = io.StringIO()
    c = Console(stream=stream, format="json")
    c.report("h", "line", ok=False, level="error")

    payload = json.loads(stream.getvalue())
    assert payload == {
        "event": "report",
        "level": "error",
        "host": "h",
        "line": "line",
        "ok": False,
    }


def test_json_one_object_per_call_newline_separated():
    stream = io.StringIO()
    c = Console(stream=stream, format="json")
    c.dry("h1", "c1")
    c.report("h2", "l2", ok=True)
    c.error("h3", "err")
    c.info("hi")

    lines = stream.getvalue().splitlines()
    assert len(lines) == 4
    events = [json.loads(line)["event"] for line in lines]
    assert events == ["dry", "report", "error", "info"]


def test_json_no_ansi_even_when_color_always():
    stream = io.StringIO()
    c = Console(stream=stream, format="json", color="always")
    c.dry("h", "cmd")

    assert "\033[" not in stream.getvalue()


# ---------------------------------------------------------------------------
# Color-mode precedence
# ---------------------------------------------------------------------------


def test_no_color_env_overrides_auto(monkeypatch):
    stream = io.StringIO()
    monkeypatch.setenv("NO_COLOR", "1")
    c = Console(stream=stream, format="text", color="auto")

    assert c._use_color() is False


def test_color_always_beats_no_color_env(monkeypatch):
    stream = io.StringIO()
    monkeypatch.setenv("NO_COLOR", "1")
    c = Console(stream=stream, format="text", color="always")

    assert c._use_color() is True


def test_non_tty_stream_disables_auto_color(monkeypatch):
    monkeypatch.delenv("NO_COLOR", raising=False)
    stream = io.StringIO()  # no .isatty() True
    c = Console(stream=stream, format="text", color="auto")

    assert c._use_color() is False


def test_tty_stream_enables_auto_color(monkeypatch):
    monkeypatch.delenv("NO_COLOR", raising=False)

    class FakeTTY(io.StringIO):
        def isatty(self) -> bool:
            return True

    stream = FakeTTY()
    c = Console(stream=stream, format="text", color="auto")

    assert c._use_color() is True


# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------


def test_default_format_is_text():
    c = Console()
    assert c.format == "text"


def test_default_color_is_auto():
    c = Console()
    assert c.color == "auto"


@pytest.mark.parametrize("level", ["info", "warning", "error"])
def test_report_levels_all_emit_in_text(level):
    stream = io.StringIO()
    c = Console(stream=stream, format="text", color="never")
    c.report("h", "line", ok=True, level=level)

    assert stream.getvalue() == "h - line\n"
