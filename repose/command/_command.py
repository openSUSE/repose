from abc import ABC, abstractmethod
import logging
import sys
from urllib.error import HTTPError, URLError
from urllib.request import urlopen

from ..display import CommandDisplay
from ..target.hostgroup import HostGroup
from ..template import load_template
from ..template.resolver import Repoq
from ..types import ExitCode
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

    def __init__(self, args) -> None:
        __dtargets = {}

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

        self.dryrun = args.dry
        self.template_path = args.config
        self.display = CommandDisplay(sys.stdout)
        self.repa = args.repa if "repa" in args else None
        self.yaml = args.yaml if "yaml" in args else False

    def _load_template(self):
        return load_template(self.template_path)

    def _init_repoq(self) -> Repoq:
        return Repoq(self._load_template())

    def _report_target(self, target) -> None:
        if self.targets[target].out[-1][3] == 0:
            for line in self.targets[target].out[-1][1].splitlines():
                logger.info(blue(f"{target}") + f" - {line}")
        elif self.targets[target].out[-1][3] == 4:
            for line in self.targets[target].out[-1][1].splitlines():
                logger.warning(blue(f"{target}") + f" - {line}")
        else:
            for line in self.targets[target].out[-1][2].splitlines():
                logger.warning(blue(f"{target}") + f" - {line}")

    @staticmethod
    def check_url(url) -> bool:
        state = True
        try:
            urlopen(url + "repodata/repomd.xml")
        except (HTTPError, URLError):
            state = False

        if not state:
            try:
                urlopen(url + "suse/repodata/repomd.xml")
            except (HTTPError, URLError):
                state = False
        return state

    @abstractmethod
    def run(self) -> ExitCode:
        return 0
