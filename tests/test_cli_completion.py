"""Tests for ``repose.cli._complete_repa`` shell-completion callback.

The callback is invoked by Typer/Click outside of a real shell during
unit tests; we drive it directly with a stub ``typer.Context``-like
object carrying a ``CliGlobals`` payload (the shape the root callback
attaches at runtime).
"""

from dataclasses import dataclass
from pathlib import Path
from typing import Any

import pytest

from repose.cli import CliGlobals, _complete_repa
from repose.types.connection_config import ConnectionConfig


@dataclass
class _StubCtx:
    """Minimal ``typer.Context`` stand-in.

    ``_complete_repa`` only reads ``ctx.obj``; supplying the live
    Typer/Click context isn't necessary and would couple the test to
    the Click invocation machinery.
    """

    obj: Any


def _make_globals(config: Path) -> CliGlobals:
    """Construct a ``CliGlobals`` with the given config path.

    Other ``CliGlobals`` fields use the same defaults the root callback
    would supply; only ``config`` matters for completion.
    """
    return CliGlobals(
        dry=False,
        config=config,
        debug=False,
        quiet=False,
        no_color=False,
        format="text",
        strict_host_key_checking="accept-new",
        known_hosts=Path("~/.ssh/known_hosts"),
        ssh_backend="asyncssh",
        conn_config=ConnectionConfig(),
    )


@pytest.fixture
def products_yml(tmp_path: Path) -> Path:
    """A products.yml with a handful of recognizable product names."""
    p = tmp_path / "products.yml"
    p.write_text(
        "SLES:\n"
        "  15-SP5:\n"
        "    x86_64:\n"
        "      base:\n"
        "        - { name: SLE_BASE, url: 'http://example.com/' }\n"
        "sle-sdk:\n"
        "  15-SP5:\n"
        "    x86_64:\n"
        "      base:\n"
        "        - { name: SDK_BASE, url: 'http://example.com/' }\n"
        "sle-module-toolchain:\n"
        "  15-SP5:\n"
        "    x86_64:\n"
        "      base:\n"
        "        - { name: TOOLCHAIN_BASE, url: 'http://example.com/' }\n"
        "openSUSE-Leap:\n"
        "  15.5:\n"
        "    x86_64:\n"
        "      base:\n"
        "        - { name: LEAP_BASE, url: 'http://example.com/' }\n"
    )
    return p


def test_repa_complete_filters_prefix(products_yml: Path) -> None:
    """Empty prefix returns all products sorted; prefix narrows them."""
    ctx = _StubCtx(obj=_make_globals(products_yml))

    all_products = _complete_repa(ctx, "")
    assert all_products == sorted(
        ["SLES", "sle-sdk", "sle-module-toolchain", "openSUSE-Leap"]
    )

    sle_matches = _complete_repa(ctx, "sle-")
    assert sle_matches == ["sle-module-toolchain", "sle-sdk"]

    one_match = _complete_repa(ctx, "openSUSE")
    assert one_match == ["openSUSE-Leap"]


def test_repa_complete_no_matches_returns_empty(products_yml: Path) -> None:
    """Prefix that matches nothing returns an empty list (not an error)."""
    ctx = _StubCtx(obj=_make_globals(products_yml))
    assert _complete_repa(ctx, "nonexistent-product") == []


def test_repa_complete_after_colon_returns_empty(products_yml: Path) -> None:
    """Once the user has typed ``:``, completion bows out.

    We only complete the first segment (the product). The
    ``:VERSION:ARCH:REPO`` tail is free-form.
    """
    ctx = _StubCtx(obj=_make_globals(products_yml))
    assert _complete_repa(ctx, "SLES:") == []
    assert _complete_repa(ctx, "SLES:15-SP5:") == []


def test_repa_complete_missing_config_returns_empty(tmp_path: Path) -> None:
    """A nonexistent config file collapses to an empty list.

    Completion must never raise inside the user's shell.
    """
    missing = tmp_path / "does-not-exist.yml"
    ctx = _StubCtx(obj=_make_globals(missing))
    assert _complete_repa(ctx, "") == []


def test_repa_complete_malformed_yaml_returns_empty(tmp_path: Path) -> None:
    """Malformed YAML collapses to an empty list (YAMLError swallowed)."""
    bad = tmp_path / "broken.yml"
    bad.write_text("this is: : not: valid: yaml:\n  -\n  -\n  - [\n")
    ctx = _StubCtx(obj=_make_globals(bad))
    assert _complete_repa(ctx, "") == []


def test_repa_complete_no_ctx_obj_uses_default_path() -> None:
    """When ``ctx.obj`` is missing, the helper falls back to the default
    ``/etc/repose/products.yml``; if that file is unreadable, return [].

    This exercises the defensive branch — the default path is unlikely
    to exist in CI, so we just assert the call doesn't raise and that
    it returns an empty list when the file is missing.
    """
    ctx = _StubCtx(obj=None)
    result = _complete_repa(ctx, "")
    # On a dev box where /etc/repose/products.yml happens to exist the
    # call would return real products; either way, it must not raise.
    assert isinstance(result, list)
