import logging
from xml.etree import ElementTree as ET

from ..parsers import Repository

logger = logging.getLogger("repose.target.parsers.repository")

_REQUIRED_ATTRIBUTES = frozenset(["alias", "name", "enabled"])


def parse_repositories(xml: str) -> set[Repository]:
    """Parse ``zypper -x lr`` XML output into a set of repositories.

    A ``<repo>`` element missing one of the required attributes
    (``alias``, ``name``, ``enabled``) or lacking a non-empty ``<url>``
    child is skipped with a warning instead of aborting the parse, so
    well-formed sibling repositories are still returned.

    Args:
        xml: XML document produced by ``zypper -x lr``.

    Returns:
        Set of well-formed repositories found in the document.
    """
    repos: set[Repository] = set()
    root = ET.fromstring(xml)

    for repo in root.findall("./repo-list/repo"):
        missing = [attr for attr in _REQUIRED_ATTRIBUTES if attr not in repo.attrib]
        url_element = repo.find("./url")
        url = url_element.text if url_element is not None else None
        if missing or not url:
            if not url:
                missing.append("url")
            identifier = repo.attrib.get("alias") or repo.attrib.get("name") or "?"
            logger.warning(
                "skipping malformed repository entry '%s': missing %s",
                identifier,
                ", ".join(missing),
            )
            continue
        enabled = repo.attrib["enabled"] == "1"
        repos.add(Repository(repo.attrib["alias"], repo.attrib["name"], url, enabled))

    return repos
