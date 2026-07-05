import logging
import shlex
from typing import Any, Iterable

from . import Command, UpdateFn
from ..target.async_hostgroup import AsyncHostGroup
from ..types import ExitCode
from ..types.repa import Repa

logger = logging.getLogger("repose.command.remove")


class Remove(Command, name="remove"):
    def _calculate_pattern(self, orepa: Iterable[Repa], host: str) -> set[str]:
        pattern = "{product}:{version}::{repo}"
        products = self.targets[host].products.flatten()
        patterns: set[str] = set()
        for repa in orepa:
            for prd in products:
                if repa.product:
                    if repa.product == prd.name:
                        product = repa.product
                    else:
                        continue
                else:
                    product = prd.name
                if repa.version:
                    if repa.version == prd.version:
                        version = repa.version
                    else:
                        continue
                else:
                    version = prd.version
                repo = "" if not repa.repo else repa.repo
                patterns.add(
                    pattern.format(product=product, version=version, repo=repo)
                )
        return patterns

    def _calculate_repolist(self, host: str, patterns: set[str]) -> set[str]:
        """Map REPA patterns to concrete repo aliases on ``host``.

        A pattern ending in ``::`` carries no repo component, i.e. the
        operator asked to remove *all* repos for a given
        ``product:version``; such patterns match by prefix. A pattern
        naming a specific repo alias must match that alias exactly, so
        that removing ``repo1`` never also deletes ``repo10`` or
        ``repo1-debuginfo``.
        """
        repolist: set[str] = set()
        for pattern in patterns:
            all_repos = pattern.endswith("::")
            for repo in self.targets[host].repos.keys():
                matched = pattern in repo if all_repos else repo == pattern
                if matched:
                    repolist.add(repo)
        return repolist

    def _run(self, host: str, update: UpdateFn, *args: Any) -> bool:
        """Compute and (optionally) issue the ``zypper rr`` command.

        Returns ``True`` when no work was found (an INFO-level no-op,
        not an error) and ``True``/``False`` from ``_report_target``
        when the command actually ran.
        """
        update(host, "computing patterns")
        patterns = self._calculate_pattern(self.repa, host)

        if not patterns:
            logger.info("For %s no repos for remove found", host)
            return True
        repolist = self._calculate_repolist(host, patterns)

        if not repolist:
            logger.info("For %s no repos for remove found", host)
            return True
        cmd = self.rrcmd.format(repos=shlex.join(repolist))

        if self.dryrun:
            self.console.dry(host, cmd)
            return True

        update(host, f"removing {len(repolist)} repo(s)")
        self.targets[host].run(cmd)
        return self._report_target(host)

    async def _arun_one(self, host: str, update: UpdateFn, *args: Any) -> bool:
        update(host, "computing patterns")
        patterns = self._calculate_pattern(self.repa, host)

        if not patterns:
            logger.info("For %s no repos for remove found", host)
            return True
        repolist = self._calculate_repolist(host, patterns)

        if not repolist:
            logger.info("For %s no repos for remove found", host)
            return True
        cmd = self.rrcmd.format(repos=shlex.join(repolist))

        if self.dryrun:
            self.console.dry(host, cmd)
            return True

        update(host, f"removing {len(repolist)} repo(s)")
        await self.targets[host].run(cmd)
        return self._report_target(host)

    def _srun(self) -> ExitCode:
        self.targets.read_repos()
        self.targets.parse_repos()
        futures = self._run_parallel(self._run)
        self.targets.close()
        return self._aggregate(futures)

    async def _arun(self) -> ExitCode:
        if not isinstance(self.targets, AsyncHostGroup):
            raise TypeError("_arun requires the asyncssh backend")
        await self.targets.read_repos()
        await self.targets.parse_repos()
        tasks = await self._arun_parallel(self._arun_one)
        await self.targets.close()
        return self._aggregate_tasks(tasks)
