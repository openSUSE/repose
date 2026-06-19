from logging import getLogger
from typing import Any
from ..utils import timestamp

from ..connection import Connection, CommandTimeout
from .parsers.product import parse_system
from .parsers.repository import parse_repositories
from ..messages import ConnectingTargetFailedMessage
from ..types.connection_config import ConnectionConfig
from ..types.repositories import Repositories
from ..types.system import System

logger = getLogger("repose.target")


class Target:
    def __init__(
        self,
        hostname: str,
        port: int,
        username: str,
        connector: type[Connection] = Connection,
        *,
        config: ConnectionConfig | None = None,
    ) -> None:
        # TODO: timeout handling ?
        self.port = port
        self.hostname = hostname
        self.username = username
        self.products: System | None = None
        self.raw_repos: Any = None
        self.repos: Repositories | None = None
        self.connector = connector
        self.config: ConnectionConfig = config or ConnectionConfig()
        self.is_connected = False
        # ``connector`` is a class (or test stub) accepting the historical
        # positional ``(hostname, username, port)`` plus the keyword-only
        # ``config``. Test fixtures that pass a plain lambda swallow
        # ``**kwargs`` already.
        self.connection = self.connector(
            self.hostname, self.username, self.port, config=self.config
        )
        self.out: list[list[Any]] = []

    def __repr__(self) -> str:
        return f"<{self.__class__.__name__} object {self.username}@{self.hostname}:{self.port} - connected: {self.is_connected}>"

    def connect(self) -> "Target":
        if not self.is_connected:
            logger.info("Connecting to %s:%s", self.hostname, self.port)
            try:
                self.connection.connect()
            except Exception as e:
                logger.critical(
                    ConnectingTargetFailedMessage(self.hostname, self.port, e)
                )
            else:
                self.is_connected = True

        return self

    def read_products(self) -> None:
        if not self.is_connected:
            self.connect()
        self.products = parse_system(self.connection)

    def reboot(self, command: str, *, retry: int = 10, backoff: bool = True) -> bool:
        """Reboot the host and wait for it to come back.

        Dispatches ``command`` fire-and-forget (the SSH link drops), then
        reconnects with retries/backoff and checks the boot id changed.
        Used after a transactional package operation so the new snapshot
        becomes active.

        Args:
            command: The reboot command to dispatch.
            retry: Maximum reconnect attempts.
            backoff: Grow the wait between attempts exponentially.

        Returns:
            True once the host is reachable again, else False.
        """
        before = self.connection.boot_id()
        logger.info("Rebooting %s:%s", self.hostname, self.port)
        self.connection.fire_and_forget(command)
        self.is_connected = False
        if not self.connection.wait_reconnect(retry=retry, backoff=backoff):
            logger.error(
                "%s:%s did not come back after reboot", self.hostname, self.port
            )
            return False
        self.is_connected = True
        after = self.connection.boot_id()
        if before and after and before == after:
            logger.warning(
                "%s:%s boot id unchanged after reboot", self.hostname, self.port
            )
        return True

    def close(self) -> None:
        self.connection.close()
        self.is_connected = False

    def __bool__(self) -> bool:
        return self.is_connected

    def run(self, command: str, lock: Any = None) -> tuple[str, str, int] | None:
        logger.debug("run %s on %s:%s", command, self.hostname, self.port)
        time_before = timestamp()

        # Pre-initialize so the exception branches below never leave
        # stdout/stderr unbound (would otherwise raise UnboundLocalError
        # at the self.out.append call below).
        stdout, stderr, exitcode = "", "", -1

        try:
            stdout, stderr, exitcode = self.connection.run(command, lock)
        except CommandTimeout:
            logger.critical('%s: command "%s" timed out', self.hostname, command)
        except AssertionError:
            logger.debug("zombie command terminated", exc_info=True)
            return None
        except Exception as e:
            # failed to run command
            logger.error('%s: failed to run command "%s"', self.hostname, command)
            logger.debug("exception %s", e, exc_info=True)

        runtime = int(timestamp()) - int(time_before)

        self.out.append([command, stdout, stderr, exitcode, runtime])
        return (stdout, stderr, exitcode)

    def parse_repos(self) -> None:
        if not self.products:
            self.read_products()
        if not self.raw_repos:
            self.read_repos()
        assert self.products is not None  # narrowed by read_products()
        self.repos = Repositories(self.raw_repos, self.products.arch())

    def read_repos(self) -> None:
        if not self.is_connected:
            logger.debug("Host %s:%s not connected", self.hostname, self.port)
            return

        result = self.run("zypper -x lr")
        if result is None:
            # AssertionError path in run() — treat as transient failure.
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

    def report_products(self, sink: Any) -> Any:
        return sink(self.hostname, self.port, self.products)

    def report_products_yaml(self, sink: Any) -> Any:
        return sink(self.hostname, self.products)

    def report_repos(self, sink: Any) -> Any:
        return sink(self.hostname, self.port, self.raw_repos)
