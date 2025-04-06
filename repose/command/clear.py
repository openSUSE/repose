import concurrent.futures
import logging

from . import Command
from ..types import ExitCode
from ..utils import blue


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
            logger.info("Repositories cleared from %s", host)

    def run(self) -> ExitCode:
        self.targets.read_repos()

        with concurrent.futures.ThreadPoolExecutor() as executor:
            targets = [
                executor.submit(self._run, target) for target in self.targets.keys()
            ]
            concurrent.futures.wait(targets)

        self.targets.close()
        return 0
