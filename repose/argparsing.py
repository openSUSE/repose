import argparse
import logging
from pathlib import Path

from repose import __version__

from .host import ParseHosts
from .types.repa import Repa

logger = logging.getLogger("repose.arg")


def do_install(args):
    from repose.command import Install

    Install(args).run()


def do_list_products(args):
    from repose.command import ListProducts

    ListProducts(args).run()


def do_list_repos(args):
    from repose.command import ListRepos

    ListRepos(args).run()


def do_remove(args):
    from repose.command import Remove

    Remove(args).run()


def do_clear(args):
    from repose.command import Clear

    Clear(args).run()


def do_uninstall(args):
    from repose.command import Uninstall

    Uninstall(args).run()


def do_add(args):
    from repose.command import Add

    Add(args).run()


def do_reset(args):
    from repose.command import Reset

    Reset(args).run()


def do_known_products(args):
    from repose.command import KnownProducts

    KnownProducts(args).run()


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
        version="%(prog)s version: {}".format(__version__),
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

    commands = parser.add_subparsers()

    def add_subparser(name, help_text, func, arguments=None):
        """Helper to create a subparser and add common arguments."""
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
        subparser.set_defaults(func=func)
        return subparser

    # command ADD
    add_subparser(
        "add", "add specified repository to target", do_add, ["target", "repa"]
    )

    # command REMOVE
    add_subparser(
        "remove", "remove repository from target", do_remove, ["target", "repa"]
    )

    # command RESET
    add_subparser(
        "reset",
        "reset target repositories to only installed products repositories",
        do_reset,
        ["target"],
    )

    # command INSTALL
    add_subparser(
        "install",
        "add specified repository to target and install product",
        do_install,
        ["target", "repa"],
    )

    # command CLEAR
    add_subparser("clear", "clear all repositories from target", do_clear, ["target"])

    # command Uninstall
    add_subparser(
        "uninstall",
        "remove specified repository from target and uninstall product",
        do_uninstall,
        ["target", "repa"],
    )

    # command LIST-Products
    cmdlistp = add_subparser(
        "list-products", "list products on target", do_list_products, ["target"]
    )
    glistp = cmdlistp.add_mutually_exclusive_group()
    glistp.add_argument(
        "--yaml",
        action="store_true",
        help="Generate YAML host spec for refhosts.yml generator without normalization. Default for SLE 12-SP5 and SLE 15-SP3+ products",
    )

    # command LIST-Repos
    add_subparser(
        "list-repos", "list repositories on target", do_list_repos, ["target"]
    )

    # command KnownProducts
    add_subparser(
        "known-products", "list known products by 'repose'", do_known_products
    )

    return parser
