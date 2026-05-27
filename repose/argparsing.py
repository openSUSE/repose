import argparse
import logging
from pathlib import Path

import repose.command  # noqa: F401 — populate Command.registry
from repose import __version__
from repose.command import Command

from .host import ParseHosts
from .types.repa import Repa

logger = logging.getLogger("repose.arg")


def get_parser():
    """
    Process the parsed arguments and return the result
    :param argv: passed arguments
    """

    parser = argparse.ArgumentParser(
        description="Repository manipulation tool for QAM", prog="repose"
    )

    parser.add_argument(
        "-n",
        "--print",
        dest="dry",
        action="store_true",
        help="print commands for host and exit",
    )
    parser.add_argument(
        "-V",
        "--version",
        action="version",
        version=f"%(prog)s version: {__version__}",
    )
    parser.add_argument(
        "-c",
        "--config",
        action="store",
        type=Path,
        help="path to repose configuration",
        default=Path("/etc/repose/products.yml"),
    )
    group = parser.add_mutually_exclusive_group()
    group.add_argument(
        "-d", "--debug", action="store_true", help="enable debug logging"
    )
    group.add_argument(
        "-q", "--quiet", action="store_true", help="suppress messages from repose"
    )
    parser.add_argument(
        "--no-color",
        action="store_true",
        help="disable ANSI color in console output (honors NO_COLOR)",
    )
    parser.add_argument(
        "--format",
        choices=["text", "json"],
        default="text",
        help="console output format: 'text' (default) or 'json' (one event per line)",
    )

    commands = parser.add_subparsers()

    def add_subparser(name, help_text, arguments=None):
        """Create a subparser dispatched via ``Command.registry``.

        The CLI ``name`` must match the ``name=`` kwarg used when the
        corresponding ``Command`` subclass is declared.
        """
        if arguments is None:
            arguments = []
        subparser = commands.add_parser(name, help=help_text)
        if "target" in arguments:
            subparser.add_argument(
                "-t",
                "--target",
                metavar="HOST",
                type=ParseHosts,
                action="append",
                required=True,
                help="target to operate on",
            )
        if "repa" in arguments:
            subparser.add_argument(
                "repa",
                metavar="REPA",
                nargs="+",
                type=Repa,
                help="REPA pattern specification for needed repository",
            )
        # Late-bind via default arg to dodge the closure late-binding
        # trap, and resolve through the registry at call time so tests
        # can monkeypatch ``Command.registry`` entries.
        subparser.set_defaults(
            func=lambda args, _n=name: Command.registry[_n](args).run()
        )
        return subparser

    # command ADD
    add_subparser("add", "add specified repository to target", ["target", "repa"])

    # command REMOVE
    add_subparser("remove", "remove repository from target", ["target", "repa"])

    # command RESET
    add_subparser(
        "reset",
        "reset target repositories to only installed products repositories",
        ["target"],
    )

    # command INSTALL
    add_subparser(
        "install",
        "add specified repository to target and install product",
        ["target", "repa"],
    )

    # command CLEAR
    add_subparser("clear", "clear all repositories from target", ["target"])

    # command Uninstall
    add_subparser(
        "uninstall",
        "remove specified repository from target and uninstall product",
        ["target", "repa"],
    )

    # command LIST-Products
    cmdlistp = add_subparser("list-products", "list products on target", ["target"])
    glistp = cmdlistp.add_mutually_exclusive_group()
    glistp.add_argument(
        "--yaml",
        action="store_true",
        help="Generate YAML host spec for refhosts.yml generator without normalization. Default for SLE 12-SP5 and SLE 15-SP3+ products",
    )

    # command LIST-Repos
    add_subparser("list-repos", "list repositories on target", ["target"])

    # command KnownProducts
    add_subparser("known-products", "list known products by 'repose'")

    return parser
