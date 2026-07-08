"""Tests for ``repose.colorlog``."""

import logging

import repose.colorlog as _colorlog_mod
from repose.colorlog import (
    COLORS,
    ColorFormatter,
    PlainFormatter,
    create_logger,
)


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


# ``formatColor`` inspects the DEBUG origin frame at a fixed stack depth of
# ``inspect.getouterframes(caller)[9]`` to build its ``[module:function]`` tag.
# Asserting that tag deterministically means controlling which frame lands at
# depth 9 (its function name) and depth 10 (the neighbour a wrong index would
# report).  Two details make this reproducible across runners:
#
#   * The chain is compiled with a synthetic filename so ``inspect.getmodule``
#     cannot resolve the origin frame to an importable module; the module
#     component then falls back to the literal ``"unknown"`` everywhere.
#   * The chain alternates between two mutually recursive functions, so depth 9
#     always reports ``_origin_odd`` and depth 10 always reports
#     ``_origin_even`` regardless of how many wrapper frames a runner injects
#     between the formatter and its caller (the injected count is even).
_ORIGIN_SRC = """
def _origin_even(fmt, record_factory, n):
    if n <= 0:
        return fmt.format(record_factory("DEBUG", "hi"))
    return _origin_odd(fmt, record_factory, n - 1)


def _origin_odd(fmt, record_factory, n):
    if n <= 0:
        return fmt.format(record_factory("DEBUG", "hi"))
    return _origin_even(fmt, record_factory, n - 1)


def _emit_debug_origin(fmt, record_factory):
    return _origin_even(fmt, record_factory, 10)
"""


def _format_debug_origin(fmt):
    namespace: dict = {}
    exec(compile(_ORIGIN_SRC, "<repose-colorlog-tests>", "exec"), namespace)
    return namespace["_emit_debug_origin"](fmt, _record)


# The exec-above deliberately defeats module resolution (synthetic filename) so
# ``formatColor``'s ``else`` branch -- ``module = "unknown"`` -- is exercised.
# To also cover the *truthy* branch at colorlog.py:31-32
# (``if mod := inspect.getmodule(frame): module = mod.__name__``) the SAME chain
# is compiled instead against the module-under-test's own ``__file__`` (read
# live from the imported module).  ``inspect.getmodule`` resolves a frame by
# matching ``frame.f_code.co_filename`` against a loaded module's ``__file__``,
# so tagging the origin frames with colorlog's real path makes it resolve the
# depth-9 frame to the (always-importable) colorlog module and take the truthy
# branch.
#
# Deriving both the compile filename and the expected module name from the same
# live module object keeps this stable under mutmut: mutmut writes the mutant
# ``colorlog.py`` to its ``mutants/`` tree, so ``_colorlog_mod.__file__`` there
# points at that on-disk copy and the frames resolve identically.  (Tagging the
# frames with the *test* module's filename would be fragile -- a stale, copied
# ``__pycache__`` can leave the compiled test carrying its original path, which
# no longer matches the copy's ``__file__`` and silently falls back to
# ``"unknown"``.)
def _format_debug_origin_resolved(fmt):
    namespace: dict = {}
    exec(compile(_ORIGIN_SRC, _colorlog_mod.__file__, "exec"), namespace)
    return namespace["_emit_debug_origin"](fmt, _record)


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


def test_non_debug_record_has_erase_line_prefix_and_no_origin_tag():
    # Setup: a non-DEBUG record through the full ``%(levelname)s`` format.
    fmt = ColorFormatter("%(levelname)s: %(message)s")

    # Exercise
    out = fmt.format(_record("INFO", "hi"))

    # Verify: exact byte sequence -- \033[2K erase-line prefix, bold-green
    # (30 + GREEN == 32) level, lowercased name, reset, and *no*
    # ``[module:function]`` origin tag (that is DEBUG-only).
    assert out == "\033[2K\033[1;32minfo\033[0m: hi"


def test_debug_record_appends_origin_tag_with_level_color():
    # Setup
    fmt = ColorFormatter("%(levelname)s: %(message)s")

    # Exercise: emit from a controlled 9-frame-deep stack (see helper above).
    out = _format_debug_origin(fmt)

    # Verify: exact byte sequence -- \033[2K erase-line prefix, bold-blue
    # (30 + BLUE == 34) level, lowercased ``debug``, reset, then the
    # ``[module:function]`` origin tag recovered from the frame at depth 9.
    assert out == "\033[2K\033[1;34mdebug\033[0m [unknown:_origin_odd]: hi"


def test_debug_origin_tag_reports_resolved_module_not_unknown_or_none():
    # Setup
    fmt = ColorFormatter("%(levelname)s: %(message)s")

    # Exercise: origin frames now carry colorlog's own ``__file__``, so
    # ``inspect.getmodule(frame)`` resolves the depth-9 frame to a real module
    # and the truthy branch (``module = mod.__name__``) runs.
    out = _format_debug_origin_resolved(fmt)

    # Verify: the module component is a real, non-empty name -- never the
    # ``else`` fallback ``"unknown"`` (which mutating ``getmodule(frame)`` to
    # ``getmodule(None)`` forces) nor the stringified ``None`` (which mutating
    # ``module = mod.__name__`` to ``module = None`` produces).  The name is
    # parsed out of the tag rather than hard-coded so the kill holds even if a
    # runner aliases the module's ``__name__``; ``_origin_odd`` at depth 9 stays
    # fixed by the alternating recursion.
    prefix = "\033[2K\033[1;34mdebug\033[0m ["
    assert out.startswith(prefix)
    module, rest = out[len(prefix) :].split(":", 1)
    assert rest == "_origin_odd]: hi"
    assert module and module not in ("unknown", "None")
    assert module == _colorlog_mod.__name__
