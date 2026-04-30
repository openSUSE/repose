"""Tests for ``repose.host.ParseHosts``."""

import pytest

from repose.host import HostParseError, ParseHosts, PortNotIntError
from repose.target import Target


def test_default_user_and_port():
    hosts = ParseHosts("example.com")
    assert "example.com" in hosts

    target = hosts["example.com"]
    assert isinstance(target, Target)
    assert target.hostname == "example.com"
    assert target.username == "root"
    assert target.port == 22


def test_explicit_user_and_port():
    hosts = ParseHosts("admin@example.com:2222")
    key = "example.com:2222"
    assert key in hosts

    target = hosts[key]
    assert target.hostname == "example.com"
    assert target.username == "admin"
    assert target.port == 2222


def test_user_only_keeps_default_port():
    hosts = ParseHosts("alice@example.com")
    target = hosts["example.com"]
    assert target.username == "alice"
    assert target.port == 22


def test_invalid_port_raises_port_not_int_error():
    with pytest.raises(PortNotIntError):
        ParseHosts("user@example.com:notaport")


def test_port_not_int_error_is_host_parse_error():
    # Hierarchy contract used by argparse rendering
    assert issubclass(PortNotIntError, HostParseError)
