from itertools import chain
import logging

from . import Command
from ..types import ExitCode

logger = logging.getLogger("repose.command.install")


class Install(Command, name="install"):
    def _run(self, target) -> bool:
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

        for repo in chain.from_iterable(x for x in (y for y in repositories.values())):
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

    def run(self) -> ExitCode:
        # Materialise the shared ``Repoq`` on the main thread before
        # ``_run_parallel`` spawns workers (see ``Command.repoq``).
        _ = self.repoq
        self.targets.read_products()
        self.targets.read_repos()
        futures = self._run_parallel(self._run)
        self.targets.close()
        return self._aggregate(futures)
