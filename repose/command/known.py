from . import Command
from ..types import ExitCode


class KnownProducts(Command):
    command = True

    def run(self) -> ExitCode:
        template = self._load_template()
        self.display.list_known_products(template.keys())
        return 0
