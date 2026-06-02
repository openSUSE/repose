from itertools import chain
import logging

from . import Command, UpdateFn
from ..target.async_hostgroup import AsyncHostGroup
from ..types import ExitCode

logger = logging.getLogger("repose.command.install")


class Install(Command, name="install"):
    def _run(self, target: str, update: UpdateFn) -> bool:
        update(target, "resolving repos")
        repositories = {}
        ok = True
        for repa in self.repa:
            try:
                repositories.update(
                    self.repoq.solve_repa(
                        repa, self.targets[target].products.get_base()
                    )
                )
            except ValueError as error:
                logger.error(error)
                ok = False

        # New in PR 08: probe candidate URLs in parallel and skip
        # adding repos whose mirror is dead. The product install below
        # is unchanged - it still iterates ``repositories.keys()`` so a
        # product whose repos all fail the probe is still requested
        # from whatever sources zypper already knows about.
        all_repos = list(chain.from_iterable(repositories.values()))
        live_repos = self._filter_live_urls(all_repos)
        if live_repos:
            update(target, f"adding {len(live_repos)} repo(s)")
        for repo in live_repos:
            addcmd = self.addcmd.format(
                name=repo.name, url=repo.url, params="-cfkn" if repo.refresh else "-ckn"
            )
            if self.dryrun:
                self.console.dry(target, addcmd)
            else:
                self.targets[target].run(addcmd)
                if not self._report_target(target):
                    ok = False
                self.targets[target].run(self.refcmd)

        if repositories.keys():
            transactional = False
            if "SL-Micro" in repositories.keys():
                transactional = True
                inscmd = self.ipdtcmd.format(products=" ".join(repositories.keys()))
            else:
                inscmd = self.ipdcmd.format(products=" ".join(repositories.keys()))
            update(target, "installing products")
            if self.dryrun:
                self.console.dry(str(target), inscmd)
            else:
                self.targets[target].run(inscmd)
                if not self._report_target(target):
                    ok = False
                if transactional:
                    logger.info(
                        "Reboot %s to switch into correct snapshot", str(target)
                    )
        else:
            logger.error("No products to install")
            ok = False
        return ok

    async def _arun_one(self, target: str, update: UpdateFn) -> bool:
        """Async per-host worker — mirror of ``_run`` for asyncssh.

        Identical structure; only the ``targets[target].run(...)``
        calls become ``await``s.
        """
        update(target, "resolving repos")
        repositories: dict = {}
        ok = True
        for repa in self.repa:
            try:
                repositories.update(
                    self.repoq.solve_repa(
                        repa, self.targets[target].products.get_base()
                    )
                )
            except ValueError as error:
                logger.error(error)
                ok = False

        all_repos = list(chain.from_iterable(repositories.values()))
        live_repos = await self._afilter_live_urls(all_repos)
        if live_repos:
            update(target, f"adding {len(live_repos)} repo(s)")
        for repo in live_repos:
            addcmd = self.addcmd.format(
                name=repo.name,
                url=repo.url,
                params="-cfkn" if repo.refresh else "-ckn",
            )
            if self.dryrun:
                self.console.dry(target, addcmd)
            else:
                await self.targets[target].run(addcmd)
                if not self._report_target(target):
                    ok = False
                await self.targets[target].run(self.refcmd)

        if repositories.keys():
            transactional = False
            if "SL-Micro" in repositories.keys():
                transactional = True
                inscmd = self.ipdtcmd.format(products=" ".join(repositories.keys()))
            else:
                inscmd = self.ipdcmd.format(products=" ".join(repositories.keys()))
            update(target, "installing products")
            if self.dryrun:
                self.console.dry(str(target), inscmd)
            else:
                await self.targets[target].run(inscmd)
                if not self._report_target(target):
                    ok = False
                if transactional:
                    logger.info(
                        "Reboot %s to switch into correct snapshot",
                        str(target),
                    )
        else:
            logger.error("No products to install")
            ok = False
        return ok

    def _srun(self) -> ExitCode:
        # Materialise the shared ``Repoq`` on the main thread before
        # ``_run_parallel`` spawns workers (see ``Command.repoq``).
        _ = self.repoq
        self.targets.read_products()
        self.targets.read_repos()
        futures = self._run_parallel(self._run)
        self.targets.close()
        return self._aggregate(futures)

    async def _arun(self) -> ExitCode:
        if not isinstance(self.targets, AsyncHostGroup):
            raise TypeError("_arun requires the asyncssh backend")
        _ = self.repoq
        await self.targets.read_products()
        await self.targets.read_repos()
        tasks = await self._arun_parallel(self._arun_one)
        await self.targets.close()
        return self._aggregate_tasks(tasks)
