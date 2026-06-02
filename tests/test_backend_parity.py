"""Backend parity smoke test: same command, two backends, same outcome.

Runs ``KnownProducts`` (the simplest read-only command that doesn't
touch SSH at all) through ``Command.run`` once with
``ssh_backend=paramiko`` and once with ``ssh_backend=asyncssh``, and
asserts identical exit codes plus identical stdout. This is the
project-wide parity gate that the PR-14 plan calls out as the
strongest behaviour-preservation guarantee while the two backends
coexist.

The more thorough end-to-end coverage (real SSH sessions, listdir,
file open, exec) lives in ``tests/test_aiossh.py`` for the asyncssh
backend and ``tests/test_connection.py`` for the paramiko backend.
"""

from __future__ import annotations

import io
from argparse import Namespace
from pathlib import Path
from unittest.mock import patch

import pytest

from repose.command.known import KnownProducts


@pytest.fixture
def template_with_two_products(tmp_path):
    """Minimal repose template with two product entries."""
    tpl = tmp_path / "products.yml"
    tpl.write_text(
        "SLES:\n"
        "  15-SP5:\n"
        "    x86_64:\n"
        "      base:\n"
        "        - { name: SLE_BASE, url: 'http://example.com/' }\n"
        "openSUSE-Leap:\n"
        "  15.5:\n"
        "    x86_64:\n"
        "      base:\n"
        "        - { name: LEAP_BASE, url: 'http://example.com/' }\n"
    )
    return tpl


def _ns(tpl: Path, backend: str) -> Namespace:
    return Namespace(
        dry=False,
        target=[],
        config=tpl,
        yaml=False,
        repa=[],
        ssh_backend=backend,
        debug=False,
        quiet=True,
        no_color=True,
        format="text",
        strict_host_key_checking="accept-new",
        known_hosts=None,
    )


def test_known_products_parity_across_backends(template_with_two_products):
    """``known-products`` must emit identical output on both backends."""
    outputs: dict[str, str] = {}
    exit_codes: dict[str, int] = {}

    for backend in ("paramiko", "asyncssh"):
        buf = io.StringIO()
        with patch("sys.stdout", buf):
            cmd = KnownProducts(_ns(template_with_two_products, backend))
            rc = cmd.run()
        outputs[backend] = buf.getvalue()
        exit_codes[backend] = rc

    assert exit_codes["paramiko"] == exit_codes["asyncssh"] == 0
    assert outputs["paramiko"] == outputs["asyncssh"]
    # And the output mentions the two products in the template.
    assert "SLES" in outputs["paramiko"]
    assert "openSUSE-Leap" in outputs["paramiko"]
