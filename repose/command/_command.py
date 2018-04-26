
import sys
from urllib.request import urlopen
from urllib.error import HTTPError
import logging

from ..target.hostgroup import HostGroup
from ..display import CommandDisplay
from ..template import load_template
from ..template.resolver import Repoq
from ..utils import blue

logger = logging.getLogger('repose.command')


class Command(object):
    addcmd = "zypper -n ar {params} {name} {url} {name}"
    rrcmd = "zypper -n rr {repos}"
    refcmd = "zypper -n --gpg-auto-import-keys ref -f"
    ipdcmd = "zypper -n in -t product -l -f {products}"
    rrpcmd = "zypper -n rm -t product {products}"

    def __init__(self, args):
        __dtargets = {}

        for x in args.target:
            __dtargets.update(x)

        targets = HostGroup(__dtargets)
        targets.connect()

        # cann't  use dict comprehension - custom dict for hostgroup:(
        for target in list(targets.keys()):
            if not targets[target]:
                del(targets[target])
        self.targets = targets

        self.dryrun = args.dry
        self.template_path = args.config
        self.display = CommandDisplay(sys.stdout)
        self.repa = args.repa if 'repa' in args else None

    def _init_repoq(self):
        return Repoq(load_template(self.template_path))

    def _report_target(self, target):

        if self.targets[target].out[-1][3] == 0:
            for line in self.targets[target].out[-1][1].splitlines():
                logger.info(blue("{}".format(target)) + " - {}".format(line))
        else:
            for line in self.targets[target].out[-1][2].splitlines():
                logger.warning(blue("{}".format(target)) + " - {}".format(line))

    @staticmethod
    def check_url(url):
        state = True
        try:
            urlopen(url + "repodata/repomd.xml")
        except HTTPError:
            state = False
        if not state:
            try:
                urlopen(url + "suse/repodata/repomd.xml")
            except HTTPError:
                state = False
        return state
