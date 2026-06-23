"""Typer-based CLI for ``repose``.

Replaces the hand-rolled ``argparse`` driver. Each Typer command maps
1:1 to a ``Command`` subclass via ``Command.registry``; the call shape
(``Command(args).run()``) is preserved by constructing an
``argparse.Namespace`` shim per invocation. Global flags live on a
``CliGlobals`` dataclass attached to ``ctx.obj`` so subcommand
functions can read them at dispatch time.

Why a Namespace shim instead of refactoring ``Command.__init__``:
the latter would touch every command subclass and every command-layer
test. The shim is surgical and matches the layout this PR replaces.
"""

import argparse
import logging
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Annotated, Callable, Optional

import typer
from ruamel.yaml import YAMLError

import repose.command  # noqa: F401  — populate Command.registry
from repose import __version__
from repose.colorlog import create_logger
from repose.command import Command
from repose.host import ParseHosts
from repose.template import load_template
from repose.types.connection_config import ConnectionConfig
from repose.types.repa import Repa

logger = logging.getLogger("repose.cli")


# ---------------------------------------------------------------------------
# Choice enums (Typer auto-renders these as ``--flag {a,b,c}`` and
# validates the value at parse time — equivalent to argparse
# ``choices=[...]`` without depending on click directly).
# ---------------------------------------------------------------------------


class OutputFormat(str, Enum):
    text = "text"
    json = "json"


class HostKeyPolicyChoice(str, Enum):
    yes = "yes"
    accept_new = "accept-new"
    no = "no"
    off = "off"


class SSHBackendChoice(str, Enum):
    asyncssh = "asyncssh"
    paramiko = "paramiko"


# ---------------------------------------------------------------------------
# Global state carrier
# ---------------------------------------------------------------------------


@dataclass
class CliGlobals:
    """Snapshot of every top-level option, captured in the root callback.

    Subcommand functions read these out of ``ctx.obj`` and stitch them
    into the ``Namespace`` they hand to ``Command``.
    """

    dry: bool = False
    config: Path = Path("/etc/repose/products.yml")
    debug: bool = False
    quiet: bool = False
    no_color: bool = False
    format: str = "text"
    strict_host_key_checking: str = "accept-new"
    known_hosts: Optional[Path] = None
    ssh_backend: str = "asyncssh"
    # The ``ConnectionConfig`` derived from ``strict_host_key_checking``
    # + ``known_hosts`` + ``ssh_backend``; threaded into ``ParseHosts``
    # so ``-t`` parsing builds ``Target``/``AsyncTarget``s with the
    # correct transport policy.
    conn_config: ConnectionConfig = ConnectionConfig()


# ---------------------------------------------------------------------------
# Typer parser callables
# ---------------------------------------------------------------------------


def _target_parser_factory(cfg: ConnectionConfig) -> Callable[[str], ParseHosts]:
    """Return a one-arg callable Typer uses to parse each ``-t`` value.

    Typer calls ``parser(token)`` once per occurrence and collects into
    the declared ``list[...]``; mirrors argparse's ``type=`` +
    ``action="append"`` semantics. The captured ``cfg`` is the
    ``ConnectionConfig`` materialised from the SSH transport globals.
    """
    factory = ParseHosts(cfg)
    return factory


# Module-level reference so the per-subcommand target parser can be
# rebuilt against the *current* ConnectionConfig set by the root
# callback. Typer's `parser=` is evaluated lazily on first use, but we
# need each invocation to see fresh globals (tests reuse the app across
# many ``CliRunner.invoke`` calls). We solve this by exposing
# ``_current_target_parser`` as a function that looks up the live
# ConnectionConfig on the active ``typer.Context``.
def _target_parser(value: str) -> ParseHosts:
    """Parser shim invoked per ``-t`` token.

    Reads the active ``ConnectionConfig`` from the live click context.
    Falls back to a default ``ConnectionConfig`` if no context (e.g.
    during introspection or when invoked outside the CLI).

    The ``get_current_context`` import path differs across Typer
    releases: Typer >=0.26 vendors click and pushes its context onto a
    private stack reachable only via ``typer._click.globals`` (plain
    ``click.get_current_context`` returns ``None`` there), while Typer
    0.16 uses real click and exposes the context through
    ``click.get_current_context``. Prefer the vendored namespace and
    fall back to plain click when it's absent (e.g. Leap 16's Typer
    0.16.0).
    """
    try:
        from typer._click.globals import get_current_context
    except ModuleNotFoundError:
        from click import get_current_context

    try:
        ctx = get_current_context(silent=True)
    except (LookupError, RuntimeError):
        ctx = None
    if ctx is not None and isinstance(getattr(ctx, "obj", None), CliGlobals):
        cfg = ctx.obj.conn_config
    else:
        cfg = ConnectionConfig()
    return _target_parser_factory(cfg)(value)


