from abc import ABC, abstractmethod
from argparse import Namespace
import concurrent.futures
from concurrent.futures import Future
import logging
import sys
from pathlib import Path
from typing import Any, Callable
from urllib.error import HTTPError, URLError
from urllib.request import urlopen

from ..display import CommandDisplay
from ..target.hostgroup import HostGroup
from ..template import load_template
from ..template.resolver import Repoq
from ..types import ExitCode
from ..types.repa import Repa
from ..utils import blue

logger = logging.getLogger("repose.command")


class Command(ABC):
    addcmd: str = "zypper -n ar {params} {name} {url} {name}"
    rrcmd: str = "zypper -n rr {repos}"
    refcmd: str = "zypper -n --gpg-auto-import-keys ref -f"
    ipdcmd: str = "zypper -n in -t product -l -f {products}"
    rrpcmd: str = "zypper -n rm -t product {products}"
    ipdtcmd: str = "transactional-update pkg in -t product -l -f {products}"
    rrpdtcmd: str = "transactional-update pkg rm -t product -l -f {products}"
    reboot: str = "rebootmgrctl reboot now"

    def __init__(self, args: Namespace) -> None:
        __dtargets: dict = {}

        if "target" in args:
            for x in args.target:
                __dtargets.update(x)

        targets = HostGroup(__dtargets)
        targets.connect()

        # cann't  use dict comprehension - custom dict for hostgroup:(
        for target in list(targets.keys()):
            if not targets[target]:
                del targets[target]
        self.targets = targets

        self.dryrun: bool = args.dry
        self.template_path: str = args.config
        self.display = CommandDisplay(sys.stdout)
        # ``repa`` is None for commands that don't accept a REPA argument
        # (list, known, clear, reset). Commands that *do* iterate over it
        # (add, install, remove, uninstall) gate on truthiness first.
        self.repa: list[Repa] = args.repa if "repa" in args else []
        self.yaml: bool = args.yaml if "yaml" in args else False

    def _load_template(self) -> dict:
        return load_template(Path(self.template_path))

    def _init_repoq(self) -> Repoq:
        return Repoq(self._load_template())

    def _report_target(self, target: str) -> None:
        if self.targets[target].out[-1][3] == 0:
            for line in self.targets[target].out[-1][1].splitlines():
                logger.info(blue(f"{target}") + f" - {line}")
        elif self.targets[target].out[-1][3] == 4:
            for line in self.targets[target].out[-1][1].splitlines():
                logger.warning(blue(f"{target}") + f" - {line}")
        else:
            for line in self.targets[target].out[-1][2].splitlines():
                logger.warning(blue(f"{target}") + f" - {line}")

    def _run_parallel(
        self,
        fn: Callable[..., None],
        *extra_args: Any,
    ) -> list[Future[None]]:
        """Fan ``fn(host, *extra_args)`` across all live targets.

        Returns the futures so callers can inspect ``.exception()``
        (used by PR 6 for exit-code propagation).
        """
        with concurrent.futures.ThreadPoolExecutor() as ex:
            futures = [ex.submit(fn, host, *extra_args) for host in self.targets.keys()]
            concurrent.futures.wait(futures)
            return futures

    @staticmethod
    def check_url(url: str) -> bool:
        """Check whether a repository URL exposes a valid repomd.xml.

        Tries ``<url>repodata/repomd.xml`` first and falls back to
        ``<url>suse/repodata/repomd.xml`` (used by SUSE-style layouts).

        Returns ``True`` if either probe succeeds, ``False`` otherwise.
        """
        try:
            urlopen(url + "repodata/repomd.xml")
            return True
        except (HTTPError, URLError):
            pass

        try:
            urlopen(url + "suse/repodata/repomd.xml")
            return True
        except (HTTPError, URLError):
            return False

    @abstractmethod
    def run(self) -> ExitCode:
        return 0
