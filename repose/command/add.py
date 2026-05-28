from itertools import chain
import logging

from . import Command
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
        cmds.update(
            self.addcmd.format(
                name=x.name, url=x.url, params="-cfkn" if x.refresh else "-ckn"
            )
            for x in repolist
            if self.check_url(x.url)
        )
        return cmds, ok

    def _run(self, target) -> bool:
        cmds, ok = self._add(target)
        for cmd in cmds:
            if self.dryrun:
                self.console.dry(target, cmd)
            else:
                self.targets[target].run(cmd)
                if not self._report_target(target):
                    ok = False
        return ok

    def run(self) -> ExitCode:
        # Materialise the shared ``Repoq`` on the main thread before
        # ``_run_parallel`` spawns workers (see ``Command.repoq``).
        _ = self.repoq
        self.targets.read_products()
        futures = self._run_parallel(self._run)

        if not self.dryrun:
            self.targets.run(self.refcmd)
        self.targets.close()

        return self._aggregate(futures)
