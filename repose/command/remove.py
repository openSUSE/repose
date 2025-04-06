import concurrent.futures
import logging

from . import Command
from ..types import ExitCode
from ..utils import blue

logger = logging.getLogger("repose.command.remove")


class Remove(Command):
    command = True

    def _calculate_pattern(self, orepa, host):
        pattern = "{product}:{version}::{repo}"
        products = self.targets[host].products.flatten()
        patterns = set()
        for repa in orepa:
            for prd in products:
                if repa.product:
                    if repa.product == prd.name:
                        product = repa.product
                    else:
                        continue
                else:
                    product = prd.name
                if repa.version:
                    if repa.version == prd.version:
                        version = repa.version
                    else:
                        continue
                else:
                    version = prd.version
                repo = "" if not repa.repo else repa.repo
                patterns.add(
                    pattern.format(product=product, version=version, repo=repo)
                )
        return patterns

    def _calculate_repolist(self, host, patterns):
        repolist = set()
        for pattern in patterns:
            for repo in self.targets[host].repos.keys():
                if pattern in repo:
                    repolist.add(repo)
        return repolist

    def _run(self, host, *args) -> None:
        patterns = self._calculate_pattern(self.repa, host)

        if not patterns:
            logger.info("For %s no repos for remove found", host)
            return
        repolist = self._calculate_repolist(host, patterns)

        if not repolist:
            logger.info("For %s no repos for remove found", host)
        cmd = self.rrcmd.format(repos=" ".join(repolist))

        if self.dryrun:
            print(blue(host) + f" - {cmd}")
        else:
            self.targets[host].run(cmd)
            self._report_target(host)

    def run(self) -> ExitCode:
        self.targets.read_repos()
        self.targets.parse_repos()

        with concurrent.futures.ThreadPoolExecutor() as executor:
            targets = [
                executor.submit(self._run, target) for target in self.targets.keys()
            ]
            concurrent.futures.wait(targets)

        self.targets.close()
        return 0
