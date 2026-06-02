from itertools import chain
import logging

from . import UpdateFn
from ..messages import UnsuportedProductMessage
from ..types import ExitCode
from .clear import Clear

logger = logging.getLogger("repose.command.reset")


class Reset(Clear, name="reset"):
    def _add(self, target):
        cmds = set()
        repolist = chain.from_iterable(
            x for x in self.repoq.solve_product(self.targets[target].products).values()
        )
        # Probe all candidate URLs in parallel before issuing any
        # ``zypper ar``; otherwise each per-host worker would serialise
        # 1-2 probes per repository.
        live = self._filter_live_urls(repolist)
        cmds.update(
            self.addcmd.format(
                name=x.name, url=x.url, params="-cfkn" if x.refresh else "-ckn"
            )
            for x in live
        )
        return cmds

    def _run(self, host: str, update: UpdateFn) -> bool:
        update(host, "clearing repos")
        repoaliases = self._clear(host)
        ok = True
        try:
            update(host, "resolving new repos")
            cmds = self._add(host)

            if self.dryrun:
                self.console.dry(host, self.rrcmd.format(repos=" ".join(repoaliases)))
                for cmd in cmds:
                    self.console.dry(host, cmd)
                return True

            if cmds:
                update(host, f"re-adding {len(cmds)} repo(s)")
            self.targets[host].run(self.rrcmd.format(repos=" ".join(repoaliases)))
            for cmd in cmds:
                self.targets[host].run(cmd)
                if not self._report_target(host):
                    ok = False
        except UnsuportedProductMessage as e:
            logger.error("Refhost %s - %s", host, e)
            ok = False
        return ok

    async def _aadd(self, target) -> set[str]:
        """Async sibling of ``_add`` — uses async URL probing."""
        cmds: set[str] = set()
        repolist = chain.from_iterable(
            x for x in self.repoq.solve_product(self.targets[target].products).values()
        )
        live = await self._afilter_live_urls(repolist)
        cmds.update(
            self.addcmd.format(
                name=x.name,
                url=x.url,
                params="-cfkn" if x.refresh else "-ckn",
            )
            for x in live
        )
        return cmds

    async def _arun_one(self, host: str, update: UpdateFn) -> bool:
        update(host, "clearing repos")
        repoaliases = self._clear(host)
        ok = True
        try:
            update(host, "resolving new repos")
            cmds = await self._aadd(host)

            if self.dryrun:
                self.console.dry(host, self.rrcmd.format(repos=" ".join(repoaliases)))
                for cmd in cmds:
                    self.console.dry(host, cmd)
                return True

            if cmds:
                update(host, f"re-adding {len(cmds)} repo(s)")
            await self.targets[host].run(self.rrcmd.format(repos=" ".join(repoaliases)))
            for cmd in cmds:
                await self.targets[host].run(cmd)
                if not self._report_target(host):
                    ok = False
        except UnsuportedProductMessage as e:
            logger.error("Refhost %s - %s", host, e)
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
        _ = self.repoq
        await self.targets.read_products()
        await self.targets.read_repos()
        tasks = await self._arun_parallel(self._arun_one)
        await self.targets.close()
        return self._aggregate_tasks(tasks)
