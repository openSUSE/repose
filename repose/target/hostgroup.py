
import concurrent.futures

from collections import UserDict
from .actions import RunCommand


class HostGroup(UserDict):
    def run(self, cmd):
        return RunCommand(self.data, cmd).run()

    def connect(self):
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

    def close(self):
        for hn in self.data.keys():
            self.data[hn].close()

    def read_products(self):
        with concurrent.futures.ThreadPoolExecutor() as executor:
            connections = [
                executor.submit(self.data[hn].read_products) for hn in self.data.keys()
            ]
            concurrent.futures.wait(connections)

    def read_repos(self):
        with concurrent.futures.ThreadPoolExecutor() as executor:
            connections = [
                executor.submit(self.data[hn].read_repos) for hn in self.data.keys()
            ]
            concurrent.futures.wait(connections)

    def parse_repos(self):
        with concurrent.futures.ThreadPoolExecutor() as executor:
            connections = [
                executor.submit(self.data[hn].parse_repos) for hn in self.data.keys()
            ]
            concurrent.futures.wait(connections)

    def report_products(self, sink):
        for hn in sorted(self.data.keys()):
            self.data[hn].report_products(sink)

    def report_products_yaml(self, sink):
        for hn in sorted(self.data.keys()):
            self.data[hn].report_products_yaml(sink)

    def report_repos(self, sink):
        for hn in sorted(self.data.keys()):
            self.data[hn].report_repos(sink)
