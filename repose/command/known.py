from . import Command
from ..types import ExitCode


class KnownProducts(Command, name="known-products"):
    def run(self) -> ExitCode:
        template = self._load_template()
        self.display.list_known_products(sorted(template.keys()))
        return 0
