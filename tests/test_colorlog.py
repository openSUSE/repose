"""Tests for ``repose.colorlog``."""

import logging

from repose.colorlog import COLORS, ColorFormatter, PlainFormatter, create_logger


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


def test_create_logger_installs_handler_only_once():
    """Repeated create_logger calls must not stack handlers."""
    name = "repose-test-idempotent-handlers"
    logger = create_logger(name)
    create_logger(name)
    try:
        assert len(logger.handlers) == 1
    finally:
        for handler in list(logger.handlers):
            logger.removeHandler(handler)


def test_create_logger_twice_emits_record_once(capsys):
    """A record logged after two create_logger calls appears exactly once."""
    name = "repose-test-idempotent-emission"
    logger = create_logger(name)
    logger.propagate = False
    create_logger(name)
    try:
        logger.info("emitted-once")
    finally:
        for handler in list(logger.handlers):
            logger.removeHandler(handler)
    assert capsys.readouterr().err.count("emitted-once") == 1


def test_plain_formatter_emits_no_escape_sequences():
    fmt = PlainFormatter("%(levelname)s: %(message)s")
    out = fmt.format(_record("WARNING", "warn"))
    assert "\x1b" not in out
    assert out == "warning: warn"


def test_plain_formatter_debug_keeps_origin_tag_without_ansi():
    record = logging.LogRecord(
        name="repose.test",
        level=logging.DEBUG,
        pathname=__file__,
        lineno=1,
        msg="dbg",
        args=(),
        exc_info=None,
        func="my_func",
    )
    fmt = PlainFormatter("%(levelname)s: %(message)s")
    out = fmt.format(record)
    assert "\x1b" not in out
    assert out == "debug [repose.test:my_func]: dbg"


def test_plain_formatter_without_levelname_token_passes_through():
    fmt = PlainFormatter("%(message)s")
    out = fmt.format(_record("INFO", "plain"))
    assert out == "plain"


def test_create_logger_no_color_attaches_plain_formatter():
    logger = create_logger("repose.test.nocolor", no_color=True)
    formatted = logger.handlers[-1].format(_record("ERROR", "boom"))
    assert "\x1b" not in formatted
    assert formatted == "error: boom"


def test_create_logger_default_attaches_color_formatter():
    logger = create_logger("repose.test.color")
    formatted = logger.handlers[-1].format(_record("ERROR", "boom"))
    assert "\x1b[1;31m" in formatted
