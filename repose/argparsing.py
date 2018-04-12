

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


def get_parser():
    """
    Process the parsed arguments and return the result
    :param argv: passed arguments
    """

    parser = argparse.ArgumentParser(
        description="repository manipulation tool for QAM", prog="repose")

    parser.add_argument(
        "-n",
        "--print",
        dest='dry',
        action="store_true",
        help="Print commnads for hosts, and exit")
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
        help="Patch to repose config",
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

    cmdadd = commands.add_parser('add', help='Adds specified repos to targets')
    cmdadd.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action="append",
        required=True,
        help='Target to operate on')
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
        help='Removes repositories from targets')
    cmdremove.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="Target to operate on")
    cmdremove.add_argument("repa", metavar="REPA", type=Repa, nargs="+")
    cmdremove.set_defaults(func=do_remove)

    # command RESET

    cmdreset = commands.add_parser(
        'reset',
        help="Reset target repos to only installed products repositories")
    cmdreset.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="Target to operate on")
    cmdreset.set_defaults(func=do_reset)

    # command INSTALL

    cmdinstall = commands.add_parser(
        'install',
        help='Adds specified repos from target and install product')
    cmdinstall.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="Target to operate on")
    cmdinstall.add_argument("repa", metavar="REPA", type=Repa, nargs="+")
    cmdinstall.set_defaults(func=do_install)

    # command CLEAR

    cmdclear = commands.add_parser(
        'clear',
        help="Clear all repositories from target")
    cmdclear.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="Target to operate on")
    cmdclear.set_defaults(func=do_clear)

    # command Unistall

    cmduninstall = commands.add_parser(
        'uninstall',
        help='Removes specified repos to target and uninstall product')
    cmduninstall.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="Target to operate on")
    cmduninstall.add_argument("repa", metavar="REPA", type=Repa, nargs="+")
    cmduninstall.set_defaults(func=do_uninstall)

    # command LIST-Products

    cmdlistp = commands.add_parser(
        'list-products',
        help="List products on target")
    cmdlistp.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="Target to operate on")
    cmdlistp.set_defaults(func=do_list_products)

    # command LIST-Repos

    cmdlistr = commands.add_parser(
        'list-repos',
        help="List repositories on target")
    cmdlistr.add_argument(
        "-t",
        "--target",
        metavar="HOST",
        type=ParseHosts,
        action='append',
        required=True,
        help="Target to operate on")
    cmdlistr.set_defaults(func=do_list_repos)

    return parser
