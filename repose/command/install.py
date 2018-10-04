import concurrent.futures
from . import Command
from itertools import chain
import logging
from ..utils import blue

logger = logging.getLogger("repose.command.install")


class Install(Command):
    command = True

    def _run(self, repoq, target):
        repositories = {}
        for repa in self.repa:
            try:
                repositories.update(
                    repoq.solve_repa(repa, self.targets[target].products.get_base())
                )
            except ValueError as error:
                logger.error(error)

        for repo in chain.from_iterable(x for x in (y for y in repositories.values())):
            addcmd = self.addcmd.format(
                name=repo.name, url=repo.url, params="-cfkn" if repo.refresh else "-ckn"
            )
            if self.dryrun:
                print(blue("{}".format(target)) + " - {}".format(addcmd))
            else:
                self.targets[target].run(addcmd)
                self._report_target(target)
                self.targets[target].run(self.refcmd)

        if repositories.keys():
            inscmd = self.ipdcmd.format(products=" ".join(repositories.keys()))
            if self.dryrun:
                print(blue(str(target)) + " - {}".format(inscmd))
            else:
                self.targets[target].run(inscmd)
                self._report_target(target)
        else:
            logger.error("No products to install")

    def run(self):
        repoq = self._init_repoq()
        self.targets.read_products()
        self.targets.read_repos()

        with concurrent.futures.ThreadPoolExecutor() as executor:
            targets = [
                executor.submit(self._run, repoq, target)
                for target in self.targets.keys()
            ]
            concurrent.futures.wait(targets)

        self.targets.close()
