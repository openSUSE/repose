import logging
import xml.etree.ElementTree as ET
from typing import Any

from ...connection import Connection
from ...types.system import System
from ..parsers import Product

logger = logging.getLogger("repose.tartget.parsers.product")


def __parse_product(prod: Any) -> tuple[str, str, str]:
    root = ET.fromstringlist(prod)
    name = root.find("./name").text
    arch = root.find("./arch").text
    try:
        version = root.find("./baseversion").text
        sp = (
            root.find("./patchlevel").text
            if root.find("./patchlevel").text != "0"
            else ""
        )
        version += f"-SP{sp}" if sp else ""
    except AttributeError:
        version = root.find("./version").text
        logger.debug("simpleversion")

    # CAASP uses ALL for update repos and there is only one supported version at time
    # can change in tommorow
    if name == "CAASP":
        version = "ALL"
    return (name, version, arch)


def __parse_os_release(f: Any) -> tuple[str, str, str]:
    # TODO : ...
    logger.debug("TODO parse OSRELEASE file")
    return ("rhel", "7", "x86_64")


def parse_system(connection: Connection) -> System:
    files = []
    try:
        files = [
            x for x in connection.listdir("/etc/products.d") if x.endswith(".prod")
        ]
    except OSError:
        logger.debug("Not SUSE's system")
        suse = False
    else:
        suse = True

    if not suse:
        try:
            with connection.open("/etc/os-release") as f:
                name, version, arch = __parse_os_release(f)
        except FileNotFoundError:
            # TODO: old RH systems have only /etc/redhat-release
            return System(Product("rhel", "6", "x86_64"))

        return System(Product(name, version, arch))

    basefile = connection.readlink("/etc/products.d/baseproduct")
    if "/" in basefile:
        basefile = basefile.split("/")[-1]
    files.remove(basefile)

    with connection.open(f"/etc/products.d/{basefile}") as f:
        logger.debug("Parsing basefile")
        name, version, arch = __parse_product(f)
        base = Product(name, version, arch)

    addons = set()

    for x in files:
        with connection.open(f"/etc/products.d/{x}") as f:
            logger.debug("parsing - %s", x)
            name, version, arch = __parse_product(f)
            if name.rpartition("-")[-1] != "migration":
                addons.add(Product(name, version, arch))
    return System(base, addons)
