
from . import Command
import logging

logger = logging.getLogger('repose.command.pokus')


class Pokus(Command):
    command = True

    def run(self):
        self.targets.run("zypper lr")
        for host in self.targets.keys():
            output = self.targets[host].out[-1]
            if output[2]:
                for line in output[2].splitlines():
                    logger.warning("{} - {}: {}".format(host, output[0], line))
            else:
                for line in output[1].splitlines():
                    logger.info("{} - {}: {}".format(host, output[0], line))

        self.targets.close()
