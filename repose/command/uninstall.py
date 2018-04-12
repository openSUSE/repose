
import logging
from itertools import chain
from ..utils import blue
from .remove import Remove


logger = logging.getLogger('repose.command.uninstall')


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

    def run(self):

        self.targets.read_repos()
        self.targets.parse_repos()
        orepa = []
        for r in self.repa:
            r.repo = None
            orepa.append(r)

        for host in self.targets.keys():

            patterns = self._calculate_pattern(orepa, host)

            if not patterns:
                logger.info("For {} no repos for remove found".format(host))
                continue

            rdict = self._calculate_repodict(host, patterns)
            if not rdict:
                logger.info("For {} no repos for remove found".format(host))
                continue

            pdcmd = self.rrpcmd.format(products=" ".join(rdict.keys()))
            rrcmd = self.rrcmd.format(repos=" ".join(chain.from_iterable(rdict.values())))

            if self.dryrun:
                print(blue(host) + " - {}".format(rrcmd))
                print(blue(host) + " - {}".format(pdcmd))
            else:
                self.targets[host].run(rrcmd)
                self._report_target(host)
                self.targets[host].run(pdcmd)
                self._report_target(host)

        self.targets.close()
