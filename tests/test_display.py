"""Tests for ``repose.display.CommandDisplay``."""

import io
import json

from ruamel.yaml import YAML

from repose.display import CommandDisplay, JsonCommandDisplay
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


# ---------------------------------------------------------------------------
# JsonCommandDisplay
# ---------------------------------------------------------------------------


def _ndjson(buf: io.StringIO) -> list[dict]:
    return [json.loads(line) for line in buf.getvalue().splitlines() if line.strip()]


def test_json_list_products_emits_base_and_addons():
    buf = io.StringIO()
    cd = JsonCommandDisplay(buf)

    base = Product("SLES", "15-SP3", "x86_64")
    addon = Product("sle-sdk", "15-SP3", "x86_64")
    system = System(base, addons={addon})
    cd.list_products("host1", 22, system)

    events = _ndjson(buf)
    assert len(events) == 2

    kinds = {e["kind"]: e for e in events}
    assert kinds["base"] == {
        "event": "product",
        "host": "host1",
        "port": 22,
        "kind": "base",
        "name": "SLES",
        "version": "15-SP3",
        "arch": "x86_64",
    }
    assert kinds["addon"]["name"] == "sle-sdk"
    assert kinds["addon"]["kind"] == "addon"


def test_json_list_products_base_only_emits_single_event():
    buf = io.StringIO()
    cd = JsonCommandDisplay(buf)

    cd.list_products("host1", 22, System(Product("SLES", "15-SP3", "x86_64")))

    events = _ndjson(buf)
    assert len(events) == 1
    assert events[0]["kind"] == "base"


def test_json_list_update_repos_one_event_per_repo():
    buf = io.StringIO()
    cd = JsonCommandDisplay(buf)

    repos = [
        Repository("a", "alpha", "http://a/", True),
        Repository("b", "beta", "http://b/", False),
    ]
    cd.list_update_repos("host1", 22, repos)

    events = _ndjson(buf)
    assert len(events) == 2
    assert events[0] == {
        "event": "repo",
        "host": "host1",
        "port": 22,
        "alias": "a",
        "name": "alpha",
        "url": "http://a/",
        "state": True,
    }
    assert events[1]["alias"] == "b"
    assert events[1]["state"] is False


def test_json_list_known_products_one_event_per_product():
    buf = io.StringIO()
    cd = JsonCommandDisplay(buf)

    cd.list_known_products(["SLES", "openSUSE", "RHEL"])

    events = _ndjson(buf)
    assert [e["event"] for e in events] == ["known_product"] * 3
    assert [e["name"] for e in events] == ["SLES", "openSUSE", "RHEL"]


def test_json_list_products_yaml_emits_host_spec_event():
    buf = io.StringIO()
    cd = JsonCommandDisplay(buf)

    cd.list_products_yaml("host1", System(Product("SLES", "15-SP3", "x86_64")))

    events = _ndjson(buf)
    assert len(events) == 1
    payload = events[0]
    assert payload["event"] == "host_spec"
    assert payload["host"] == "host1"
    assert payload["name"] == "host1"
    assert payload["arch"] == "x86_64"
    assert payload["product"]["name"] == "SLES"
