import concurrent.futures
import logging
from itertools import chain
from ..utils import blue
from .remove import Remove


logger = logging.getLogger("repose.command.uninstall")


class Uninstall(Remove):
    command = True

    def _calculate_repodict(self, host, patterns):
        rdict = {}
        for pattern in patterns:
            for repo in self.targets[host].repos.items():
                if pattern in repo[0]:
                    if repo[1].name in rdict:
                        rdict[repo[1].name].append(repo[0])
                    else:
                        rdict[repo[1].name] = [repo[0]]
        return rdict

    def _run(self, orepa, host):
        patterns = self._calculate_pattern(orepa, host)
        if not patterns:
            logger.info("For {} no products for remove found".format(host))
            return

        rdict = self._calculate_repodict(host, patterns)
        if not rdict:
            logger.info("For {} no repos for remove found".format(host))
            rrcmd = False
        else:
            rrcmd = self.rrcmd.format(
                repos=" ".join(chain.from_iterable(rdict.values()))
            )

        pdcmd = self.rrpcmd.format(products=" ".join(x.split(":")[0] for x in patterns))

        if self.dryrun:
            if rrcmd:
                print(blue(host) + " - {}".format(rrcmd))
            print(blue(host) + " - {}".format(pdcmd))
        else:
            if rrcmd:
                self.targets[host].run(rrcmd)
                self._report_target(host)
            self.targets[host].run(pdcmd)
            self._report_target(host)

    def run(self):
        self.targets.read_repos()
        self.targets.parse_repos()
        orepa = []

        for r in self.repa:
            r.repo = None
            orepa.append(r)

        with concurrent.futures.ThreadPoolExecutor() as executor:
            targets = [
                executor.submit(self._run, orepa, target)
                for target in self.targets.keys()
            ]
            concurrent.futures.wait(targets)

        self.targets.close()
