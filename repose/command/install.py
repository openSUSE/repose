from itertools import chain
import logging
import shlex

from . import Command, UpdateFn
from ..target.async_hostgroup import AsyncHostGroup
from ..template.resolver import Repos
from ..types import ExitCode

logger = logging.getLogger("repose.command.install")


def _merge_repos(
    repositories: dict[str, list[Repos]], resolved: dict[str, list[Repos]]
) -> None:
    """Merge a resolved REPA solution into the accumulator in place.

    Two REPAs may name the same product (e.g. differing only in the
    requested repo). A plain ``dict.update`` would overwrite the first
    product's repo list with the second's, silently dropping the repos
    from the earlier REPA. Instead, extend each product's list with any
    repos not already present so every resolved repo is retained.
    """
    for product, repos in resolved.items():
        existing = repositories.setdefault(product, [])
        for repo in repos:
            if repo not in existing:
                existing.append(repo)


class Install(Command, name="install"):
    def _run(self, target: str, update: UpdateFn) -> bool:
        update(target, "resolving repos")
        repositories: dict[str, list[Repos]] = {}
        ok = True
        for repa in self.repa:
            try:
                _merge_repos(
                    repositories,
                    self.repoq.solve_repa(
                        repa, self.targets[target].products.get_base()
                    ),
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
                name=shlex.quote(repo.name),
                url=shlex.quote(repo.url),
                params="-cfkn" if repo.refresh else "-ckn",
            )
            if self.dryrun:
                self.console.dry(target, addcmd)
            else:
                self.targets[target].run(addcmd)
                if not self._report_target(target):
                    ok = False
                self.targets[target].run(self.refcmd)

        if repositories.keys():
            # Transactional is a property of the *host* (read-only /usr),
            # not of the product being installed: any product on a
            # transactional host must go through transactional-update.
            transactional = self.targets[target].products.is_transactional()
            if transactional:
                inscmd = self.ipdtcmd.format(products=shlex.join(repositories.keys()))
            else:
                inscmd = self.ipdcmd.format(products=shlex.join(repositories.keys()))
            update(target, "installing products")
            if self.dryrun:
                if transactional:
                    self.console.dry(str(target), self.reftcmd)
                self.console.dry(str(target), inscmd)
                if transactional and not self.no_reboot:
                    self.console.dry(str(target), self.reboot)
            else:
                # Import repo keys into the snapshot keyring first, else the
                # inner zypper of the transactional install rejects the repo
                # signature (see reftcmd).
                if transactional:
                    self.targets[target].run(self.reftcmd)
                self.targets[target].run(inscmd)
                if not self._report_target(target):
                    ok = False
                elif transactional:
                    if not self._reboot_and_verify(target, list(repositories.keys())):
                        ok = False
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
        repositories: dict[str, list[Repos]] = {}
        ok = True
        for repa in self.repa:
            try:
                _merge_repos(
                    repositories,
                    self.repoq.solve_repa(
                        repa, self.targets[target].products.get_base()
                    ),
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
                name=shlex.quote(repo.name),
                url=shlex.quote(repo.url),
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
            # Host property, not product property (see sync ``_run``).
            transactional = self.targets[target].products.is_transactional()
            if transactional:
                inscmd = self.ipdtcmd.format(products=shlex.join(repositories.keys()))
            else:
                inscmd = self.ipdcmd.format(products=shlex.join(repositories.keys()))
            update(target, "installing products")
            if self.dryrun:
                if transactional:
                    self.console.dry(str(target), self.reftcmd)
                self.console.dry(str(target), inscmd)
                if transactional and not self.no_reboot:
                    self.console.dry(str(target), self.reboot)
            else:
                # Import repo keys into the snapshot keyring first (see reftcmd).
                if transactional:
                    await self.targets[target].run(self.reftcmd)
                await self.targets[target].run(inscmd)
                if not self._report_target(target):
                    ok = False
                elif transactional:
                    if not await self._areboot_and_verify(
                        target, list(repositories.keys())
                    ):
                        ok = False
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
