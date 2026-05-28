import argparse
import logging
from pathlib import Path

import repose.command  # noqa: F401 — populate Command.registry
from repose import __version__
from repose.command import Command

from .host import ParseHosts
from .types.connection_config import ConnectionConfig
from .types.repa import Repa

logger = logging.getLogger("repose.arg")


def _globals_parser() -> argparse.ArgumentParser:
    """Tiny parser that only knows the SSH transport globals.

    Used by ``parse()`` for a non-failing first pass that extracts
    ``--strict-host-key-checking`` and ``--known-hosts`` before the
    real parser is built. ``parse_known_args`` on this parser ignores
    every other token, including subcommands and their flags.
    """
    p = argparse.ArgumentParser(add_help=False)
    p.add_argument(
        "--strict-host-key-checking",
        choices=["yes", "accept-new", "no", "off"],
        default="accept-new",
        dest="strict_host_key_checking",
    )
    p.add_argument("--known-hosts", type=Path, default=None, dest="known_hosts")
    return p


def build_config_from_args(args: argparse.Namespace) -> ConnectionConfig:
    """Materialise a ``ConnectionConfig`` from a parsed ``Namespace``."""
    return ConnectionConfig(
        host_key_policy=getattr(args, "strict_host_key_checking", "accept-new"),
        known_hosts=getattr(args, "known_hosts", None),
    )


def parse(argv: list[str]) -> argparse.Namespace:
    """Two-pass argparse driver used by ``main.py``.

    First pass: extract the SSH transport globals so the
    ``ParseHosts`` factory can be configured before the *real* parser
    binds it as ``type=``. Second pass: full parser, returns the
    fully-populated ``Namespace``.

    Tests that previously called ``get_parser().parse_args(...)``
    continue to work — they receive the parser built with a default
    ``ConnectionConfig()``.
    """
    pre_args, _ = _globals_parser().parse_known_args(argv)
    cfg = build_config_from_args(pre_args)
    return get_parser(cfg).parse_args(argv)


def get_parser(config: ConnectionConfig | None = None):
    """Build the full argparse parser.

    ``config`` configures the ``ParseHosts`` factory bound to the
    ``-t/--target`` flag. When omitted a default ``ConnectionConfig``
    is used; this keeps the historical ``get_parser()`` no-arg call
    shape working for tests that don't care about transport policy.
    """
    if config is None:
        config = ConnectionConfig()

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
    parser.add_argument(
        "--strict-host-key-checking",
        choices=["yes", "accept-new", "no", "off"],
        default="accept-new",
        dest="strict_host_key_checking",
        help=(
            "SSH host-key policy (OpenSSH semantics): "
            "'yes' refuses unknown hosts; "
            "'accept-new' (default) accepts unknown hosts on first "
            "contact but rejects changed keys; "
            "'no'/'off' accepts both unknown and changed keys "
            "(pre-1.12 behaviour)"
        ),
    )
    parser.add_argument(
        "--known-hosts",
        type=Path,
        default=None,
        dest="known_hosts",
        help="path to a custom known_hosts file (overrides ~/.ssh/known_hosts)",
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
            # Late-bind via the module attribute so test fixtures that
            # monkeypatch ``repose.argparsing.ParseHosts`` to a plain
            # ``lambda x: x`` keep working unchanged: we call whatever
            # ``ParseHosts`` is *at parse time*, not what it was at
            # parser-build time. The real factory class is detected by
            # ``isinstance`` against the resolved type so swapping in a
            # different callable (test stub, future replacement) still
            # works as a one-arg ``type=`` adapter.
            def _host_type(host_str, _cfg=config):
                import repose.argparsing as _mod

                ph = _mod.ParseHosts
                if isinstance(ph, type) and issubclass(ph, ParseHosts):
                    # Real factory class: configure with cfg, then call
                    # the resulting instance with the host string.
                    return ph(_cfg)(host_str)
                # Test stub or any other one-arg callable.
                return ph(host_str)

            subparser.add_argument(
                "-t",
                "--target",
                metavar="HOST",
                type=_host_type,
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
        if "probe" in arguments:
            subparser.add_argument(
                "--probe-timeout",
                type=float,
                default=5.0,
                metavar="SECONDS",
                help="seconds to wait per repository URL probe (default: 5)",
            )
            subparser.add_argument(
                "--no-probe",
                action="store_true",
                help="skip repository URL liveness probes",
            )
        # Late-bind via default arg to dodge the closure late-binding
        # trap, and resolve through the registry at call time so tests
        # can monkeypatch ``Command.registry`` entries.
        subparser.set_defaults(
            func=lambda args, _n=name: Command.registry[_n](args).run()
        )
        return subparser

    # command ADD
    add_subparser(
        "add",
        "add specified repository to target",
        ["target", "repa", "probe"],
    )

    # command REMOVE
    add_subparser("remove", "remove repository from target", ["target", "repa"])

    # command RESET
    add_subparser(
        "reset",
        "reset target repositories to only installed products repositories",
        ["target", "probe"],
    )

    # command INSTALL
    add_subparser(
        "install",
        "add specified repository to target and install product",
        ["target", "repa", "probe"],
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