def _repa_parser(value: str) -> Repa:
    """Parser shim for each ``REPA`` positional token."""
    return Repa(value)


def _complete_repa(ctx: typer.Context, incomplete: str) -> list[str]:
    """Shell-completion callback for ``REPA`` positional arguments.

    Reads the products YAML pointed at by ``-c/--config`` (or the
    default ``/etc/repose/products.yml``) and returns product-name
    prefixes that match ``incomplete``. Completes only the first
    colon-separated segment (the product); subsequent segments
    (``:VERSION:ARCH:REPO``) are left to free-form user input.

    Any failure to read or parse the YAML — missing file, permission
    error, malformed YAML — collapses to an empty completion list so
    the user's shell never raises mid-keystroke.
    """
    # The root callback populates ``ctx.obj``; during completion it
    # has typically already run. Fall back gracefully if not.
    config_path: Path | None = None
    obj = getattr(ctx, "obj", None)
    if isinstance(obj, CliGlobals):
        config_path = obj.config
    if config_path is None:
        config_path = Path("/etc/repose/products.yml")
    try:
        template = load_template(config_path)
    except (OSError, YAMLError):
        return []
    # Only complete the first ``:``-separated segment (product name).
    # If the user has already typed past a colon, return nothing so we
    # don't clobber the version/arch/repo they're filling in by hand.
    if ":" in incomplete:
        return []
    return sorted(name for name in template.keys() if name.startswith(incomplete))


# ---------------------------------------------------------------------------
# Dispatch helpers
# ---------------------------------------------------------------------------


def _build_ns(ctx: typer.Context, **subcmd_kwargs: object) -> argparse.Namespace:
    """Stitch globals + subcommand args into a Namespace for ``Command``.

    ``Command.__init__`` introspects this with ``"target" in args``
    (delegating to ``Namespace.__contains__``) and ``getattr(args, ...)``
    calls. Building a plain ``argparse.Namespace`` keeps the API
    contract intact without touching the command layer.
    """
    g = ctx.obj
    assert isinstance(g, CliGlobals), "root callback must run first"
    ns = argparse.Namespace(
        dry=g.dry,
        config=g.config,
        debug=g.debug,
        quiet=g.quiet,
        no_color=g.no_color,
        format=g.format,
        strict_host_key_checking=g.strict_host_key_checking,
        known_hosts=g.known_hosts,
        ssh_backend=g.ssh_backend,
        **subcmd_kwargs,
    )
    return ns


def _dispatch(name: str, ns: argparse.Namespace) -> int:
    """Run ``Command.registry[name](ns).run()`` with friendly errors.

    Translates the most common config-load failures into a one-line
    log message + exit code 2; ``--debug`` re-raises so contributors
    see the full traceback. ``KeyboardInterrupt`` collapses to the
    conventional ``128 + SIGINT (2) = 130``.
    """
    cls = Command.registry[name]
    try:
        rc = cls(ns).run()
    except FileNotFoundError as e:
        if ns.debug:
            raise
        logger.error(
            "config file not found: %s (use -c PATH to point at another)",
            e.filename or "<unknown>",
        )
        return 2
    except PermissionError as e:
        if ns.debug:
            raise
        logger.error("permission denied reading config: %s", e.filename or "<unknown>")
        return 2
    except IsADirectoryError as e:
        if ns.debug:
            raise
        logger.error(
            "config path is a directory, not a file: %s",
            e.filename or "<unknown>",
        )
        return 2
    except YAMLError as e:
        if ns.debug:
            raise
        logger.error("invalid YAML in config: %s", e)
        return 2
    except KeyboardInterrupt:
        logger.error("interrupted")
        return 130
    return int(rc) if rc is not None else 0


# ---------------------------------------------------------------------------
# Typer app
# ---------------------------------------------------------------------------


app = typer.Typer(
    help="Repository manipulation tool for QAM",
    add_completion=True,
    # Subcommand names use kebab-case (e.g. ``list-products``) which
    # argparse exposed natively. Typer needs ``no_args_is_help=False``
    # because we render usage explicitly in the callback so bare
    # ``repose`` matches the legacy stdout/exit-0 behaviour.
    no_args_is_help=False,
    rich_markup_mode=None,
    context_settings={"help_option_names": ["-h", "--help"]},
)


def _version_callback(value: bool) -> None:
    if value:
        typer.echo(f"repose version: {__version__}")
        raise typer.Exit()


