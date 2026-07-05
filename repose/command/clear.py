import logging
import shlex

from . import Command, UpdateFn
from ..target.async_hostgroup import AsyncHostGroup
from ..types import ExitCode


logger = logging.getLogger("repose.command.clear")


class Clear(Command, name="clear"):
    def _clear(self, host):
        return set(r.alias for r in self.targets[host].raw_repos)

    def _run(self, host: str, update: UpdateFn) -> bool:
        """Remove every repository from ``host`` via ``zypper rr``.

        A host without any repositories is an INFO-level no-op, not an
        error: issuing the command anyway would produce a bare
        ``zypper -n rr`` that zypper rejects with a non-zero exit.
        """
        update(host, "clearing repos")
        repoaliases = self._clear(host)

        if not repoaliases:
            logger.info("No repositories to clear from %s", host)
            return True

        if self.dryrun:
            self.console.dry(host, self.rrcmd.format(repos=shlex.join(repoaliases)))
            return True

        self.targets[host].run(self.rrcmd.format(repos=shlex.join(repoaliases)))
        logger.info("Repositories cleared from %s", host)
        return True

    async def _arun_one(self, host: str, update: UpdateFn) -> bool:
        """Async sibling of ``_run`` with the same no-repos no-op guard."""
        update(host, "clearing repos")
        repoaliases = self._clear(host)

        if not repoaliases:
            logger.info("No repositories to clear from %s", host)
            return True

        if self.dryrun:
            self.console.dry(host, self.rrcmd.format(repos=shlex.join(repoaliases)))
            return True

        await self.targets[host].run(self.rrcmd.format(repos=shlex.join(repoaliases)))
        logger.info("Repositories cleared from %s", host)
        return True

    def _srun(self) -> ExitCode:
        self.targets.read_repos()
        futures = self._run_parallel(self._run)
        self.targets.close()
        return self._aggregate(futures)

    async def _arun(self) -> ExitCode:
        if not isinstance(self.targets, AsyncHostGroup):
            raise TypeError("_arun requires the asyncssh backend")
        await self.targets.read_repos()
        tasks = await self._arun_parallel(self._arun_one)
        await self.targets.close()
        return self._aggregate_tasks(tasks)
