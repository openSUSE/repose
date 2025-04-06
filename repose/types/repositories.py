from collections import UserDict

from ..target.parsers import Product


def _parse_product(name: str, arch) -> tuple[None, None] | Product:
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
