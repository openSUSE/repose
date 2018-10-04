import concurrent.futures
import logging
from itertools import chain

from .clear import Clear
from ..utils import blue
from ..messages import UnsuportedProductMessage

logger = logging.getLogger("repose.command.reset")


class Reset(Clear):
    command = True

    def _add(self, target):
        repoq = self._init_repoq()
        cmds = set()
        repolist = chain.from_iterable(
            x for x in repoq.solve_product(self.targets[target].products).values()
        )
        cmds.update(
            self.addcmd.format(
                name=x.name, url=x.url, params="-cfkn" if x.refresh else "-ckn"
            )
            for x in repolist
            if self.check_url(x.url)
        )
        return cmds

    def _run(self, host):
        repoaliases = self._clear(host)
        try:
            cmds = self._add(host)

            if self.dryrun:
                print(
                    blue(host)
                    + " - {}".format(self.rrcmd.format(repos=" ".join(repoaliases)))
                )
                for cmd in cmds:
                    print(blue(host) + " - {}".format(cmd))
            else:
                self.targets[host].run(self.rrcmd.format(repos=" ".join(repoaliases)))
                for cmd in cmds:
                    self.targets[host].run(cmd)
                    self._report_target(host)
        except UnsuportedProductMessage as e:
            logger.error("Refhost {} - {}".format(host, e))

    def run(self):
        self.targets.read_products()
        self.targets.read_repos()

        with concurrent.futures.ThreadPoolExecutor() as executor:
            targets = [
                executor.submit(self._run, target) for target in self.targets.keys()
            ]
            concurrent.futures.wait(targets)
        self.targets.close()
