from collections import UserDict
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..target.parsers import Product


def _parse_product(name: str, arch) -> "tuple[None, None] | Product":
    # Imported lazily: ``repose.target.parsers`` lives under the
    # ``repose.target`` package, whose ``__init__`` imports this module.
    # A module-level import here makes the two modules a hard import
    # cycle that breaks whenever ``repose.types.repositories`` is the
    # first of the pair to be imported. Deferring to call time keeps the
    # cycle from forming at import time.
    from ..target.parsers import Product

    parts = name.split(":")
    # TODO: more check for possible products
    if len(parts) != 4:
        return None, None

    return Product(parts[0], parts[1], arch)


class Repositories(UserDict):
    """Dictionary holding repositories on host"""

    def __init__(self, iterable, arch) -> None:
        """
        :param: iterable ... containing instances of Repository namedtuple
        :arch: architecture of target
        """
        self.data = {x.alias: _parse_product(x.name, arch) for x in iterable}
