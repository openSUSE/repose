
from .utils import green, yellow, red, blue


class CommandDisplay(object):

    def __init__(self, output):
        self.output = output

    def println(self, msg='', eol='\n'):
        return self.output.write(msg + eol)

    def list_products(self, hostname, port, system):
        self.println("{}: {}:{}".format(green("Host"), yellow(hostname), yellow(port)))
        for x in system.pretty():
            self.println(x)
        self.println()

    def list_update_repos(self, hostname, port, repos):

        self.println("{} on {}:{}".format(green("Repositories"), blue(hostname), blue(str(port))))
        for repository in repos:
            self.println("{}: {}".format(green("REPO name"), repository.name))
            self.println("{}: {}".format(green("REPO URL"), repository.url))
        self.println()
