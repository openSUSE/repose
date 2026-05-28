"""Tests for ``repose.host.ParseHosts``."""

from pathlib import Path

import pytest

from repose.host import HostParseError, ParseHosts, PortNotIntError
from repose.target import Target
from repose.types.connection_config import ConnectionConfig


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


def test_oneshot_mode_uses_default_config():
    hosts = ParseHosts("example.com")
    target = hosts["example.com"]
    # Default ConnectionConfig is applied.
    assert target.config == ConnectionConfig()


def test_factory_mode_threads_config_into_targets():
    """``ParseHosts(cfg)(host_str)`` propagates ``cfg`` to each Target."""
    cfg = ConnectionConfig(host_key_policy="yes", known_hosts=Path("/tmp/kh"))
    factory = ParseHosts(cfg)
    # Factory itself starts empty.
    assert dict(factory) == {}

    result = factory("admin@example.com:2222")
    assert isinstance(result, ParseHosts)
    target = result["example.com:2222"]
    assert target.config == cfg
    assert target.hostname == "example.com"
    assert target.username == "admin"
    assert target.port == 2222


def test_factory_mode_each_call_returns_independent_dict():
    """Two ``__call__`` invocations on one factory must not share state."""
    factory = ParseHosts(ConnectionConfig())
    a = factory("a.example.com")
    b = factory("b.example.com")
    assert "a.example.com" in a
    assert "b.example.com" in b
    assert "a.example.com" not in b
    assert "b.example.com" not in a
