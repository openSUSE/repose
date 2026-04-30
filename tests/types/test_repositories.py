"""Tests for ``repose.types.repositories``."""

from repose.target.parsers import Product, Repository
from repose.types.repositories import Repositories, _parse_product


def test_parse_product_four_parts_returns_product():
    result = _parse_product("SLES:15-SP3::update", "x86_64")
    assert result == Product("SLES", "15-SP3", "x86_64")


def test_parse_product_too_few_parts_returns_none_pair():
    assert _parse_product("SLES", "x86_64") == (None, None)
    assert _parse_product("SLES:15-SP3", "x86_64") == (None, None)
    assert _parse_product("a:b:c:d:e", "x86_64") == (None, None)


def test_repositories_keyed_by_alias():
    repos = [
        Repository("alias-a", "SLES:15-SP3::update", "http://a", True),
        Repository("alias-b", "openSUSE:15.3::oss", "http://b", False),
        Repository("alias-c", "not-a-product-name", "http://c", True),
    ]
    r = Repositories(repos, "x86_64")

    assert set(r.keys()) == {"alias-a", "alias-b", "alias-c"}
    assert r["alias-a"] == Product("SLES", "15-SP3", "x86_64")
    assert r["alias-b"] == Product("openSUSE", "15.3", "x86_64")
    assert r["alias-c"] == (None, None)


def test_repositories_empty_iterable():
    r = Repositories([], "x86_64")
    assert dict(r) == {}
