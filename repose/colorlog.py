#
# implementation of a logging.Formatter to enable color output
#

import inspect
import logging
from logging import Logger

(BLACK, RED, GREEN, YELLOW, BLUE, MAGENTA, CYAN, WHITE) = list(range(8))

RESET_SEQ = "\033[0m"
COLOR_SEQ = "\033[1;{}m"

COLORS = {
    "WARNING": YELLOW,
    "INFO": GREEN,
    "DEBUG": BLUE,
    "CRITICAL": RED,
    "ERROR": RED,
}


class ColorFormatter(logging.Formatter):
    def __init__(self, msg: str) -> None:
        super().__init__(msg)

    def formatColor(self, levelname: str) -> str:
        if levelname == "DEBUG":
            caller = inspect.currentframe()
            frame, _, _, function, _, _ = inspect.getouterframes(caller)[9]
            if mod := inspect.getmodule(frame):
                module = mod.__name__
            else:
                module = "unknown"
            return (
                "\033[2K"
                + COLOR_SEQ.format(30 + COLORS[levelname])
                + levelname.lower()
                + RESET_SEQ
                + f" [{module!s}:{function!s}]"
            )
        return (
            "\033[2K"
            + COLOR_SEQ.format(30 + COLORS[levelname])
            + levelname.lower()
            + RESET_SEQ
        )

    def format(self, record: logging.LogRecord) -> str:
        record.message = record.getMessage()
        if self._fmt and self._fmt.find("%(levelname)") >= 0:
            record.levelname = self.formatColor(record.levelname)

        return logging.Formatter.format(self, record)


class PlainFormatter(logging.Formatter):
    """``ColorFormatter``'s message layout without escape sequences.

    Attached instead of :class:`ColorFormatter` when color output is
    disabled (``--no-color`` or the ``NO_COLOR`` environment variable),
    so logs redirected to files or non-ANSI terminals stay clean. The
    line layout is preserved: lowercased level name, plus the
    ``[module:function]`` origin tag on DEBUG records (sourced from the
    record's logger name, which repose names after the module).
    """

    def format(self, record: logging.LogRecord) -> str:
        """Format ``record`` with a decorated but color-free levelname."""
        if self._fmt and self._fmt.find("%(levelname)") >= 0:
            levelname = record.levelname.lower()
            if record.levelname == "DEBUG":
                levelname += f" [{record.name!s}:{record.funcName!s}]"
            record.levelname = levelname

        return logging.Formatter.format(self, record)


def create_logger(
    name: str | None = None, level: str = "INFO", no_color: bool = False
) -> Logger:
    """Return a logger with a stream handler and repose's log format.

    Handler installation is idempotent: if the logger already has a
    handler, no new one is added. Without this guard, every invocation
    (in-process test runners importing the CLI, embedding via
    ``repose.main``, invoking the Typer app twice in one process) would
    stack another ``StreamHandler`` and each log record would be
    emitted once per accumulated handler.

    Args:
        name: Logger to configure; the root logger when omitted.
        level: Initial log level name (e.g. ``"INFO"``).
        no_color: When ``True``, attach a :class:`PlainFormatter` so no
            ANSI escape sequences are emitted; otherwise attach the
            default :class:`ColorFormatter`.

    Returns:
        The configured :class:`logging.Logger`.
    """
    out = logging.getLogger(name) if name else logging.getLogger()
    out.setLevel(level)
    if not out.handlers:
        handler = logging.StreamHandler()
        formatter_cls = PlainFormatter if no_color else ColorFormatter
        handler.setFormatter(formatter_cls("%(levelname)s: %(message)s"))
        out.addHandler(handler)
    return out
