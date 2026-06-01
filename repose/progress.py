"""Live per-host progress overlay built on ``rich.live.Live``.

Each row of the table is one target host; each cell is the current
status string posted by the per-host worker. Updates are thread-safe
(workers run concurrently in a ``ThreadPoolExecutor``).

The overlay auto-disables for non-TTY output, ``--format=json``, and
``--quiet`` (see :meth:`repose.command._command.Command._make_progress`
for the gating). When disabled the public surface is a no-op: state
still mutates in-memory (cheap, useful for future consumers) but
nothing is rendered and the logging stack is left untouched.

When enabled, the context manager additionally swaps the active
``logging`` handler on the ``"repose"`` logger family for a
``rich.logging.RichHandler`` bound to the same ``Console`` Live is
writing to. This routes log records through Rich's renderer so they
don't tear the live frame. The previous handler set + level are
snapshotted and restored on ``__exit__`` (including on exceptions).
"""

from __future__ import annotations

import logging
import threading
from contextlib import AbstractContextManager
from dataclasses import dataclass, field
from typing import Iterable

from rich.console import Console as RichConsole
from rich.live import Live
from rich.logging import RichHandler
from rich.table import Table


# Loggers we swap handlers on while Live owns the screen. The root
# logger is intentionally left alone — third-party libraries (paramiko,
# urllib3) keep emitting through their own configured handlers.
_MANAGED_LOGGER_NAMES: tuple[str, ...] = ("repose",)


@dataclass
class _LoggingSnapshot:
    """State needed to restore logging after Live tears down."""

    handlers: dict[str, list[logging.Handler]] = field(default_factory=dict)
    levels: dict[str, int] = field(default_factory=dict)


def _install_rich_logging(console: RichConsole) -> _LoggingSnapshot:
    """Swap managed loggers' handlers for a ``RichHandler`` on ``console``.

    Snapshots the prior handler list + level for each managed logger
    so :func:`_restore_logging` can put everything back verbatim.
    """
    snap = _LoggingSnapshot()
    handler = RichHandler(
        console=console,
        show_path=False,
        show_time=False,
        rich_tracebacks=False,
        markup=False,
    )
    for name in _MANAGED_LOGGER_NAMES:
        lg = logging.getLogger(name)
        snap.handlers[name] = list(lg.handlers)
        snap.levels[name] = lg.level
        for h in list(lg.handlers):
            lg.removeHandler(h)
        lg.addHandler(handler)
    return snap


def _restore_logging(snap: _LoggingSnapshot) -> None:
    """Restore handlers + level captured by :func:`_install_rich_logging`."""
    for name, handlers in snap.handlers.items():
        lg = logging.getLogger(name)
        for h in list(lg.handlers):
            lg.removeHandler(h)
        for h in handlers:
            lg.addHandler(h)
        lg.setLevel(snap.levels.get(name, logging.NOTSET))


class Progress(AbstractContextManager["Progress"]):
    """Live per-host status table.

    Construct with the host list and an ``enabled`` flag; mutate per
    host via :meth:`update`. The context manager owns the
    ``rich.live.Live`` lifetime and the logging swap.

    ``update`` is safe to call from worker threads.
    """

    def __init__(
        self,
        hosts: Iterable[str],
        *,
        enabled: bool,
        console: RichConsole | None = None,
    ) -> None:
        self.enabled = enabled
        self._state: dict[str, str] = {h: "pending" for h in hosts}
        self._lock = threading.Lock()
        self._console = console if console is not None else RichConsole()
        self._live: Live | None = None
        self._log_snapshot: _LoggingSnapshot | None = None

    # ------------------------------------------------------------------
    # Rendering
    # ------------------------------------------------------------------

    def _render(self) -> Table:
        t = Table.grid(padding=(0, 2))
        t.add_column(style="bold")
        t.add_column()
        for host in sorted(self._state):
            t.add_row(host, self._state[host])
        return t

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def update(self, host: str, status: str) -> None:
        """Set the status cell for ``host`` and refresh the live frame.

        Thread-safe. When ``enabled`` is False the state dict still
        mutates so callers can introspect or future consumers can
        observe progress without rendering.
        """
        with self._lock:
            self._state[host] = status
            if self._live is not None:
                self._live.update(self._render())

    @property
    def state(self) -> dict[str, str]:
        """Snapshot copy of per-host status (for tests / introspection)."""
        with self._lock:
            return dict(self._state)

    # ------------------------------------------------------------------
    # Context manager
    # ------------------------------------------------------------------

    def __enter__(self) -> "Progress":
        if not self.enabled:
            return self
        self._log_snapshot = _install_rich_logging(self._console)
        live = Live(
            self._render(),
            console=self._console,
            refresh_per_second=8,
            transient=True,
        )
        live.__enter__()
        self._live = live
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        try:
            if self._live is not None:
                self._live.__exit__(exc_type, exc, tb)
        finally:
            self._live = None
            if self._log_snapshot is not None:
                _restore_logging(self._log_snapshot)
                self._log_snapshot = None
