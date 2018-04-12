
import logging
from . import Command

logger = logging.getLogger('repose.command.list')


class ListRepos(Command):
    command = True

    def run(self):

        self.targets.read_repos()
        self.targets.report_repos(self.display.list_update_repos)
        self.targets.close()


class ListProducts(Command):
    command = True

    def run(self):

        self.targets.read_products()
        self.targets.report_products(self.display.list_products)
        self.targets.close()
