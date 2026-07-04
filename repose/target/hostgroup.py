from collections import UserDict
import concurrent.futures
import logging
from typing import Any, Callable

logger = logging.getLogger("repose.target.hostgroup")


class HostGroup(UserDict):
    def _fanout(self, op_name: str, fn: Callable[[Any], Any]) -> None:
        """Run ``fn(target)`` for every host and surface per-host failures.

        Each host's work is submitted to a thread pool; results are
        collected via :func:`concurrent.futures.as_completed` and
        ``future.result()`` is called so that a worker exception (e.g.
        an ``OSError`` on a dropped SSH link or a ``ValueError`` while
        parsing repositories) is logged against the offending host
        instead of being silently swallowed. Sibling hosts still run to
        completion.

        Args:
            op_name: Short label for the operation, used in log lines.
            fn: Callable invoked with each host's target object.
        """
        with concurrent.futures.ThreadPoolExecutor() as executor:
            futures = {
                executor.submit(fn, self.data[hn]): hn for hn in self.data.keys()
            }
            for future in concurrent.futures.as_completed(futures):
                hostname = futures[future]
                try:
                    future.result()
                except Exception as exc:
                    logger.warning("%s failed for %s: %s", op_name, hostname, exc)

    def run(self, cmd: dict[str, str] | str) -> None:
        with concurrent.futures.ThreadPoolExecutor() as executor:
            futures = [
                executor.submit(
                    self.data[hn].run,
                    cmd[hn] if isinstance(cmd, dict) else cmd,
                )
                for hn in self.data
            ]
            concurrent.futures.wait(futures)

    def connect(self) -> None:
        with concurrent.futures.ThreadPoolExecutor() as executor:
            connections = {
                executor.submit(self.data[hn].connect): hn for hn in self.data.keys()
            }
            for future in concurrent.futures.as_completed(connections):
                hostname = connections[future]
                try:
                    self.data[hostname] = future.result()
                except Exception as exc:
                    logger.warning("failed to connect %s: %s", hostname, exc)

    def close(self) -> None:
        with concurrent.futures.ThreadPoolExecutor() as executor:
            connections = [
                executor.submit(self.data[hn].close) for hn in self.data.keys()
            ]
            concurrent.futures.wait(connections)

    def read_products(self) -> None:
        self._fanout("read_products", lambda t: t.read_products())

    def read_repos(self) -> None:
        self._fanout("read_repos", lambda t: t.read_repos())

    def parse_repos(self) -> None:
        self._fanout("parse_repos", lambda t: t.parse_repos())

    def report_products(self, sink) -> None:
        for hn in sorted(self.data.keys()):
            self.data[hn].report_products(sink)

    def report_products_yaml(self, sink) -> None:
        for hn in sorted(self.data.keys()):
            self.data[hn].report_products_yaml(sink)

    def report_repos(self, sink) -> None:
        for hn in sorted(self.data.keys()):
            self.data[hn].report_repos(sink)
