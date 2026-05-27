import logging
from itertools import chain
from typing import Any

from ..utils import blue
from .remove import Remove
from ..types import ExitCode


logger = logging.getLogger("repose.command.uninstall")


class Uninstall(Remove, name="uninstall"):
    def _calculate_repodict(
        self, host: str, patterns: set[str]
    ) -> dict[str, list[str]]:
        rdict: dict[str, list[str]] = {}
        for pattern in patterns:
            for repo in self.targets[host].repos.items():
                if pattern in repo[0]:
                    if repo[1].name in rdict:
                        rdict[repo[1].name].append(repo[0])
                    else:
                        rdict[repo[1].name] = [repo[0]]
        return rdict

    def _run(self, host: str, *args: Any) -> bool:
        # First positional ``args`` element is the orepa list (see ``run``).
        orepa = args[0]
        patterns = self._calculate_pattern(orepa, host)
        if not patterns:
            logger.info("For %s no products for remove found", host)
            return True

        rdict = self._calculate_repodict(host, patterns)
        if not rdict:
            logger.info("For %s no repos for remove found", host)
            rrcmd = False
        else:
            rrcmd = self.rrcmd.format(
                repos=" ".join(chain.from_iterable(rdict.values()))
            )

        # Patterns are formatted as "<product>:<version>::<repo>" — detect
        # SL-Micro by matching the product component, not the whole pattern.
        transactional = any(p.split(":", 1)[0] == "SL-Micro" for p in patterns)
        products_arg = " ".join(x.split(":")[0] for x in patterns)
        if transactional:
            pdcmd = self.rrpdtcmd.format(products=products_arg)
        else:
            pdcmd = self.rrpcmd.format(products=products_arg)

        if self.dryrun:
            if rrcmd:
                print(blue(host) + " - {}".format(rrcmd))
            print(blue(host) + " - {}".format(pdcmd))
            return True

        ok = True
        if rrcmd:
            self.targets[host].run(rrcmd)
            if not self._report_target(host):
                ok = False
        self.targets[host].run(pdcmd)
        if not self._report_target(host):
            ok = False
        if transactional:
            logger.info("Reboot %s to switch into updated snapshot", host)
        return ok

    def run(self) -> ExitCode:
        self.targets.read_repos()
        self.targets.parse_repos()
        orepa = []

        for r in self.repa:
            r.repo = None
            orepa.append(r)

        futures = self._run_parallel(self._run, orepa)
        self.targets.close()
        return self._aggregate(futures)
