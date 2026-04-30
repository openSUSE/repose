"""Tests for ``repose.types.repa.Repa``."""

import pytest
from hypothesis import given, strategies as st

from repose.types.repa import Repa


@pytest.mark.parametrize(
    "raw,product,version,arch,repo",
    [
        ("SLES:15-SP3:x86_64:update", "SLES", "15-SP3", "x86_64", "update"),
        ("SLES:15-SP3:x86_64", "SLES", "15-SP3", "x86_64", None),
        ("SLES:15-SP3", "SLES", "15-SP3", None, None),
        ("SLES", "SLES", None, None, None),
        ("", None, None, None, None),
        (":15-SP3::repo", None, "15-SP3", None, "repo"),
    ],
)
def test_parse_components(raw, product, version, arch, repo):
    r = Repa(raw)
    assert r.product == product
    assert r.version == version
    assert r.arch == arch
    assert r.repo == repo


def test_too_many_components_raises():
    with pytest.raises(ValueError, match="more than 4"):
        Repa("a:b:c:d:e")


@pytest.mark.parametrize(
    "version,baseversion,smallver",
    [
        ("15-SP3", "15", "-SP3"),
        ("15-SP0", "15", "-SP0"),
        ("12-SP5", "12", "-SP5"),
        ("15", None, None),  # baseversion=version, smallver=None
        ("ALL", None, None),
    ],
)
def test_version_setter_decomposition(version, baseversion, smallver):
    r = Repa(f"SLES:{version}::")
    assert r.smallver == smallver
    if "-SP" in version:
        assert r.baseversion == baseversion
    else:
        # plain version — baseversion equals version
        assert r.baseversion == version


def test_version_none_keeps_helpers_none():
    r = Repa("SLES")
    assert r.version is None
    assert r.smallver is None
    assert r.baseversion is None


def test_repr_includes_components():
    r = Repa("SLES:15-SP3:x86_64:update")
    text = repr(r)
    assert "SLES" in text
    assert "15-SP3" in text
    assert "x86_64" in text
    assert "update" in text


# ---------------------------------------------------------------------------
# Property-based tests
# ---------------------------------------------------------------------------


# Allow letters/digits/dashes in components, no colons
_component = st.text(
    alphabet=st.characters(
        whitelist_categories=("L", "N"),
        whitelist_characters="-_.",
    ),
    min_size=0,
    max_size=8,
)


@given(
    p=_component,
    v=_component,
    a=_component,
    r=_component,
)
def test_parse_roundtrip_never_raises(p, v, a, r):
    """Any well-formed 4-component REPA should parse without exception."""
    Repa(f"{p}:{v}:{a}:{r}")


@given(extra=st.integers(min_value=5, max_value=20))
def test_more_than_four_always_raises(extra):
    raw = ":".join(["x"] * extra)
    with pytest.raises(ValueError):
        Repa(raw)
