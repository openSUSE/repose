"""Async equivalent of :class:`repose.target.hostgroup.HostGroup`.

Uses :class:`asyncio.TaskGroup` for structured concurrency: Ctrl-C
cancels every in-flight host coroutine cleanly, and unhandled
exceptions surface as ``ExceptionGroup`` (Python 3.11+).

**Important semantics**: we want "one host failure does not cancel
siblings" (parity with the sync ``HostGroup``). ``TaskGroup``'s
default is the opposite — a raising task cancels the group. We
therefore wrap each per-host coroutine in :func:`_isolate` which
catches and logs, so the ``TaskGroup`` never sees an exception. This
costs us the "first-failure cancels siblings" upside from the plan,
which is the right call for ``connect``/``read_*``/``run`` (we want
every host to be attempted) — the upshot is that commands still get
to inspect each host's outcome via ``AsyncTarget.is_connected`` /
``AsyncTarget.out`` exactly like today's sync ``HostGroup``.
"""

from __future__ import annotations

import asyncio
import logging
from collections import UserDict
from typing import Any, Callable, Coroutine


logger = logging.getLogger("repose.target.async_hostgroup")


async def _isolate(name: str, coro: Coroutine[Any, Any, Any]) -> None:
    """Run ``coro`` and log any exception under ``name``.

    Swallowing keeps a single failing host from cancelling its
    siblings inside an :class:`asyncio.TaskGroup`. The caller
    inspects per-target state (``is_connected``, ``out``) after the
    group completes to discover what failed.
    """
    try:
        await coro
    except Exception as exc:  # noqa: BLE001
        logger.warning("failed for %s: %s", name, exc)


class AsyncHostGroup(UserDict):
    """Dict-of-AsyncTarget with async fan-out helpers.

    Method set mirrors :class:`repose.target.hostgroup.HostGroup`
    one-for-one so commands can be backend-agnostic.
    """

    async def _fanout(
        self,
        op_name: str,
        fn: Callable[[Any], Coroutine[Any, Any, Any]],
    ) -> None:
        """Run ``fn(target)`` for every host inside one ``TaskGroup``.

        ``fn`` returns a coroutine for the per-host work; ``op_name``
        is a short label used in the structured log line on failure.
        Per-host failures are isolated (see module docstring).
        """
        async with asyncio.TaskGroup() as tg:
            for hn, target in self.data.items():
                tg.create_task(
                    _isolate(f"{op_name}:{hn}", fn(target)),
                    name=f"{op_name}:{hn}",
                )

    async def connect(self) -> None:
        await self._fanout("connect", lambda t: t.connect())

    async def close(self) -> None:
        await self._fanout("close", lambda t: t.close())

    async def read_products(self) -> None:
        await self._fanout("read_products", lambda t: t.read_products())

    async def read_repos(self) -> None:
        await self._fanout("read_repos", lambda t: t.read_repos())

    async def parse_repos(self) -> None:
        await self._fanout("parse_repos", lambda t: t.parse_repos())

    async def run(self, cmd: dict[str, str] | str) -> None:
        """Broadcast ``cmd`` (or per-host dict) to every host in parallel."""

        def _per_host(hn: str, target: Any) -> Coroutine[Any, Any, Any]:
            return target.run(cmd[hn] if isinstance(cmd, dict) else cmd)

        async with asyncio.TaskGroup() as tg:
            for hn, target in self.data.items():
                tg.create_task(
                    _isolate(f"run:{hn}", _per_host(hn, target)),
                    name=f"run:{hn}",
                )

    # Reporting stays sync — these touch in-memory state only.
    def report_products(self, sink: Any) -> None:
        for hn in sorted(self.data.keys()):
            self.data[hn].report_products(sink)

    def report_products_yaml(self, sink: Any) -> None:
        for hn in sorted(self.data.keys()):
            self.data[hn].report_products_yaml(sink)

    def report_repos(self, sink: Any) -> None:
        for hn in sorted(self.data.keys()):
            self.data[hn].report_repos(sink)