@app.callback(invoke_without_command=True)
def main_callback(
    ctx: typer.Context,
    dry: Annotated[
        bool,
        typer.Option(
            "-n",
            "--print",
            help="print commands for host and exit",
        ),
    ] = False,
    version: Annotated[
        bool,
        typer.Option(
            "-V",
            "--version",
            callback=_version_callback,
            is_eager=True,
            help="show program's version number and exit",
        ),
    ] = False,
    config: Annotated[
        Path,
        typer.Option(
            "-c",
            "--config",
            help="path to repose configuration",
        ),
    ] = Path("/etc/repose/products.yml"),
    debug: Annotated[
        bool,
        typer.Option(
            "-d",
            "--debug",
            help="enable debug logging",
        ),
    ] = False,
    quiet: Annotated[
        bool,
        typer.Option(
            "-q",
            "--quiet",
            help="suppress messages from repose",
        ),
    ] = False,
    no_color: Annotated[
        bool,
        typer.Option(
            "--no-color",
            help="disable ANSI color in console output (honors NO_COLOR)",
        ),
    ] = False,
    format_: Annotated[
        OutputFormat,
        typer.Option(
            "--format",
            case_sensitive=False,
            help=(
                "console output format: 'text' (default) or 'json' (one event per line)"
            ),
        ),
    ] = OutputFormat.text,
    strict_host_key_checking: Annotated[
        HostKeyPolicyChoice,
        typer.Option(
            "--strict-host-key-checking",
            case_sensitive=False,
            help=(
                "SSH host-key policy (OpenSSH semantics): "
                "'yes' refuses unknown hosts; "
                "'accept-new' (default) accepts unknown hosts on first "
                "contact but rejects changed keys; "
                "'no'/'off' accepts both unknown and changed keys "
                "(pre-1.12 behaviour)"
            ),
        ),
    ] = HostKeyPolicyChoice.accept_new,
    known_hosts: Annotated[
        Optional[Path],
        typer.Option(
            "--known-hosts",
            help="path to a custom known_hosts file (overrides ~/.ssh/known_hosts)",
        ),
    ] = None,
    ssh_backend: Annotated[
        SSHBackendChoice,
        typer.Option(
            "--ssh-backend",
            case_sensitive=False,
            help=(
                "SSH backend implementation: "
                "'asyncssh' (default, structured concurrency, no thread "
                "pool) or 'paramiko' (legacy, available for one release "
                "as a safety net while asyncssh settles)"
            ),
        ),
    ] = SSHBackendChoice.asyncssh,
) -> None:
    """Repository manipulation tool for QAM."""
    # Mutual exclusion: argparse used a ``mutually_exclusive_group``;
    # Typer has no native equivalent so we enforce it here. Matching the
    # argparse exit-code convention (parser errors exit 2 via
    # ``typer.BadParameter``).
    if debug and quiet:
        raise typer.BadParameter(
            "argument -q/--quiet: not allowed with argument -d/--debug"
        )

    # Collapse the Enums back to plain strings so the rest of the
    # codebase (Console init, command logic, ConnectionConfig, tests)
    # keeps comparing against ``"json"`` / ``"yes"`` and friends.
    fmt_str = format_.value
    hkp_str = strict_host_key_checking.value
    backend_str = ssh_backend.value
    cfg = ConnectionConfig(
        host_key_policy=hkp_str,  # type: ignore[arg-type]
        known_hosts=known_hosts,
        ssh_backend=backend_str,  # type: ignore[arg-type]
    )
    ctx.obj = CliGlobals(
        dry=dry,
        config=config,
        debug=debug,
        quiet=quiet,
        no_color=no_color,
        format=fmt_str,
        strict_host_key_checking=hkp_str,
        known_hosts=known_hosts,
        ssh_backend=backend_str,
        conn_config=cfg,
    )

    # Configure root logger level from -d/-q. This used to live in
    # ``main.py`` after parsing; the Typer port relocates it here
    # because the callback runs before any subcommand body.
    root_logger = create_logger("repose")
    if debug:
        root_logger.setLevel("DEBUG")
    elif quiet:
        root_logger.setLevel("WARNING")

    if ctx.invoked_subcommand is None:
        # Bare ``repose`` invocation: render top-level help on stdout
        # (matching the prior ``argparse.print_usage()`` behaviour) and
        # exit 0. ``typer.echo`` writes to stdout by default.
        typer.echo(ctx.get_help())
        raise typer.Exit(0)


# ---------------------------------------------------------------------------
# Subcommands
# ---------------------------------------------------------------------------


