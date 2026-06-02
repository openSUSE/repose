"""Integration tests for the sync/async dispatch in ``Command.run``.

These tests exercise the new ``Command.run`` template-method
behaviour: choose ``_arun`` over ``_srun`` only when the asyncssh
backend is active *and* the subclass implements ``_arun``. They
complement the unit tests in ``test_command_base.py`` and the
end-to-end SSH smoke in ``test_aiossh.py``.
"""

from __future__ import annotations

from argparse import Namespace
from unittest.mock import MagicMock

import pytest

from repose.command._command import Command
from repose.types import ExitCode


def _build_args(**overrides) -> Namespace:
    defaults = {
        "dry": False,
        "target": [],
        "config": "p",
        "yaml": False,
        "repa": [],
        "ssh_backend": "paramiko",
    }
    defaults.update(overrides)
    return Namespace(**defaults)


# Subclasses live inside a fixture so they don't pollute
# ``Command.registry`` (a module-level ``class _X(Command, name=...):``
# would persist across the test session and break the
# ``test_registry_contains_expected_commands`` discovery test).


@pytest.fixture
def sync_only_cls():
    """A Command subclass exposing only ``_srun`` — fresh per test."""

    class _SyncOnly(Command):
        sync_calls: list[int] = []

        def _srun(self) -> ExitCode:
            self.sync_calls.append(1)
            return 0

    _SyncOnly.sync_calls = []
    return _SyncOnly


@pytest.fixture
def both_cls():
    """A Command subclass implementing both ``_srun`` and ``_arun``."""

    class _Both(Command):
        sync_calls: list[int] = []
        async_calls: list[int] = []

        def _srun(self) -> ExitCode:
            self.sync_calls.append(1)
            return 0

        async def _arun(self) -> ExitCode:
            self.async_calls.append(1)
            return 0

    _Both.sync_calls = []
    _Both.async_calls = []
    return _Both


@pytest.fixture
def patch_hostgroup(monkeypatch):
    """Patch HostGroup so ``Command.__init__`` doesn't try real SSH."""
    import repose.command._command as cmdmod

    hg = MagicMock()
    hg.keys.return_value = []
    hg.__iter__ = lambda self: iter([])
    monkeypatch.setattr(cmdmod, "HostGroup", MagicMock(return_value=hg))
    return hg


@pytest.fixture
def patch_async_hostgroup(monkeypatch):
    """Patch AsyncHostGroup so ``Command.__init__`` doesn't try real SSH."""
    import repose.command._command as cmdmod

    hg = MagicMock()
    hg.keys.return_value = []
    hg.__iter__ = lambda self: iter([])

    # AsyncHostGroup.connect is async; return a coroutine.
    async def _connect():
        return None

    hg.connect = MagicMock(return_value=_connect())
    monkeypatch.setattr(cmdmod, "AsyncHostGroup", MagicMock(return_value=hg))
    return hg


def test_paramiko_backend_routes_to_srun(patch_hostgroup, both_cls):
    cmd = both_cls(_build_args(ssh_backend="paramiko"))
    rc = cmd.run()
    assert rc == 0
    assert both_cls.sync_calls == [1]
    assert both_cls.async_calls == []


def test_asyncssh_backend_routes_to_arun_when_implemented(
    patch_async_hostgroup, both_cls
):
    cmd = both_cls(_build_args(ssh_backend="asyncssh"))
    rc = cmd.run()
    assert rc == 0
    assert both_cls.async_calls == [1]
    assert both_cls.sync_calls == []


def test_asyncssh_backend_falls_back_to_srun_without_arun(
    patch_async_hostgroup, sync_only_cls
):
    """Subclass without ``_arun`` falls back to sync body on asyncssh."""
    cmd = sync_only_cls(_build_args(ssh_backend="asyncssh"))
    rc = cmd.run()
    assert rc == 0
    assert sync_only_cls.sync_calls == [1]


def test_default_backend_when_missing_attr(patch_hostgroup, sync_only_cls):
    """Tests building Namespace without ``ssh_backend`` get paramiko."""
    # Use a fresh Namespace without ssh_backend at all.
    ns = Namespace(
        dry=False,
        target=[],
        config="p",
        yaml=False,
        repa=[],
    )
    cmd = sync_only_cls(ns)
    assert cmd.ssh_backend == "paramiko"
    assert cmd._is_async is False
