import concurrent.futures
from . import Command
from ..utils import blue
import logging


logger = logging.getLogger("repose.command.clear")


class Clear(Command):
    command = True

    def _clear(self, host):
        return set(r.alias for r in self.targets[host].raw_repos)

    def _run(self, host):
        repoaliases = self._clear(host)

        if self.dryrun:
            print(
                blue("host:")
                + " {} - cmd: {}".format(
                    host, self.rrcmd.format(repos=" ".join(repoaliases))
                )
            )
        else:
            self.targets[host].run(self.rrcmd.format(repos=" ".join(repoaliases)))
            logger.info("Repositories cleared from {}".format(host))

    def run(self):
        self.targets.read_repos()

        with concurrent.futures.ThreadPoolExecutor() as executor:
            targets = [
                executor.submit(self._run, target) for target in self.targets.keys()
            ]
            concurrent.futures.wait(targets)

        self.targets.close()
