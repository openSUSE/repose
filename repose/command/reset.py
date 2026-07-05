from itertools import chain
import logging
import shlex

from . import UpdateFn
from ..messages import UnsuportedProductMessage
from ..target.async_hostgroup import AsyncHostGroup
from ..types import ExitCode
from .clear import Clear

logger = logging.getLogger("repose.command.reset")


class Reset(Clear, name="reset"):
    def _add(self, target) -> tuple[set[str], list[str]]:
        """Resolve replacement repos for ``target`` into ``zypper ar`` cmds.

        Returns ``(cmds, dropped)`` where ``dropped`` is the list of
        resolved-candidate repo names whose live-URL probe failed. A
        non-empty ``dropped`` means the probe removed a proper subset of
        the candidates; the caller must treat that as a failure rather
        than silently re-adding only the survivors, which would
        permanently lose the dropped repos on the destructive ``rr``.
        """
        cmds = set()
        repolist = list(
            chain.from_iterable(
                x
                for x in self.repoq.solve_product(
                    self.targets[target].products
                ).values()
            )
        )
        # Probe all candidate URLs in parallel before issuing any
        # ``zypper ar``; otherwise each per-host worker would serialise
        # 1-2 probes per repository.
        live = self._filter_live_urls(repolist)
        live_ids = {id(r) for r in live}
        dropped = [r.name for r in repolist if id(r) not in live_ids]
        cmds.update(
            self.addcmd.format(
                name=shlex.quote(x.name),
                url=shlex.quote(x.url),
                params="-cfkn" if x.refresh else "-ckn",
            )
            for x in live
        )
        return cmds, dropped

    def _run(self, host: str, update: UpdateFn) -> bool:
        update(host, "clearing repos")
        repoaliases = self._clear(host)
        # A host whose current repo list is empty has nothing to clear;
        # issuing the removal anyway would produce a bare ``zypper -n rr``
        # that zypper rejects with a non-zero exit. Skip the ``rr`` step
        # and proceed straight to re-adding the replacement repos.
        if not repoaliases:
            logger.info("No repositories to clear from %s", host)
        ok = True
        try:
            update(host, "resolving new repos")
            cmds, dropped = self._add(host)

            # Guards run BEFORE the dry-run preview so ``--dry`` predicts
            # the real abort instead of showing a destructive plan that
            # never executes.
            if not cmds:
                logger.error(
                    "Refhost %s - no live replacement repositories "
                    "resolved; aborting reset to avoid leaving the host "
                    "without any repositories",
                    host,
                )
                return False

            if dropped:
                logger.error(
                    "Refhost %s - live-URL probe dropped %d of the "
                    "resolved replacement repositories (%s); aborting "
                    "reset to avoid permanently losing repositories over "
                    "a transient mirror failure",
                    host,
                    len(dropped),
                    ", ".join(sorted(dropped)),
                )
                return False

            if self.dryrun:
                if repoaliases:
                    self.console.dry(
                        host, self.rrcmd.format(repos=shlex.join(repoaliases))
                    )
                for cmd in cmds:
                    self.console.dry(host, cmd)
                return True

            update(host, f"re-adding {len(cmds)} repo(s)")
            if repoaliases:
                self.targets[host].run(self.rrcmd.format(repos=shlex.join(repoaliases)))
                if not self._report_target(host):
                    ok = False
            for cmd in cmds:
                self.targets[host].run(cmd)
                if not self._report_target(host):
                    ok = False
        except UnsuportedProductMessage as e:
            logger.error("Refhost %s - %s", host, e)
            ok = False
        return ok

    async def _aadd(self, target) -> tuple[set[str], list[str]]:
        """Async sibling of ``_add`` — uses async URL probing.

        Returns ``(cmds, dropped)`` with the same contract as ``_add``:
        ``dropped`` names the resolved candidates whose live-URL probe
        failed so the caller can refuse a partial reset.
        """
        cmds: set[str] = set()
        repolist = list(
            chain.from_iterable(
                x
                for x in self.repoq.solve_product(
                    self.targets[target].products
                ).values()
            )
        )
        live = await self._afilter_live_urls(repolist)
        live_ids = {id(r) for r in live}
        dropped = [r.name for r in repolist if id(r) not in live_ids]
        cmds.update(
            self.addcmd.format(
                name=shlex.quote(x.name),
                url=shlex.quote(x.url),
                params="-cfkn" if x.refresh else "-ckn",
            )
            for x in live
        )
        return cmds, dropped

    async def _arun_one(self, host: str, update: UpdateFn) -> bool:
        update(host, "clearing repos")
        repoaliases = self._clear(host)
        # Same no-repos guard as ``_run``: never issue a bare ``zypper rr``.
        if not repoaliases:
            logger.info("No repositories to clear from %s", host)
        ok = True
        try:
            update(host, "resolving new repos")
            cmds, dropped = await self._aadd(host)

            # Guards run BEFORE the dry-run preview so ``--dry`` predicts
            # the real abort instead of showing a destructive plan that
            # never executes.
            if not cmds:
                logger.error(
                    "Refhost %s - no live replacement repositories "
                    "resolved; aborting reset to avoid leaving the host "
                    "without any repositories",
                    host,
                )
                return False

            if dropped:
                logger.error(
                    "Refhost %s - live-URL probe dropped %d of the "
                    "resolved replacement repositories (%s); aborting "
                    "reset to avoid permanently losing repositories over "
                    "a transient mirror failure",
                    host,
                    len(dropped),
                    ", ".join(sorted(dropped)),
                )
                return False

            if self.dryrun:
                if repoaliases:
                    self.console.dry(
                        host, self.rrcmd.format(repos=shlex.join(repoaliases))
                    )
                for cmd in cmds:
                    self.console.dry(host, cmd)
                return True

            update(host, f"re-adding {len(cmds)} repo(s)")
            if repoaliases:
                await self.targets[host].run(
                    self.rrcmd.format(repos=shlex.join(repoaliases))
                )
                if not self._report_target(host):
                    ok = False
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
        if not isinstance(self.targets, AsyncHostGroup):
            raise TypeError("_arun requires the asyncssh backend")
        _ = self.repoq
        await self.targets.read_products()
        await self.targets.read_repos()
        tasks = await self._arun_parallel(self._arun_one)
        await self.targets.close()
        return self._aggregate_tasks(tasks)
