from collections import UserDict
import concurrent.futures

from .actions import RunCommand


class HostGroup(UserDict):
    def run(self, cmd) -> None:
        return RunCommand(self.data, cmd).run()

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
                    print(exc)

    def close(self) -> None:
        with concurrent.futures.ThreadPoolExecutor() as executor:
            connections = [
                executor.submit(self.data[hn].close) for hn in self.data.keys()
            ]
            concurrent.futures.wait(connections)

    def read_products(self) -> None:
        with concurrent.futures.ThreadPoolExecutor() as executor:
            connections = [
                executor.submit(self.data[hn].read_products) for hn in self.data.keys()
            ]
            concurrent.futures.wait(connections)

    def read_repos(self) -> None:
        with concurrent.futures.ThreadPoolExecutor() as executor:
            connections = [
                executor.submit(self.data[hn].read_repos) for hn in self.data.keys()
            ]
            concurrent.futures.wait(connections)

    def parse_repos(self) -> None:
        with concurrent.futures.ThreadPoolExecutor() as executor:
            connections = [
                executor.submit(self.data[hn].parse_repos) for hn in self.data.keys()
            ]
            concurrent.futures.wait(connections)

    def report_products(self, sink) -> None:
        for hn in sorted(self.data.keys()):
            self.data[hn].report_products(sink)

    def report_products_yaml(self, sink) -> None:
        for hn in sorted(self.data.keys()):
            self.data[hn].report_products_yaml(sink)

    def report_repos(self, sink) -> None:
        for hn in sorted(self.data.keys()):
            self.data[hn].report_repos(sink)
