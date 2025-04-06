import concurrent.futures
from itertools import chain
import logging

from . import Command
from ..types import ExitCode
from ..utils import blue

logger = logging.getLogger("repose.command.add")


class Add(Command):
    command = True

    def _add(self, target):
        repoq = self._init_repoq()
        repolist = []
        cmds = set()
        for repa in self.repa:
            try:
                repolist += chain.from_iterable(
                    x
                    for x in repoq.solve_repa(
                        repa, self.targets[target].products.get_base()
                    ).values()
                )
            except ValueError as error:
                logger.error(error)
        cmds.update(
            self.addcmd.format(
                name=x.name, url=x.url, params="-cfkn" if x.refresh else "-ckn"
            )
            for x in repolist
            if self.check_url(x.url)
        )
        return cmds

    def _run(self, target) -> None:
        cmds = self._add(target)
        for cmd in cmds:
            if self.dryrun:
                print("{} - {}".format(blue(target), cmd))
            else:
                self.targets[target].run(cmd)
                self._report_target(target)

    def run(self) -> ExitCode:
        self.targets.read_products()
        with concurrent.futures.ThreadPoolExecutor() as executor:
            targets = [
                executor.submit(self._run, target) for target in self.targets.keys()
            ]
            concurrent.futures.wait(targets)

        if not self.dryrun:
            self.targets.run(self.refcmd)
        self.targets.close()

        return 0