# Common annotated parameter aliases — avoid repeating the long
# ``Annotated[...]`` form on every subcommand.
TargetOpt = Annotated[
    list[ParseHosts],
    typer.Option(
        "-t",
        "--target",
        metavar="HOST",
        parser=_target_parser,
        help="target to operate on",
    ),
]
RepaArg = Annotated[
    list[Repa],
    typer.Argument(
        metavar="REPA",
        parser=_repa_parser,
        help="REPA pattern specification for needed repository",
        autocompletion=_complete_repa,
    ),
]
ProbeTimeoutOpt = Annotated[
    float,
    typer.Option(
        "--probe-timeout",
        metavar="SECONDS",
        help="seconds to wait per repository URL probe (default: 5)",
    ),
]
NoProbeOpt = Annotated[
    bool,
    typer.Option(
        "--no-probe",
        help="skip repository URL liveness probes",
    ),
]
NoRebootOpt = Annotated[
    bool,
    typer.Option(
        "--no-reboot",
        help=(
            "on transactional hosts (SL Micro), stage the package change "
            "but do not reboot/reconnect/verify (default: reboot)"
        ),
    ),
]


@app.command("add", help="add specified repository to target")
def add_cmd(
    ctx: typer.Context,
    target: TargetOpt,
    repa: RepaArg,
    probe_timeout: ProbeTimeoutOpt = 5.0,
    no_probe: NoProbeOpt = False,
) -> None:
    ns = _build_ns(
        ctx,
        target=target,
        repa=repa,
        probe_timeout=probe_timeout,
        no_probe=no_probe,
    )
    raise typer.Exit(_dispatch("add", ns))


@app.command("remove", help="remove repository from target")
def remove_cmd(
    ctx: typer.Context,
    target: TargetOpt,
    repa: RepaArg,
) -> None:
    ns = _build_ns(ctx, target=target, repa=repa)
    raise typer.Exit(_dispatch("remove", ns))


@app.command(
    "reset",
    help="reset target repositories to only installed products repositories",
)
def reset_cmd(
    ctx: typer.Context,
    target: TargetOpt,
    probe_timeout: ProbeTimeoutOpt = 5.0,
    no_probe: NoProbeOpt = False,
) -> None:
    ns = _build_ns(
        ctx,
        target=target,
        probe_timeout=probe_timeout,
        no_probe=no_probe,
    )
    raise typer.Exit(_dispatch("reset", ns))


@app.command("install", help="add specified repository to target and install product")
def install_cmd(
    ctx: typer.Context,
    target: TargetOpt,
    repa: RepaArg,
    probe_timeout: ProbeTimeoutOpt = 5.0,
    no_probe: NoProbeOpt = False,
    no_reboot: NoRebootOpt = False,
) -> None:
    ns = _build_ns(
        ctx,
        target=target,
        repa=repa,
        probe_timeout=probe_timeout,
        no_probe=no_probe,
        no_reboot=no_reboot,
    )
    raise typer.Exit(_dispatch("install", ns))


@app.command("clear", help="clear all repositories from target")
def clear_cmd(
    ctx: typer.Context,
    target: TargetOpt,
) -> None:
    ns = _build_ns(ctx, target=target)
    raise typer.Exit(_dispatch("clear", ns))


@app.command(
    "uninstall",
    help="remove specified repository from target and uninstall product",
)
def uninstall_cmd(
    ctx: typer.Context,
    target: TargetOpt,
    repa: RepaArg,
    no_reboot: NoRebootOpt = False,
) -> None:
    ns = _build_ns(ctx, target=target, repa=repa, no_reboot=no_reboot)
    raise typer.Exit(_dispatch("uninstall", ns))


@app.command("list-products", help="list products on target")
def list_products_cmd(
    ctx: typer.Context,
    target: TargetOpt,
    yaml: Annotated[
        bool,
        typer.Option(
            "--yaml",
            help=(
                "Generate YAML host spec for refhosts.yml generator without "
                "normalization. Default for SLE 12-SP5 and SLE 15-SP3+ "
                "products"
            ),
        ),
    ] = False,
) -> None:
    ns = _build_ns(ctx, target=target, yaml=yaml)
    raise typer.Exit(_dispatch("list-products", ns))


@app.command("list-repos", help="list repositories on target")
def list_repos_cmd(
    ctx: typer.Context,
    target: TargetOpt,
) -> None:
    ns = _build_ns(ctx, target=target)
    raise typer.Exit(_dispatch("list-repos", ns))


@app.command("known-products", help="list known products by 'repose'")
def known_products_cmd(ctx: typer.Context) -> None:
    ns = _build_ns(ctx)
    raise typer.Exit(_dispatch("known-products", ns))
