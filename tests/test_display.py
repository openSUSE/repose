"""Tests for ``repose.display.CommandDisplay``."""

import io

from ruamel.yaml import YAML

from repose.display import CommandDisplay
from repose.target.parsers import Product, Repository
from repose.types.system import System


def test_println_writes_message_with_newline():
    buf = io.StringIO()
    CommandDisplay(buf).println("hello")
    assert buf.getvalue() == "hello\n"


def test_println_default_empty_writes_just_newline():
    buf = io.StringIO()
    CommandDisplay(buf).println()
    assert buf.getvalue() == "\n"


def test_println_custom_eol():
    buf = io.StringIO()
    CommandDisplay(buf).println("x", eol="!")
    assert buf.getvalue() == "x!"


def test_list_products_writes_host_and_pretty_lines():
    buf = io.StringIO()
    cd = CommandDisplay(buf)

    system = System(Product("SLES", "15-SP3", "x86_64"))
    cd.list_products("host1", 22, system)

    out = buf.getvalue()
    assert "host1" in out
    assert "22" in out
    assert "SLES-15-SP3-x86_64" in out


def test_list_update_repos_writes_each_repo():
    buf = io.StringIO()
    cd = CommandDisplay(buf)

    repos = [
        Repository("a", "alpha", "http://a/", True),
        Repository("b", "beta", "http://b/", False),
    ]
    cd.list_update_repos("host1", 22, repos)

    out = buf.getvalue()
    assert "host1" in out
    assert "alpha" in out
    assert "beta" in out
    assert "http://a/" in out
    assert "http://b/" in out


def test_list_known_products_joins_with_spaces():
    buf = io.StringIO()
    cd = CommandDisplay(buf)
    cd.list_known_products(["SLES", "openSUSE", "RHEL"])

    out = buf.getvalue()
    assert "Products known by 'repose':" in out
    assert "SLES openSUSE RHEL" in out


def test_list_products_yaml_emits_parseable_yaml():
    buf = io.StringIO()
    cd = CommandDisplay(buf)

    system = System(Product("SLES", "15-SP3", "x86_64"))
    cd.list_products_yaml("host1", system)

    parsed = YAML(typ="safe").load(buf.getvalue())
    assert parsed["name"] == "host1"
    assert parsed["arch"] == "x86_64"
    assert parsed["product"]["name"] == "SLES"
    assert parsed["product"]["version"] == {"major": 15, "minor": "SP3"}
