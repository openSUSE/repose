from . import Command
from itertools import chain
import logging
from ..utils import blue

logger = logging.getLogger('repose.command.add')


class Add(Command):
    command = True

    def _add(self, target):
        repoq = self._init_repoq()
        repolist = []
        cmds = set()
        for repa in self.repa:
          try:
            repolist += chain.from_iterable(x
                                            for x in repoq.solve_repa(repa,
                                                                      self.targets[target].products.get_base()).values())
          except ValueError as error:
            logger.error(error)
        cmds.update(self.addcmd.format(name=x.name, url=x.url, params="-cfkn" if x.refresh else "-ckn")
                    for x in repolist if self.check_url(x.url))
        return cmds

    def run(self):
        self.targets.read_products()

        for target in self.targets.keys():
            cmds = self._add(target)
            for cmd in cmds:
                if self.dryrun:
                    print("{} - {}".format(blue(target), cmd))
                else:
                    self.targets[target].run(cmd)
                    self._report_target(target)

        if not self.dryrun:
            self.targets.run(self.refcmd)
        self.targets.close()
