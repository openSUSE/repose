import logging

from . import Command
from ..types import ExitCode

logger = logging.getLogger("repose.command.list")


class ListRepos(Command):
    command = True

    def run(self) -> ExitCode:
        self.targets.read_repos()
        self.targets.report_repos(self.display.list_update_repos)
        self.targets.close()
        return 0


class ListProducts(Command):
    command = True

    def run(self) -> ExitCode:
        self.targets.read_products()
        if self.yaml:
            self.targets.report_products_yaml(self.display.list_products_yaml)
        else:
            self.targets.report_products(self.display.list_products)
        self.targets.close()
        return 0
