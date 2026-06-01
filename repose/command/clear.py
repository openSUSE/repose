import logging

from . import Command, UpdateFn
from ..types import ExitCode


logger = logging.getLogger("repose.command.clear")


class Clear(Command, name="clear"):
    def _clear(self, host):
        return set(r.alias for r in self.targets[host].raw_repos)

    def _run(self, host: str, update: UpdateFn) -> bool:
        update(host, "clearing repos")
        repoaliases = self._clear(host)

        if self.dryrun:
            self.console.dry(host, self.rrcmd.format(repos=" ".join(repoaliases)))
            return True

        self.targets[host].run(self.rrcmd.format(repos=" ".join(repoaliases)))
        logger.info("Repositories cleared from %s", host)
        return True

    def run(self) -> ExitCode:
        self.targets.read_repos()
        futures = self._run_parallel(self._run)
        self.targets.close()
        return self._aggregate(futures)
