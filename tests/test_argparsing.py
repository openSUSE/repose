import sys

import pytest

import repose.argparsing
from repose import __version__
from repose.argparsing import (
    do_add,
    do_clear,
    do_install,
    do_known_products,
    do_list_products,
    do_list_repos,
    do_remove,
    do_reset,
    do_uninstall,
    get_parser,
)


@pytest.fixture
def mock_types(monkeypatch):
    monkeypatch.setattr(repose.argparsing, "Repa", lambda x: x)
    monkeypatch.setattr(repose.argparsing, "ParseHosts", lambda x: x)


def test_version_action(monkeypatch, capsys):
    monkeypatch.setattr(sys, "argv", ["repose", "--version"])
    with pytest.raises(SystemExit):
        get_parser().parse_args()

    ret = capsys.readouterr()
    assert ret.out == f"repose version: {__version__}\n"


def test_debug_action():
    parser = get_parser()
    args = parser.parse_args(["-d", "add", "-t", "dummy", "dummy"])
    assert args.debug is True

    parser = get_parser()
    args = parser.parse_args(["-q", "add", "-t", "dummy", "dummy"])
    assert args.quiet is True


@pytest.mark.parametrize(
    "command, expected_func, cli_args",
    [
        ("add", do_add, ["add", "-t", "dummy", "dummy"]),
        ("remove", do_remove, ["remove", "-t", "dummy", "dummy"]),
        ("reset", do_reset, ["reset", "-t", "dummy"]),
        ("install", do_install, ["install", "-t", "dummy", "dummy"]),
        ("clear", do_clear, ["clear", "-t", "dummy"]),
        ("uninstall", do_uninstall, ["uninstall", "-t", "dummy", "dummy"]),
        ("list-products", do_list_products, ["list-products", "-t", "dummy"]),
        ("list-repos", do_list_repos, ["list-repos", "-t", "dummy"]),
        ("known-products", do_known_products, ["known-products"]),
    ],
)
def test_command_functions(mock_types, command, expected_func, cli_args):
    parser = get_parser()
    args = parser.parse_args(cli_args)
    assert args.func == expected_func


# ---------------------------------------------------------------------------
# Extended coverage: defaults, mutually exclusive group, missing args.
# ---------------------------------------------------------------------------


def test_dry_default_is_false(mock_types):
    args = get_parser().parse_args(["add", "-t", "h", "x"])
    assert args.dry is False


def test_dry_short_flag_sets_true(mock_types):
    args = get_parser().parse_args(["-n", "add", "-t", "h", "x"])
    assert args.dry is True


def test_print_long_flag_sets_dry(mock_types):
    args = get_parser().parse_args(["--print", "add", "-t", "h", "x"])
    assert args.dry is True


def test_config_default_path():
    from pathlib import Path

    parser = get_parser()
    args = parser.parse_args(["known-products"])
    assert args.config == Path("/etc/repose/products.yml")


def test_config_override():
    from pathlib import Path

    args = get_parser().parse_args(["-c", "/tmp/my.yml", "known-products"])
    assert args.config == Path("/tmp/my.yml")


def test_debug_and_quiet_mutually_exclusive():
    with pytest.raises(SystemExit):
        get_parser().parse_args(["-d", "-q", "known-products"])


def test_target_required_for_add():
    with pytest.raises(SystemExit):
        # missing -t
        get_parser().parse_args(["add", "x"])


def test_repa_required_positional_for_add(mock_types):
    with pytest.raises(SystemExit):
        get_parser().parse_args(["add", "-t", "h"])


def test_multiple_targets(mock_types):
    args = get_parser().parse_args(["add", "-t", "h1", "-t", "h2", "x"])
    assert args.target == ["h1", "h2"]


def test_multiple_repa_args(mock_types):
    args = get_parser().parse_args(["add", "-t", "h", "x", "y", "z"])
    assert args.repa == ["x", "y", "z"]


def test_list_products_yaml_flag():
    args = get_parser().parse_args(["list-products", "-t", "h", "--yaml"])
    assert args.yaml is True


def test_list_products_yaml_default_false():
    args = get_parser().parse_args(["list-products", "-t", "h"])
    assert args.yaml is False


# Smoke tests for do_* dispatchers — make sure they import the right
# command class and call .run().


@pytest.mark.parametrize(
    "do_func,class_name",
    [
        (do_add, "Add"),
        (do_remove, "Remove"),
        (do_install, "Install"),
        (do_uninstall, "Uninstall"),
        (do_clear, "Clear"),
        (do_reset, "Reset"),
        (do_list_products, "ListProducts"),
        (do_list_repos, "ListRepos"),
        (do_known_products, "KnownProducts"),
    ],
)
def test_do_funcs_invoke_command_run(monkeypatch, do_func, class_name):
    """Each ``do_*`` instantiates the named command class and calls run()."""
    from unittest.mock import MagicMock
    import repose.command as cmd_pkg

    instance = MagicMock()
    klass = MagicMock(return_value=instance)
    # do_* funcs do `from repose.command import <Class>` — patch the
    # name on the package namespace.
    monkeypatch.setattr(cmd_pkg, class_name, klass)

    do_func("the-args")

    klass.assert_called_once_with("the-args")
    instance.run.assert_called_once()
