"""Tests for ``repose.template.resolver.Repoq``."""

import pytest

from repose.messages import UnsuportedProductMessage
from repose.target.parsers import Product
from repose.template.resolver import Repoq, Repos
from repose.types.repa import Repa
from repose.types.system import System


@pytest.fixture
def template():
    return {
        "SLES": {
            "15-SP3": {
                "default_repos": ["update", "pool"],
                "update": {
                    "url": "http://example.com/$version/$arch/upd",
                    "enabled": True,
                },
                "pool": {
                    "url": "http://example.com/$shortver/$arch/pool",
                    "enabled": False,
                },
            },
            "15": {
                "default_repos": ["update"],
                "update": {
                    "url": "http://example.com/15/$arch/upd",
                    "enabled": True,
                },
            },
        },
        "openSUSE": {
            "15.5": {
                "default_repos": ["oss"],
                "oss": {
                    "url": "http://download.opensuse.org/$version/$arch",
                    "enabled": True,
                },
            },
        },
    }


@pytest.fixture
def repoq(template):
    return Repoq(template)


@pytest.fixture
def base():
    return Product("SLES", "15-SP3", "x86_64")


def test_solve_repa_default_repos_returns_all(repoq, base):
    repa = Repa("SLES:15-SP3:x86_64:")
    result = repoq.solve_repa(repa, base)

    repos = result["SLES"]
    assert len(repos) == 2
    names = {r.name for r in repos}
    assert names == {
        "SLES:15-SP3::update",
        "SLES:15-SP3::pool",
    }


def test_solve_repa_specific_repo(repoq, base):
    repa = Repa("SLES:15-SP3:x86_64:update")
    result = repoq.solve_repa(repa, base)

    repos = result["SLES"]
    assert len(repos) == 1
    assert repos[0] == Repos(
        "SLES:15-SP3::update",
        "http://example.com/15-SP3/x86_64/upd",
        True,
    )


def test_template_substitution_includes_shortver(repoq, base):
    repa = Repa("SLES:15-SP3:x86_64:pool")
    (repo,) = repoq.solve_repa(repa, base)["SLES"]
    # shortver = version with "-" stripped
    assert repo.url == "http://example.com/15SP3/x86_64/pool"


def test_arch_inherited_from_base_when_missing(repoq, base):
    repa = Repa("SLES:15-SP3::update")
    (repo,) = repoq.solve_repa(repa, base)["SLES"]
    assert "x86_64" in repo.url


def test_version_inherited_from_base_when_missing(repoq, base):
    repa = Repa("SLES:::update")
    (repo,) = repoq.solve_repa(repa, base)["SLES"]
    assert repo.name == "SLES:15-SP3::update"


def test_baseversion_fallback_when_full_version_missing(repoq):
    """If repa.version not in template, use baseversion (e.g. '15' for '15-SP9')."""
    base = Product("SLES", "15-SP9", "x86_64")
    repa = Repa("SLES:15-SP9:x86_64:")
    result = repoq.solve_repa(repa, base)
    # Should resolve via the "15" template entry
    (repo,) = result["SLES"]
    assert repo.name == "SLES:15::update"


def test_unknown_product_raises_with_suggestion(repoq, base):
    repa = Repa("SLE:15-SP3:x86_64:")  # close to SLES
    with pytest.raises(ValueError) as exc:
        repoq.solve_repa(repa, base)
    msg = str(exc.value)
    assert "SLE" in msg
    assert "SLES" in msg  # close-match suggestion


def test_unknown_product_raises_without_suggestion_when_no_match(repoq, base):
    repa = Repa("ZZZZZZ:15-SP3:x86_64:")
    with pytest.raises(ValueError) as exc:
        repoq.solve_repa(repa, base)
    assert "ZZZZZZ" in str(exc.value)


def test_unknown_version_raises(repoq):
    base = Product("SLES", "99", "x86_64")
    repa = Repa("SLES:99:x86_64:")
    with pytest.raises(ValueError, match="Unknow version"):
        repoq.solve_repa(repa, base)


def test_missing_repo_in_template_yields_empty_url(repoq, base):
    repa = Repa("SLES:15-SP3:x86_64:nonexistent")
    (repo,) = repoq.solve_repa(repa, base)["SLES"]
    assert repo.url == "http://empty.url"
    assert repo.refresh is False


def test_solve_product_happy_path(repoq):
    base = Product("SLES", "15-SP3", "x86_64")
    system = System(base)
    result = repoq.solve_product(system)

    assert "SLES" in result
    assert len(result["SLES"]) == 2  # default_repos = update + pool


def test_solve_product_unknown_product_raises(repoq):
    base = Product("Unknown", "1.0", "x86_64")
    system = System(base)
    with pytest.raises(UnsuportedProductMessage):
        repoq.solve_product(system)


def test_solve_repa_does_not_mutate_input(repoq, base):
    repa = Repa("SLES:::update")
    original_arch = repa.arch
    original_version = repa.version

    repoq.solve_repa(repa, base)

    assert repa.arch == original_arch
    assert repa.version == original_version
