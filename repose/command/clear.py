
from . import Command
from ..utils import blue
import logging


logger = logging.getLogger("repose.command.clear")


class Clear(Command):
    command = True

    def _clear(self, host):
        return set(r.alias for r in self.targets[host].raw_repos)

    def run(self):
        self.targets.read_repos()

        for host in self.targets.keys():
            repoaliases = self._clear(host)

            if self.dryrun:
                print(blue("host:") + " {} - cmd: {}".format(host, self.rrcmd.format(repos=" ".join(repoaliases))))
            else:
                self.targets[host].run(self.rrcmd.format(repos=" ".join(repoaliases)))
                logger.info("Repositories cleared from {}".format(host))

        self.targets.close()
