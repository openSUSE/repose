from . import Command
from ..template import load_template
from ..types import ExitCode


class KnownProducts(Command, name="known-products"):
    def run(self) -> ExitCode:
        template = load_template(self.template_path)
        self.display.list_known_products(sorted(template.keys()))
        return 0
