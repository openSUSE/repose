from . import Command


class KnownProducts(Command):
    command = True

    def run(self):
        template = self._load_template()
        self.display.list_known_products(template.keys())
