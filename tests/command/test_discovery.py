"""Tests for ``repose.command`` registry-based command discovery."""

import pytest

import repose.command  # noqa: F401  # populate Command.registry
from repose.command import Command
from repose.types import ExitCode


EXPECTED_COMMANDS = {
    "add",
    "remove",
    "reset",
    "install",
    "clear",
    "uninstall",
    "list-products",
    "list-repos",
    "known-products",
}


def test_registry_contains_expected_commands():
    """Registry holds exactly the 9 CLI names — no more, no less."""
    assert set(Command.registry) == EXPECTED_COMMANDS


def test_registered_classes_inherit_command_base():
    for name, klass in Command.registry.items():
        assert issubclass(klass, Command), (
            f"{name!r} -> {klass!r} is not a Command subclass"
        )


def test_subclass_without_name_kwarg_is_not_registered():
    """Subclasses that omit ``name=`` must not pollute the registry."""

    class _Anon(Command):
        def run(self) -> ExitCode:
            return 0

    assert _Anon not in Command.registry.values()


def test_duplicate_name_raises():
    """Registering two classes under the same name must fail loudly."""
    sentinel = "__pr04_duplicate_test_name"
    assert sentinel not in Command.registry

    try:

        class _First(Command, name=sentinel):
            def run(self) -> ExitCode:
                return 0

        assert Command.registry[sentinel] is _First

        with pytest.raises(RuntimeError, match="Duplicate command name"):

            class _Second(Command, name=sentinel):
                def run(self) -> ExitCode:
                    return 0
    finally:
        Command.registry.pop(sentinel, None)
