"""Tests for ``repose.types.system.System``."""

from repose.target.parsers import Product
from repose.types.system import System


def make_base():
    return Product("SLES", "15-SP3", "x86_64")


def make_addon(name="sle-module-basesystem", version="15-SP3"):
    return Product(name, version, "x86_64")


def test_str_no_addons():
    s = System(make_base())
    assert str(s) == "sles-15-SP3-x86_64"


def test_str_with_addons_includes_modules_suffix():
    s = System(make_base(), addons={make_addon()})
    assert str(s) == "sles-modules-15-SP3-x86_64"


def test_pretty_no_addons():
    s = System(make_base())
    out = s.pretty()
    assert out == ["  Base product: SLES-15-SP3-x86_64"]


def test_pretty_with_addons_lists_addons():
    s = System(make_base(), addons={make_addon("module-a", "15-SP3")})
    out = s.pretty()
    assert out[0] == "  Base product: SLES-15-SP3-x86_64"
    assert "Installed Extensions and Modules:" in out[1]
    assert any("module-a" in line for line in out[2:])


def test_arch():
    assert System(make_base()).arch() == "x86_64"


def test_eq_same_data():
    s1 = System(make_base())
    s2 = System(make_base())
    assert s1 == s2


def test_eq_different_addons():
    s1 = System(make_base())
    s2 = System(make_base(), addons={make_addon()})
    assert s1 != s2


def test_eq_with_non_system_is_notimplemented():
    s = System(make_base())
    # __eq__ returns NotImplemented for non-System; equality with arbitrary
    # objects must therefore be False.
    assert (s == "not a system") is False


def test_get_addons_default_empty_set():
    s = System(make_base())
    assert s.get_addons() == set()


def test_get_addons_returns_provided_set():
    addons = {make_addon()}
    s = System(make_base(), addons=addons)
    assert s.get_addons() == addons


def test_get_base_returns_product():
    base = make_base()
    s = System(base)
    assert s.get_base() == base


def test_to_refhost_dict_shape():
    s = System(make_base(), addons={make_addon()})
    d = s.to_refhost_dict()
    assert d["arch"] == "x86_64"
    assert d["product"] == {"name": "SLES", "version": "15-SP3"}
    assert d["addons"] == [{"name": "sle-module-basesystem", "version": "15-SP3"}]
    assert "location" in d


def test_to_refhost_dict_partially_normalized():
    s = System(make_base(), addons={make_addon()})
    d = s.to_refhost_dict_partially_normalized()
    assert d["product"] == {
        "name": "SLES",
        "version": {"major": 15, "minor": "SP3"},
    }
    assert d["addons"] == [
        {
            "name": "sle-module-basesystem",
            "version": {"major": 15, "minor": "SP3"},
        }
    ]


def test_flatten_returns_base_plus_addons():
    base = make_base()
    addon = make_addon()
    s = System(base, addons={addon})
    assert s.flatten() == {base, addon}
