import dataclasses
import logging
import shlex
from itertools import chain
from typing import Any

from . import UpdateFn
from .remove import Remove
from ..target.async_hostgroup import AsyncHostGroup
from ..types import ExitCode


logger = logging.getLogger("repose.command.uninstall")


class Uninstall(Remove, name="uninstall"):
    def _calculate_repodict(
        self, host: str, patterns: set[str]
    ) -> dict[str, list[str]]:
        rdict: dict[str, list[str]] = {}
        for pattern in patterns:
            for alias, product in self.targets[host].repos.items():
                if pattern not in alias:
                    continue
                # ``Repositories`` stores a ``(None, None)`` sentinel for
                # any repo whose name isn't a 4-part product string. Such
                # a repo can't be mapped to a product to uninstall, so
                # skip it instead of dereferencing ``.name`` and crashing.
                name = getattr(product, "name", None)
                if name is None:
                    continue
                rdict.setdefault(name, []).append(alias)
        return rdict

    def _run(self, host: str, update: UpdateFn, *args: Any) -> bool:
        # First positional ``args`` element is the orepa list (see ``run``).
        orepa = args[0]
        update(host, "computing patterns")
        patterns = self._calculate_pattern(orepa, host)
        if not patterns:
            logger.info("For %s no products for remove found", host)
            return True

        rdict = self._calculate_repodict(host, patterns)
        if not rdict:
            logger.info("For %s no repos for remove found", host)
            rrcmd = False
        else:
            rrcmd = self.rrcmd.format(
                repos=shlex.join(chain.from_iterable(rdict.values()))
            )

        # Transactional is a property of the *host* (read-only /usr), not
        # of the product being removed.
        transactional = self.targets[host].products.is_transactional()
        product_names = [x.split(":")[0] for x in patterns]
        products_arg = shlex.join(product_names)
        if transactional:
            pdcmd = self.rrpdtcmd.format(products=products_arg)
        else:
            pdcmd = self.rrpcmd.format(products=products_arg)

        if self.dryrun:
            if rrcmd:
                self.console.dry(host, rrcmd)
            self.console.dry(host, pdcmd)
            if transactional and not self.no_reboot:
                self.console.dry(host, self.reboot)
            return True

        ok = True
        if rrcmd:
            update(host, "removing repos")
            self.targets[host].run(rrcmd)
            if not self._report_target(host):
                ok = False
        update(host, "removing products")
        self.targets[host].run(pdcmd)
        if not self._report_target(host):
            ok = False
        elif transactional:
            if not self._reboot_and_verify(host, product_names, present=False):
                ok = False
        return ok

    async def _arun_one(self, host: str, update: UpdateFn, *args: Any) -> bool:
        orepa = args[0]
        update(host, "computing patterns")
        patterns = self._calculate_pattern(orepa, host)
        if not patterns:
            logger.info("For %s no products for remove found", host)
            return True

        rdict = self._calculate_repodict(host, patterns)
        if not rdict:
            logger.info("For %s no repos for remove found", host)
            rrcmd = False
        else:
            rrcmd = self.rrcmd.format(
                repos=shlex.join(chain.from_iterable(rdict.values()))
            )

        transactional = self.targets[host].products.is_transactional()
        product_names = [x.split(":")[0] for x in patterns]
        products_arg = shlex.join(product_names)
        if transactional:
            pdcmd = self.rrpdtcmd.format(products=products_arg)
        else:
            pdcmd = self.rrpcmd.format(products=products_arg)

        if self.dryrun:
            if rrcmd:
                self.console.dry(host, rrcmd)
            self.console.dry(host, pdcmd)
            if transactional and not self.no_reboot:
                self.console.dry(host, self.reboot)
            return True

        ok = True
        if rrcmd:
            update(host, "removing repos")
            await self.targets[host].run(rrcmd)
            if not self._report_target(host):
                ok = False
        update(host, "removing products")
        await self.targets[host].run(pdcmd)
        if not self._report_target(host):
            ok = False
        elif transactional:
            if not await self._areboot_and_verify(host, product_names, present=False):
                ok = False
        return ok

    def _srun(self) -> ExitCode:
        self.targets.read_repos()
        self.targets.parse_repos()
        orepa = [dataclasses.replace(r, repo=None) for r in self.repa]

        futures = self._run_parallel(self._run, orepa)
        self.targets.close()
        return self._aggregate(futures)

    async def _arun(self) -> ExitCode:
        if not isinstance(self.targets, AsyncHostGroup):
            raise TypeError("_arun requires the asyncssh backend")
        await self.targets.read_repos()
        await self.targets.parse_repos()
        orepa = [dataclasses.replace(r, repo=None) for r in self.repa]

        tasks = await self._arun_parallel(self._arun_one, orepa)
        await self.targets.close()
        return self._aggregate_tasks(tasks)
