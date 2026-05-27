from itertools import chain
import logging

from . import Command
from ..types import ExitCode
from ..utils import blue

logger = logging.getLogger("repose.command.install")


class Install(Command, name="install"):
    def _run(self, target, repoq) -> bool:
        repositories = {}
        ok = True
        for repa in self.repa:
            try:
                repositories.update(
                    repoq.solve_repa(repa, self.targets[target].products.get_base())
                )
            except ValueError as error:
                logger.error(error)
                ok = False

        for repo in chain.from_iterable(x for x in (y for y in repositories.values())):
            addcmd = self.addcmd.format(
                name=repo.name, url=repo.url, params="-cfkn" if repo.refresh else "-ckn"
            )
            if self.dryrun:
                print(blue(f"{target}") + f" - {addcmd}")
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
                print(blue(str(target)) + f" - {inscmd}")
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
        repoq = self._init_repoq()
        self.targets.read_products()
        self.targets.read_repos()
        futures = self._run_parallel(self._run, repoq)
        self.targets.close()
        return self._aggregate(futures)
