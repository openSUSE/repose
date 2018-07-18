

from collections import UserDict
from .actions import RunCommand


class HostGroup(UserDict):
    def run(self, cmd):
        return RunCommand(self.data, cmd).run()

    def connect(self):
        for hn in self.data.keys():
            self.data[hn].connect()

    def close(self):
        for hn in self.data.keys():
            self.data[hn].close()

    def read_products(self):
        for hn in self.data.keys():
            self.data[hn].read_products()

    def read_repos(self):
        for hn in self.data.keys():
            self.data[hn].read_repos()

    def parse_repos(self):
        for hn in self.data.keys():
            self.data[hn].parse_repos()

    def report_products(self, sink):
        for hn in sorted(self.data.keys()):
            self.data[hn].report_products(sink)

    def report_products_yaml(self, sink):
        for hn in sorted(self.data.keys()):
            self.data[hn].report_products_yaml(sink)

    def report_repos(self, sink):
        for hn in sorted(self.data.keys()):
            self.data[hn].report_repos(sink)
