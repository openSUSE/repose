"""Single sink for user-facing output.

All user-visible lines (dry-run command previews and per-host run
output) flow through :class:`Console`. Logger noise (debug/warning)
stays on the ``logging`` module.

Two orthogonal toggles:

- ``format``: ``"text"`` (default) for humans, ``"json"`` for scripts.
- ``color``: ``"auto"`` (TTY + ``NO_COLOR`` respected), ``"always"``,
  or ``"never"``.
"""

import json
import os
import sys
from dataclasses import dataclass, field
from typing import Any, Literal, TextIO

Format = Literal["text", "json"]
ColorMode = Literal["auto", "always", "never"]
Level = Literal["info", "warning", "error"]

# ANSI sequences mirror ``repose.utils`` so output stays visually
# identical to legacy print/logger calls. We inline them here rather
# than delegate to utils so the Console's ``color`` toggle is the
# single source of truth (utils still keys off env+TTY for callers
# outside the Console, e.g. ``display.py``).
_ANSI_RESET = "\033[1;m"
_ANSI_BY_LEVEL: dict[str, str] = {
    "info": "\033[1;34m",  # blue
    "warning": "\033[1;33m",  # yellow
    "error": "\033[1;31m",  # red
}


@dataclass
class Console:
    """User-facing output sink with format and color toggles."""

    stream: TextIO = field(default_factory=lambda: sys.stdout)
    format: Format = "text"
    color: ColorMode = "auto"

    def _use_color(self) -> bool:
        if self.color == "always":
            return True
        if self.color == "never":
            return False
        if os.environ.get("NO_COLOR"):
            return False
        isatty = getattr(self.stream, "isatty", None)
        return bool(isatty and isatty())

    def _colorize_host(self, host: str, level: Level) -> str:
        if not self._use_color():
            return host
        seq = _ANSI_BY_LEVEL.get(level, _ANSI_BY_LEVEL["info"])
        return f"{seq}{host}{_ANSI_RESET}"

    def dry(self, host: str, cmd: str) -> None:
        """Emit a dry-run command preview."""
        self._emit("dry", level="info", host=host, cmd=cmd)

    def report(
        self,
        host: str,
        line: str,
        *,
        ok: bool,
        level: Level = "info",
    ) -> None:
        """Emit one line of per-host run output."""
        self._emit("report", level=level, host=host, line=line, ok=ok)

    def error(self, host: str, msg: str) -> None:
        """Emit a host-scoped error line."""
        self._emit("error", level="error", host=host, line=msg, ok=False)

    def info(self, msg: str) -> None:
        """Emit an unscoped informational line."""
        self._emit("info", level="info", line=msg)

    def _emit(self, event: str, *, level: Level, **fields: Any) -> None:
        if self.format == "json":
            payload: dict[str, Any] = {"event": event, "level": level}
            payload.update(fields)
            self.stream.write(json.dumps(payload) + "\n")
            self.stream.flush()
            return

        host = fields.get("host")
        cmd = fields.get("cmd")
        line = fields.get("line")

        if event == "dry" and host is not None and cmd is not None:
            prefix = self._colorize_host(str(host), "info")
            self.stream.write(f"{prefix} - {cmd}\n")
        elif event == "report" and host is not None and line is not None:
            prefix = self._colorize_host(str(host), level)
            self.stream.write(f"{prefix} - {line}\n")
        elif event == "error" and host is not None and line is not None:
            prefix = self._colorize_host(str(host), "error")
            self.stream.write(f"{prefix} - {line}\n")
        elif event == "info" and line is not None:
            self.stream.write(f"{line}\n")
        self.stream.flush()
