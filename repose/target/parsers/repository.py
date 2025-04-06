from xml.etree import ElementTree as ET

from ..parsers import Repository


def parse_repositories(xml) -> set[Repository]:
    repos = set()
    root = ET.fromstring(xml)

    for repo in root.findall("./repo-list/repo"):
        alias = repo.attrib["alias"]
        name = repo.attrib["name"]
        enabled = True if repo.attrib["enabled"] == "1" else False
        url = repo.find("./url").text
        repos.add(Repository(alias, name, url, enabled))

    return repos
