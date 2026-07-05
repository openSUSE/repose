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


def create_logger(name: str | None = None, level: str = "INFO") -> Logger:
    """Return the named (or root) logger with a color handler installed.

    Handler installation is idempotent: if the logger already has a
    handler, no new one is added. Without this guard, every invocation
    (in-process test runners importing the CLI, embedding via
    ``repose.main``, invoking the Typer app twice in one process) would
    stack another ``StreamHandler`` and each log record would be
    emitted once per accumulated handler.

    Args:
        name: Logger name; the root logger is used when omitted.
        level: Logging level name to set on the logger.

    Returns:
        The configured :class:`logging.Logger`.
    """
    out = logging.getLogger(name) if name else logging.getLogger()
    out.setLevel(level)
    if not out.handlers:
        handler = logging.StreamHandler()
        formatter = ColorFormatter("%(levelname)s: %(message)s")
        handler.setFormatter(formatter)
        out.addHandler(handler)
    return out
