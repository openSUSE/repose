from itertools import chain
import logging
import shlex

from . import Command, UpdateFn
from ..target.async_hostgroup import AsyncHostGroup
from ..types import ExitCode

logger = logging.getLogger("repose.command.add")


class Add(Command, name="add"):
    def _add(self, target) -> tuple[set[str], bool]:
        """Resolve REPA patterns for ``target`` into ``zypper ar`` commands.

        Returns ``(cmds, ok)`` where ``ok`` is ``False`` if any REPA in
        ``self.repa`` failed to resolve (caught ``ValueError`` from
        ``Repoq.solve_repa``). The caller uses ``ok`` to mark the host
        as failed in the aggregated exit code.
        """
        repolist = []
        cmds = set()
        ok = True
        for repa in self.repa:
            try:
                repolist += chain.from_iterable(
                    x
                    for x in self.repoq.solve_repa(
                        repa, self.targets[target].products.get_base()
                    ).values()
                )
            except ValueError as error:
                logger.error(error)
                ok = False
        # Probe all candidate URLs in parallel before issuing any
        # ``zypper ar`` so a slow mirror doesn't serialise the cohort.
        live = self._filter_live_urls(repolist)
        cmds.update(
            self.addcmd.format(
                name=shlex.quote(x.name),
                url=shlex.quote(x.url),
                params="-cfkn" if x.refresh else "-ckn",
            )
            for x in live
        )
        return cmds, ok

    def _run(self, target: str, update: UpdateFn) -> bool:
        update(target, "resolving repos")
        cmds, ok = self._add(target)
        if cmds:
            update(target, f"adding {len(cmds)} repo(s)")
        for cmd in cmds:
            if self.dryrun:
                self.console.dry(target, cmd)
            else:
                self.targets[target].run(cmd)
                if not self._report_target(target):
                    ok = False
        return ok

    async def _aadd(self, target) -> tuple[set[str], bool]:
        """Async sibling of ``_add`` — uses async URL probing.

        Identical to ``_add`` except for ``_afilter_live_urls`` in
        place of the threaded ``_filter_live_urls``. Kept in sync
        manually; the test suite covers both code paths.
        """
        from itertools import chain

        repolist = []
        cmds: set[str] = set()
        ok = True
        for repa in self.repa:
            try:
                repolist += chain.from_iterable(
                    x
                    for x in self.repoq.solve_repa(
                        repa, self.targets[target].products.get_base()
                    ).values()
                )
            except ValueError as error:
                logger.error(error)
                ok = False
        live = await self._afilter_live_urls(repolist)
        cmds.update(
            self.addcmd.format(
                name=shlex.quote(x.name),
                url=shlex.quote(x.url),
                params="-cfkn" if x.refresh else "-ckn",
            )
            for x in live
        )
        return cmds, ok

    async def _arun_one(self, target: str, update: UpdateFn) -> bool:
        """Async per-host worker — mirror of ``_run`` for asyncssh.

        Structurally identical to ``_run``; the differences are the
        ``await``s on the SSH calls and the async URL probing inside
        ``_aadd``. ``_report_target`` stays sync because it touches
        in-memory state only.
        """
        update(target, "resolving repos")
        cmds, ok = await self._aadd(target)
        if cmds:
            update(target, f"adding {len(cmds)} repo(s)")
        for cmd in cmds:
            if self.dryrun:
                self.console.dry(target, cmd)
            else:
                await self.targets[target].run(cmd)
                if not self._report_target(target):
                    ok = False
        return ok

    def _srun(self) -> ExitCode:
        # Materialise the shared ``Repoq`` on the main thread before
        # ``_run_parallel`` spawns workers (see ``Command.repoq``).
        _ = self.repoq
        self.targets.read_products()
        futures = self._run_parallel(self._run)

        if not self.dryrun:
            self.targets.run(self.refcmd)
        self.targets.close()

        return self._aggregate(futures)

    async def _arun(self) -> ExitCode:
        # Same orchestration as ``_srun``, but every fan-out is an
        # ``await`` on the AsyncHostGroup. Repoq is still materialised
        # up-front so workers all observe the same instance.
        if not isinstance(self.targets, AsyncHostGroup):
            raise TypeError("_arun requires the asyncssh backend")
        _ = self.repoq
        await self.targets.read_products()
        tasks = await self._arun_parallel(self._arun_one)

        if not self.dryrun:
            await self.targets.run(self.refcmd)
        await self.targets.close()

        return self._aggregate_tasks(tasks)
