"""Tests for ``repose.command`` dynamic command discovery."""

import repose.command as cmd_pkg


EXPECTED_COMMANDS = {
    "Add",
    "Remove",
    "Install",
    "Uninstall",
    "Clear",
    "Reset",
    "ListRepos",
    "ListProducts",
    "KnownProducts",
}


def test_cmd_list_contains_expected_commands():
    assert EXPECTED_COMMANDS.issubset(set(cmd_pkg.cmd_list))


def test_each_command_class_exposed_at_module_level():
    for name in EXPECTED_COMMANDS:
        assert hasattr(cmd_pkg, name)
        assert getattr(cmd_pkg, name).command is True


def test_command_classes_inherit_command_base():
    for name in EXPECTED_COMMANDS:
        klass = getattr(cmd_pkg, name)
        assert issubclass(klass, cmd_pkg.Command)
