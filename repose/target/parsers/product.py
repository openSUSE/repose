
import xml.etree.ElementTree as ET
import logging
from ..parsers import Product
from ...types.system import System

logger = logging.getLogger('repose.tartget.parsers.product')

def __parse_product(prod):
    root = ET.fromstringlist(prod)
    name = root.find('./name').text
    arch = root.find('./arch').text
    try:
        version = root.find('./baseversion').text
        sp = root.find('./patchlevel').text if root.find('./patchlevel').text != '0' else ""
        version += "-SP{}".format(sp) if sp else ""
    except AttributeError:
        version = root.find('./version').text
        logger.debug("simpleversion")

    # CAASP uses ALL for update repos and there is only one supported version at time
    # can change in tommorow
    if name == "CAASP":
        version = "ALL"
    return (name, version, arch)


def __parse_os_release(f):
    # TODO : ...
    logger.debug("TODO parse OSRELEASE file")
    return ("rhel", "7", "x86_64")


def parse_system(connection):
    try:
        files = [x for x in connection.listdir('/etc/products.d') if x != 'qa.prod' and x.endswith(".prod")]
    except IOError:
        logger.debug("Not SUSE's system")
        suse = False
    else:
        suse = True

    if not suse:
        try:
            with connection.open('/etc/os-release') as f:
                name, version, arch = __parse_os_release(f)
        except FileNotFoundError:
            # TODO: old RH systems have only /etc/redhat-release
            return System(Product("rhel", "6", "x86_64"))

        return System(Product(name, version, arch))

    basefile = connection.readlink('/etc/products.d/baseproduct')
    files.remove(basefile)

    with connection.open('/etc/products.d/{}'.format(basefile)) as f:
        logger.debug("Parsing basefile")
        name, version, arch = __parse_product(f)
        base = Product(name, version, arch)

    addons = set()

    for x in files:
        with connection.open('/etc/products.d/{}'.format(x)) as f:
            logger.debug("parsing - {}".format(x))
            name, version, arch = __parse_product(f)
            addons.add(Product(name, version, arch))

    return System(base, addons)
