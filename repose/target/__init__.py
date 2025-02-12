from logging import getLogger
from ..utils import timestamp

from ..connection import Connection, CommandTimeout
from .parsers.product import parse_system
from .parsers.repository import parse_repositories
from ..messages import ConnectingTargetFailedMessage
from ..types.repositories import Repositories

logger = getLogger("repose.target")


class Target:
    def __init__(self, hostname, port, username, connector=Connection):
        # TODO: timeout handling ?
        self.port = port
        self.hostname = hostname
        self.username = username
        self.products = None
        self.raw_repos = None
        self.repos = None
        self.connector = connector
        self.is_connected = False
        self.connection = self.connector(self.hostname, self.username, self.port)
        self.out = []

    def __repr__(self):
        return "<{} object {}@{}:{} - connected: {}>".format(
            self.__class__.__name__,
            self.username,
            self.hostname,
            self.port,
            self.is_connected,
        )

    def connect(self):
        if not self.is_connected:
            logger.info("Connecting to {}:{}".format(self.hostname, self.port))
            try:
                self.connection.connect()
            except BaseException as e:
                logger.critical(
                    ConnectingTargetFailedMessage(self.hostname, self.port, e)
                )
            else:
                self.is_connected = True

        return self

    def read_products(self):
        if not self.is_connected:
            self.connect()
        self.products = parse_system(self.connection)

    def close(self):
        self.connection.close()
        self.is_connected = False

    def __bool__(self):
        return self.is_connected

    def run(self, command, lock=None):
        logger.debug("run {} on {}:{}".format(command, self.hostname, self.port))
        time_before = timestamp()

        try:
            stdout, stderr, exitcode = self.connection.run(command, lock)
        except CommandTimeout:
            logger.critical('{}: command "{}" timed out'.format(self.hostname, command))
            exitcode = -1
        except AssertionError:
            logger.debug("zombie command terminated", exc_info=True)
            return
        except Exception as e:
            # failed to run command
            logger.error(
                '{}: failed to run command "{}"'.format(self.hostname, command)
            )
            logger.debug("exception {}".format(e), exc_info=True)
            exitcode = -1

        runtime = int(timestamp()) - int(time_before)

        self.out.append([command, stdout, stderr, exitcode, runtime])
        return (stdout, stderr, exitcode)

    def parse_repos(self):
        if not self.products:
            self.read_products()
        if not self.raw_repos:
            self.read_repos()
        self.repos = Repositories(self.raw_repos, self.products.arch())

    def read_repos(self):
        if self.is_connected:
            stdout, stderr, exitcode = self.run("zypper -x lr")

            if exitcode in (0, 106, 6):
                self.raw_repos = parse_repositories(stdout)
            else:
                logger.error(
                    "Can't parse repositories on {}, zypper returned {} exitcode".format(
                        self.hostname, exitcode
                    )
                )
                logger.debug("output:\n {}".format(stderr))
                raise ValueError(
                    "Can't read repositories on {}:{}".format(self.hostname, self.port)
                )
        else:
            logger.debug("Host {}:{} not connected".format(self.hostname, self.port))

    def report_products(self, sink):
        return sink(self.hostname, self.port, self.products)

    def report_products_yaml(self, sink):
        return sink(self.hostname, self.products)

    def report_repos(self, sink):
        return sink(self.hostname, self.port, self.raw_repos)
