"""Tests for ``repose.colorlog``."""

import logging

from repose.colorlog import COLORS, ColorFormatter, create_logger


def _record(levelname="INFO", msg="hello"):
    return logging.LogRecord(
        name="test",
        level=getattr(logging, levelname),
        pathname=__file__,
        lineno=1,
        msg=msg,
        args=(),
        exc_info=None,
    )


def test_color_constants_cover_all_standard_levels():
    assert set(COLORS.keys()) == {"WARNING", "INFO", "DEBUG", "CRITICAL", "ERROR"}


def test_format_info_includes_color_sequence():
    fmt = ColorFormatter("%(levelname)s: %(message)s")
    out = fmt.format(_record("INFO", "hi"))
    assert "info" in out
    assert "hi" in out
    # Green sequence \033[1;32m
    assert "\033[1;32m" in out


def test_format_warning_uses_yellow():
    fmt = ColorFormatter("%(levelname)s: %(message)s")
    out = fmt.format(_record("WARNING", "warn"))
    assert "\033[1;33m" in out


def test_format_error_uses_red():
    fmt = ColorFormatter("%(levelname)s: %(message)s")
    out = fmt.format(_record("ERROR", "err"))
    assert "\033[1;31m" in out


def test_format_without_levelname_token_skips_color():
    fmt = ColorFormatter("%(message)s")
    out = fmt.format(_record("INFO", "plain"))
    # No color escape since formatColor branch skipped
    assert "\033[1;32m" not in out
    assert "plain" in out


def test_create_logger_returns_named_logger():
    logger = create_logger("repose.test", level="DEBUG")
    assert logger.name == "repose.test"
    assert logger.level == logging.DEBUG
    # at least one handler attached
    assert any(isinstance(h, logging.StreamHandler) for h in logger.handlers)


def test_create_logger_root_when_no_name():
    logger = create_logger(level="WARNING")
    assert logger.level == logging.WARNING
