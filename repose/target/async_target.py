"""Async equivalent of :class:`repose.target.Target` backed by ``AsyncConnection``.

Public surface mirrors the sync ``Target`` exactly: same attributes
(``products``, ``raw_repos``, ``repos``, ``out``, ``is_connected``),
same per-command output shape (``[command, stdout, stderr, exitcode,
runtime]``), and the same report helpers
(``report_products``/``report_products_yaml``/``report_repos``) so the
upstream ``HostGroup.report_*`` iteration loops work unchanged on a
mixed group.

The only behaviour difference is that the I/O entry points
(``connect``, ``read_products``, ``read_repos``, ``run``,
``parse_repos``, ``close``) are coroutines.
"""

from __future__ import annotations

from logging import getLogger
from typing import Any

from ..aiossh import AsyncConnection, CommandTimeout
from ..messages import ConnectingTargetFailedMessage
from ..types.connection_config import ConnectionConfig
from ..types.repositories import Repositories
from ..types.system import System
from ..utils import timestamp
from .parsers.product import parse_system_async
from .parsers.repository import parse_repositories


logger = getLogger("repose.target.async_target")


class AsyncTarget:
    def __init__(
        self,
        hostname: str,
        port: int,
        username: str,
        connector: type[AsyncConnection] = AsyncConnection,
        *,
        config: ConnectionConfig | None = None,
    ) -> None:
        self.port = port
        self.hostname = hostname
        self.username = username
        self.products: System | None = None
        self.raw_repos: Any = None
        self.repos: Repositories | None = None
        self.connector = connector
        self.config: ConnectionConfig = config or ConnectionConfig()
        self.is_connected = False
        # ``connector`` is a class (or test stub) accepting the same
        # signature as ``AsyncConnection`` — fixtures that pass a
        # lambda swallow ``**kwargs`` already.
        self.connection: AsyncConnection = self.connector(
            self.hostname, self.username, self.port, config=self.config
        )
        # ``out`` keeps per-command tuples for ``_report_target`` parity
        # with the sync backend (Command._report_target inspects
        # ``out[-1]`` regardless of which Target shape it received).
        self.out: list[list[Any]] = []

    def __repr__(self) -> str:
        return (
            f"<{self.__class__.__name__} object "
            f"{self.username}@{self.hostname}:{self.port} - "
            f"connected: {self.is_connected}>"
        )

    async def connect(self) -> "AsyncTarget":
        if not self.is_connected:
            logger.info("Connecting to %s:%s", self.hostname, self.port)
            try:
                await self.connection.connect()
            except Exception as e:  # noqa: BLE001
                # Mirror sync ``Target.connect`` — log critically and
                # leave ``is_connected`` False so downstream methods
                # short-circuit instead of raising.
                logger.critical(
                    ConnectingTargetFailedMessage(self.hostname, self.port, e)
                )
            else:
                self.is_connected = True
        return self

    async def read_products(self) -> None:
        if not self.is_connected:
            await self.connect()
        self.products = await parse_system_async(self.connection)

    async def reboot(
        self, command: str, *, retry: int = 10, backoff: bool = True
    ) -> bool:
        """Async mirror of :meth:`repose.target.Target.reboot`."""
        before = await self.connection.boot_id()
        logger.info("Rebooting %s:%s", self.hostname, self.port)
        await self.connection.fire_and_forget(command)
        self.is_connected = False
        if not await self.connection.wait_reconnect(retry=retry, backoff=backoff):
            logger.error(
                "%s:%s did not come back after reboot", self.hostname, self.port
            )
            return False
        self.is_connected = True
        after = await self.connection.boot_id()
        if before and after and before == after:
            logger.warning(
                "%s:%s boot id unchanged after reboot", self.hostname, self.port
            )
        return True

    async def close(self) -> None:
        await self.connection.close()
        self.is_connected = False

    def __bool__(self) -> bool:
        return self.is_connected

    async def run(self, command: str, lock: Any = None) -> tuple[str, str, int] | None:
        logger.debug("run %s on %s:%s", command, self.hostname, self.port)
        time_before = timestamp()

        # Pre-initialise mirrors the sync Target — the exception
        # branches below must never leave stdout/stderr/exitcode
        # unbound (would raise UnboundLocalError at ``out.append``).
        stdout, stderr, exitcode = "", "", -1

        try:
            stdout, stderr, exitcode = await self.connection.run(command, lock)
        except CommandTimeout:
            logger.critical('%s: command "%s" timed out', self.hostname, command)
        except AssertionError:
            logger.debug("zombie command terminated", exc_info=True)
            return None
        except Exception as e:  # noqa: BLE001
            logger.error('%s: failed to run command "%s"', self.hostname, command)
            logger.debug("exception %s", e, exc_info=True)

        runtime = int(timestamp()) - int(time_before)

        self.out.append([command, stdout, stderr, exitcode, runtime])
        return (stdout, stderr, exitcode)

    async def parse_repos(self) -> None:
        if not self.products:
            await self.read_products()
        if not self.raw_repos:
            await self.read_repos()
        assert self.products is not None  # narrowed by read_products()
        self.repos = Repositories(self.raw_repos, self.products.arch())

    async def read_repos(self) -> None:
        if not self.is_connected:
            logger.debug("Host %s:%s not connected", self.hostname, self.port)
            return

        result = await self.run("zypper -x lr")
        if result is None:
            raise ValueError(f"Can't read repositories on {self.hostname}:{self.port}")
        stdout, stderr, exitcode = result

        if exitcode in (0, 106, 6):
            self.raw_repos = parse_repositories(stdout)
        else:
            logger.error(
                "Can't parse repositories on %s, zypper returned %s exitcode",
                self.hostname,
                exitcode,
            )
            logger.debug("output:\n %s", stderr)
            raise ValueError(f"Can't read repositories on {self.hostname}:{self.port}")

    # Report helpers stay sync; they touch in-memory state only.
    def report_products(self, sink: Any) -> Any:
        return sink(self.hostname, self.port, self.products)

    def report_products_yaml(self, sink: Any) -> Any:
        return sink(self.hostname, self.products)

    def report_repos(self, sink: Any) -> Any:
        return sink(self.hostname, self.port, self.raw_repos)
