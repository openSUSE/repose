

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
        description="Repository manipulation tool for QAM", prog="repose")

    parser.add_argument(
        "-n",
        "--print",
        dest='dry',
        action="store_true",
        help="print commands for host and exit")
    parser.add_argument(
        "-V",
        "--version",
        action="version",
        version="%(prog)s version: {}".format(__version__))
    parser.add_argument(
        "-c",
        "--config",
        action="store",
        type=Path,
        help="path to repose configuration",
        default=Path('/etc/repose/products.yml'))
    group = parser.add_mutually_exclusive_group()
    group.add_argument(
        "-d",
        "--debug",
        action="store_true",
        help="enable debug logging")
    group.add_argument(
        "-q",
        "--quiet",
        action="store_true",
        help="suppress messages from repose")

    commands = parser.add_subparsers()

    # command ADD

    cmdadd = commands.add_parser('add', help='add specified repository to target')
    cmdadd.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action="append",
        required=True,
        help='target to operate on')
    cmdadd.add_argument(
        "repa",
        metavar="REPA",
        nargs="+",
        type=Repa,
        help='REPA pattern specification for needed repository')
    cmdadd.set_defaults(func=do_add)

    # command REMOVE

    cmdremove = commands.add_parser(
        'remove',
        help='remove repository from target')
    cmdremove.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="target to operate on")
    cmdremove.add_argument("repa", metavar="REPA", type=Repa, nargs="+")
    cmdremove.set_defaults(func=do_remove)

    # command RESET

    cmdreset = commands.add_parser(
        'reset',
        help="reset target repositories to only installed products repositories")
    cmdreset.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="target to operate on")
    cmdreset.set_defaults(func=do_reset)

    # command INSTALL

    cmdinstall = commands.add_parser(
        'install',
        help='add specified repository to target and install product')
    cmdinstall.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="target to operate on")
    cmdinstall.add_argument("repa", metavar="REPA", type=Repa, nargs="+")
    cmdinstall.set_defaults(func=do_install)

    # command CLEAR

    cmdclear = commands.add_parser(
        'clear',
        help="clear all repositories from target")
    cmdclear.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="target to operate on")
    cmdclear.set_defaults(func=do_clear)

    # command Unistall

    cmduninstall = commands.add_parser(
        'uninstall',
        help='remove specified repository from target and uninstall product')
    cmduninstall.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="target to operate on")
    cmduninstall.add_argument("repa", metavar="REPA", type=Repa, nargs="+")
    cmduninstall.set_defaults(func=do_uninstall)

    # command LIST-Products

    cmdlistp = commands.add_parser(
        'list-products',
        help="list products on target")
    cmdlistp.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="target to operate on")
    glistp = cmdlistp.add_mutually_exclusive_group()
    glistp.add_argument(
        "--yaml-ng",
        action='store_true',
        help="Generate YAML host spec for refhosts.yml generator without normalization. Default for SLE 12-SP5 and SLE 15-SP3+ products")
    glistp.add_argument(
        "--yaml",
        action='store_true',
        help="Generate YAML host spec for refhosts.yml generator. Don't use with new products")
    cmdlistp.set_defaults(func=do_list_products)

    # command LIST-Repos

    cmdlistr = commands.add_parser(
        'list-repos',
        help="list repositories on target")
    cmdlistr.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="target to operate on")
    cmdlistr.set_defaults(func=do_list_repos)

    # command KnownProducts

    cmdknown = commands.add_parser(
        'known-products',
        help="list known products by 'repose'")
    cmdknown.set_defaults(func=do_known_products)

    return parser
