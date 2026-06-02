import logging
import xml.etree.ElementTree as ET
from typing import Any

from ...connection import Connection
from ...types.system import System
from ..parsers import Product

logger = logging.getLogger("repose.tartget.parsers.product")


# Local alias to keep the async-backend type-hint accurate without
# importing :mod:`repose.aiossh` at module-load time (avoids pulling
# asyncssh into the sync paramiko path's import graph).
if False:  # TYPE_CHECKING — but cheaper for ty/ruff
    from ...aiossh import AsyncConnection  # noqa: F401


def __parse_product(prod: Any) -> tuple[str, str, str]:
    root = ET.fromstringlist(prod)
    name = root.find("./name").text  # ty: ignore[unresolved-attribute]  # FOLLOWUP-ty-residuals
    arch = root.find("./arch").text  # ty: ignore[unresolved-attribute]  # FOLLOWUP-ty-residuals
    try:
        version = root.find("./baseversion").text  # ty: ignore[unresolved-attribute]  # FOLLOWUP-ty-residuals
        sp = (
            root.find("./patchlevel").text  # ty: ignore[unresolved-attribute]  # FOLLOWUP-ty-residuals
            if root.find("./patchlevel").text != "0"  # ty: ignore[unresolved-attribute]  # FOLLOWUP-ty-residuals
            else ""
        )
        version += f"-SP{sp}" if sp else ""  # ty: ignore[unsupported-operator]  # FOLLOWUP-ty-residuals
    except AttributeError:
        version = root.find("./version").text  # ty: ignore[unresolved-attribute]  # FOLLOWUP-ty-residuals
        logger.debug("simpleversion")

    # CAASP uses ALL for update repos and there is only one supported version at time
    # can change in tommorow
    if name == "CAASP":
        version = "ALL"
    return (name, version, arch)  # ty: ignore[invalid-return-type]  # FOLLOWUP-ty-residuals


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
    if "/" in basefile:  # ty: ignore[unsupported-operator]  # FOLLOWUP-ty-residuals
        basefile = basefile.split("/")[-1]  # ty: ignore[unresolved-attribute]  # FOLLOWUP-ty-residuals
    files.remove(basefile)  # ty: ignore[invalid-argument-type]  # FOLLOWUP-ty-residuals

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


async def parse_system_async(connection: Any) -> System:
    """Async equivalent of :func:`parse_system` for ``AsyncConnection``.

    Structure mirrors the sync version line-for-line — only the I/O
    calls (``listdir``, ``readlink``, ``open``) are ``await``ed. The
    ``_AsyncSFTPFileCtx`` returned by ``AsyncConnection.open`` exposes
    a sync iterator over its cached contents so the ``__parse_product``
    /``__parse_os_release`` bodies stay unchanged.

    ``connection`` is typed as ``Any`` because importing ``AsyncConnection``
    at module load would force asyncssh into the sync paramiko path's
    import graph for zero runtime benefit.
    """
    files: list[str] = []
    try:
        files = [
            x
            for x in await connection.listdir("/etc/products.d")
            if x.endswith(".prod")
        ]
    except OSError:
        logger.debug("Not SUSE's system")
        suse = False
    else:
        suse = True

    if not suse:
        try:
            async with connection.open("/etc/os-release") as f:
                name, version, arch = __parse_os_release(f)
        except FileNotFoundError:
            return System(Product("rhel", "6", "x86_64"))

        return System(Product(name, version, arch))

    basefile = await connection.readlink("/etc/products.d/baseproduct")
    if "/" in basefile:  # FOLLOWUP-ty-residuals
        basefile = basefile.split("/")[-1]  # FOLLOWUP-ty-residuals
    files.remove(basefile)  # FOLLOWUP-ty-residuals

    async with connection.open(f"/etc/products.d/{basefile}") as f:
        logger.debug("Parsing basefile")
        name, version, arch = __parse_product(f)
        base = Product(name, version, arch)

    addons = set()

    for x in files:
        async with connection.open(f"/etc/products.d/{x}") as f:
            logger.debug("parsing - %s", x)
            name, version, arch = __parse_product(f)
            if name.rpartition("-")[-1] != "migration":
                addons.add(Product(name, version, arch))
    return System(base, addons)
