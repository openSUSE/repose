
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

    def list_known_products(self, products):
        self.println(green("Products known by 'repose':"))
        self.println(" ".join(products))
        self.println()

    def list_products_yaml(self, hostname, system):
        from ruamel.yaml import YAML
        yml = YAML(typ='safe', pure=False)
        yml.default_flow_style = False
        yml.explicit_end = True
        yml.explicit_start = True
        yml.indent(mapping=4, sequence=4, offset=2)
        data = system.to_refhost_dict()
        data["name"] = str(hostname)
        yml.dump(data, self.output)
