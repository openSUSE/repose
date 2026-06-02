import logging

from . import Command
from ..target.async_hostgroup import AsyncHostGroup
from ..types import ExitCode

logger = logging.getLogger("repose.command.list")


class ListRepos(Command, name="list-repos"):
    def _srun(self) -> ExitCode:
        self.targets.read_repos()
        self.targets.report_repos(self.display.list_update_repos)
        self.targets.close()
        return 0

    async def _arun(self) -> ExitCode:
        if not isinstance(self.targets, AsyncHostGroup):
            raise TypeError("_arun requires the asyncssh backend")
        await self.targets.read_repos()
        # ``report_repos`` stays sync on both backends (touches
        # in-memory state only).
        self.targets.report_repos(self.display.list_update_repos)
        await self.targets.close()
        return 0


class ListProducts(Command, name="list-products"):
    def _srun(self) -> ExitCode:
        self.targets.read_products()
        if self.yaml:
            self.targets.report_products_yaml(self.display.list_products_yaml)
        else:
            self.targets.report_products(self.display.list_products)
        self.targets.close()
        return 0

    async def _arun(self) -> ExitCode:
        if not isinstance(self.targets, AsyncHostGroup):
            raise TypeError("_arun requires the asyncssh backend")
        await self.targets.read_products()
        if self.yaml:
            self.targets.report_products_yaml(self.display.list_products_yaml)
        else:
            self.targets.report_products(self.display.list_products)
        await self.targets.close()
        return 0
