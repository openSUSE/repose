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
