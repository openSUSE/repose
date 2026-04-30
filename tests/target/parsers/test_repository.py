"""Tests for ``repose.target.parsers.repository.parse_repositories``."""

import pytest

from repose.target.parsers import Repository
from repose.target.parsers.repository import parse_repositories


def test_parse_two_repos_with_mixed_state(sample_repos_xml):
    repos = parse_repositories(sample_repos_xml)
    assert isinstance(repos, set)
    assert repos == {
        Repository("repo-one", "Repo One", "http://example.com/one/", True),
        Repository("repo-two", "Repo Two", "http://example.com/two/", False),
    }


def test_empty_repo_list_returns_empty_set():
    xml = "<stream><repo-list></repo-list></stream>"
    assert parse_repositories(xml) == set()


@pytest.mark.parametrize(
    "enabled,state",
    [("1", True), ("0", False), ("2", False), ("", False)],
)
def test_enabled_attribute_mapping(enabled, state):
    xml = (
        "<stream><repo-list>"
        f'<repo alias="a" name="A" enabled="{enabled}">'
        "<url>http://e</url></repo>"
        "</repo-list></stream>"
    )
    (repo,) = parse_repositories(xml)
    assert repo.state is state


def test_malformed_xml_raises():
    from xml.etree.ElementTree import ParseError

    with pytest.raises(ParseError):
        parse_repositories("not <xml")
