"""Tests for ``repose.target.parsers.repository.parse_repositories``."""

import logging

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


VALID_REPO = (
    '<repo alias="good" name="Good" enabled="1">'
    "<url>http://example.com/good/</url></repo>"
)
GOOD_REPOSITORY = Repository("good", "Good", "http://example.com/good/", True)


def _wrap(repo_elements: str) -> str:
    return f"<stream><repo-list>{repo_elements}</repo-list></stream>"


@pytest.mark.parametrize(
    "malformed,name_hint,missing_hint",
    [
        pytest.param(
            '<repo name="Bad" enabled="1"><url>http://e</url></repo>',
            "Bad",
            "alias",
            id="missing-alias-attribute",
        ),
        pytest.param(
            '<repo alias="bad" enabled="1"><url>http://e</url></repo>',
            "bad",
            "name",
            id="missing-name-attribute",
        ),
        pytest.param(
            '<repo alias="bad" name="Bad"><url>http://e</url></repo>',
            "bad",
            "enabled",
            id="missing-enabled-attribute",
        ),
        pytest.param(
            '<repo alias="bad" name="Bad" enabled="1"></repo>',
            "bad",
            "url",
            id="missing-url-element",
        ),
        pytest.param(
            '<repo alias="bad" name="Bad" enabled="1"><url/></repo>',
            "bad",
            "url",
            id="empty-url-element",
        ),
    ],
)
def test_malformed_repo_is_skipped_with_warning(
    caplog, malformed, name_hint, missing_hint
):
    """A malformed <repo> is skipped; well-formed siblings still parse."""
    xml = _wrap(malformed + VALID_REPO)
    with caplog.at_level(logging.WARNING, logger="repose.target.parsers.repository"):
        repos = parse_repositories(xml)
    assert repos == {GOOD_REPOSITORY}
    assert len(caplog.records) == 1
    message = caplog.records[0].getMessage()
    assert name_hint in message
    assert missing_hint in message


def test_repo_missing_everything_is_skipped(caplog):
    """A completely empty <repo/> does not abort the parse."""
    xml = _wrap("<repo/>" + VALID_REPO)
    with caplog.at_level(logging.WARNING, logger="repose.target.parsers.repository"):
        repos = parse_repositories(xml)
    assert repos == {GOOD_REPOSITORY}
    assert len(caplog.records) == 1


def test_empty_url_never_stored_as_none(caplog):
    """An empty <url/> must not yield a Repository with url=None."""
    xml = _wrap('<repo alias="a" name="A" enabled="1"><url/></repo>')
    with caplog.at_level(logging.WARNING, logger="repose.target.parsers.repository"):
        repos = parse_repositories(xml)
    assert repos == set()
    assert len(caplog.records) == 1
