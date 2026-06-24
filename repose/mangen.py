"""Generate man pages for ``repose`` and its subcommands.

Walks the Typer/Click app object and emits one ``.1`` man page per
command into ``docs/man/``. The output is committed to the repo so
downstream packagers don't need any man-gen tooling at build time; a CI
job regenerates and ``git diff --exit-code``s to catch drift.

This module hand-builds the roff itself rather than delegating to
``click_man.core.write_man_pages`` (the ``click-man`` package), which
produced nearly empty pages for our Typer CLI for two reasons:

1. *Options were silently dropped.* ``click-man`` collects options with
   ``isinstance(param, click.Option)``. Typer >=0.26 vendors its own
   copy of click, so a ``typer.core.TyperOption`` is **not** an instance
   of the top-level ``click.Option`` ``click-man`` imported — the filter
   matched nothing and every generated ``OPTIONS`` section was empty.
   We instead select option-like params by duck-typing
   (``get_help_record`` present, not an argument) and call
   ``get_help_record(ctx)`` ourselves, which works fine on the vendored
   classes. This mirrors the vendored-click workaround already in
   ``repose.cli._target_parser``.

2. *Descriptions were one-liners.* ``click-man`` set ``DESCRIPTION`` to
   ``command.help``, which for our terse Typer commands is just the
   one-line summary already shown in ``NAME``. We keep the CLI ``--help``
   terse and inject richer prose (multi-paragraph DESCRIPTION plus
   EXAMPLES / ENVIRONMENT / FILES / SEE ALSO sections) here at man-gen
   time from the ``_COMMAND_DOCS`` table below.

Reproducibility: the ``.TH`` header stamps a date. We pin it via
``SOURCE_DATE_EPOCH`` (reproducible-builds standard) so ``uv run
repose-mangen`` is byte-stable across machines and dates; CI uses this
to catch drift via ``git diff --exit-code``. If callers already export
``SOURCE_DATE_EPOCH`` (e.g. distro builds), their value wins.
"""

import os
import textwrap
import time
from contextlib import contextmanager
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterator

import click
import typer

from repose import __version__
from repose.cli import app

# Fallback epoch (UTC ``2024-01-01 00:00:00``) used when callers
# haven't already exported ``SOURCE_DATE_EPOCH``. Pinning a constant
# keeps ``uv run repose-mangen`` byte-stable across machines and dates;
# CI uses this to catch drift via ``git diff --exit-code``.
_FALLBACK_SOURCE_DATE_EPOCH = "1704067200"  # 2024-01-01T00:00:00Z


# ---------------------------------------------------------------------------
# Per-command documentation table
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class CommandDoc:
    """Rich man-page prose for one command.

    The CLI ``--help`` output stays terse (the one-line ``help=`` on each
    Typer command); these fields enrich only the generated man page.

    Attributes:
        description: Multi-paragraph DESCRIPTION body. Blank lines
            separate paragraphs. Overrides click's one-line help.
        examples: Lines for the EXAMPLES section, in order. Lines that
            start with ``repose`` are rendered as indented commands;
            other lines render as descriptive text above the command.
        see_also: Extra ``SEE ALSO`` references (bare command names such
            as ``install``). ``repose(1)`` and the sibling pages are
            added automatically.
    """

    description: str = ""
    examples: tuple[str, ...] = field(default_factory=tuple)
    see_also: tuple[str, ...] = field(default_factory=tuple)


# Shared prose fragments reused across several commands.
_REPA_SYNTAX = (
    "A REPA (REpository PAttern) selects which repositories to act on. "
    "Its colon-separated form is PRODUCT[:VERSION[:ARCH[:REPO]]], where "
    "PRODUCT is the product name from the configuration file, VERSION is "
    "an optional version such as 12-SP2, ARCH is an optional "
    "architecture, and REPO is an optional repository type (for example "
    "pool, update, or ltss) as defined in products.yml. Multiple REPA "
    "patterns may be given; known product names can be listed with "
    "repose known-products."
)

_PROBE_NOTE = (
    "Before applying changes, repose probes each candidate repository URL "
    "in parallel and drops repositories whose URLs do not respond. Probes "
    "honor the system trust store. Use --probe-timeout to tune the "
    "per-URL wait and --no-probe to skip probing entirely."
)

_TRANSACTIONAL_NOTE = (
    "On transactional/immutable hosts (SL Micro, SLE Micro, MicroOS) the "
    "change is routed through transactional-update; repose then reboots "
    "into the new snapshot, reconnects, and verifies the result before "
    "reporting success. Pass --no-reboot to stage the change without "
    "rebooting (a no-op on non-transactional hosts)."
)


_ROOT_DOC = CommandDoc(
    description=(
        "Repose queries and manipulates the package repositories of one "
        "or more SUSE QA Maintenance reference hosts over SSH, requiring "
        "only a running sshd and zypper on the target.\n"
        "\n"
        "For each host repose queries installed products and repository "
        "configuration, then runs the zypper (or, on transactional "
        "hosts, transactional-update) commands needed to satisfy the "
        "requested change.\n"
        "\n"
        "Global options precede the command name; per-command options "
        "follow it. See the per-command manual pages for details."
    ),
    examples=(
        "Set up repositories on a refhost and install a product:",
        "repose reset -t fubar.suse.cz",
        "repose install -t fubar.suse.cz qa",
        "Operate on multiple hosts at once:",
        "repose add -t fubar.suse.cz -t snafu.suse.cz qa sle-sdk",
        "Preview the commands without running them:",
        "repose -n add -t fubar.suse.cz sle-sdk",
    ),
)

_COMMAND_DOCS: dict[str, CommandDoc] = {
    "add": CommandDoc(
        description=(
            "Add the repositories selected by one or more REPA patterns "
            "to the target host(s) without installing any product.\n"
            "\n" + _REPA_SYNTAX + "\n"
            "\n" + _PROBE_NOTE
        ),
        examples=(
            "Add the SDK repository for any SLE version:",
            "repose add -t fubar.suse.cz sle-sdk",
            "Add the SDK repository for a specific version:",
            "repose add -t fubar.suse.cz sle-sdk:12-SP2",
            "Add multiple add-ons to multiple hosts:",
            "repose add -t fubar.suse.cz -t snafu.suse.cz qa sle-sdk",
            "Allow more time for slow URL probes:",
            "repose add --probe-timeout 10 -t fubar.suse.cz sle-sdk",
        ),
        see_also=("remove", "install", "reset"),
    ),
    "remove": CommandDoc(
        description=(
            "Remove the repositories selected by one or more REPA "
            "patterns from the target host(s). The product itself is left "
            "installed; use uninstall to also remove the product.\n"
            "\n" + _REPA_SYNTAX
        ),
        examples=(
            "Remove the SDK repository:",
            "repose remove -t fubar.suse.cz sle-sdk",
        ),
        see_also=("add", "uninstall", "clear"),
    ),
    "reset": CommandDoc(
        description=(
            "Reset the target host(s) so that only the repositories of "
            "the currently installed products remain, discarding any "
            "extra repositories.\n"
            "\n" + _PROBE_NOTE
        ),
        examples=(
            "Reset a refhost to a clean repository set:",
            "repose reset -t fubar.suse.cz",
            "Skip URL probing during reset:",
            "repose reset --no-probe -t fubar.suse.cz",
        ),
        see_also=("clear", "add", "install"),
    ),
    "install": CommandDoc(
        description=(
            "Add the repositories selected by one or more REPA patterns "
            "to the target host(s) and install the corresponding "
            "product.\n"
            "\n" + _REPA_SYNTAX + "\n"
            "\n" + _PROBE_NOTE + "\n"
            "\n" + _TRANSACTIONAL_NOTE
        ),
        examples=(
            "Install a product (with reboot/verify on transactional hosts):",
            "repose install -t root@slmicro.example qa",
            "Stage the install without rebooting:",
            "repose --no-reboot install -t root@slmicro.example qa",
        ),
        see_also=("uninstall", "add", "reset"),
    ),
    "clear": CommandDoc(
        description=(
            "Remove all repositories from the target host(s). This is the "
            "blunt counterpart to reset, which keeps the repositories of "
            "installed products."
        ),
        examples=(
            "Clear every repository from a host:",
            "repose clear -t fubar.suse.cz",
        ),
        see_also=("reset", "remove"),
    ),
    "uninstall": CommandDoc(
        description=(
            "Remove the repositories selected by one or more REPA "
            "patterns from the target host(s) and uninstall the "
            "corresponding product.\n"
            "\n" + _REPA_SYNTAX + "\n"
            "\n" + _TRANSACTIONAL_NOTE
        ),
        examples=(
            "Uninstall a product and drop its repositories:",
            "repose uninstall -t fubar.suse.cz qa",
        ),
        see_also=("install", "remove", "clear"),
    ),
    "list-products": CommandDoc(
        description=(
            "List the products installed on the target host(s).\n"
            "\n"
            "With --yaml the output is a host specification suitable for "
            "the refhosts.yml generator, emitted without normalization. "
            "That YAML form is the default for SLE 12-SP5 and SLE 15-SP3 "
            "and newer products.\n"
            "\n"
            "With --format=json one product event is emitted per product "
            "per host; combined with --yaml a host_spec event is emitted "
            "per host instead."
        ),
        examples=(
            "List products on a host:",
            "repose list-products -t fubar.suse.cz",
            "Emit the refhosts.yml host spec:",
            "repose list-products --yaml -t fubar.suse.cz",
            "Machine-readable base products only:",
            "repose list-products --format=json -t fubar.suse.cz "
            "| jq 'select(.kind==\"base\")'",
        ),
        see_also=("list-repos", "known-products"),
    ),
    "list-repos": CommandDoc(
        description=(
            "List the repositories configured on the target host(s).\n"
            "\n"
            "With --format=json one repo event is emitted per repository "
            "per host, carrying its alias, name, URL, and enabled/disabled "
            "state."
        ),
        examples=(
            "List repositories on a host:",
            "repose list-repos -t fubar.suse.cz",
            "Machine-readable repository list:",
            "repose list-repos --format=json -t fubar.suse.cz | jq .",
        ),
        see_also=("list-products", "known-products"),
    ),
    "known-products": CommandDoc(
        description=(
            "List the product names repose knows about, as defined in the "
            "configuration file. These are the names usable as the PRODUCT "
            "segment of a REPA pattern.\n"
            "\n"
            "With --format=json one known_product event is emitted per "
            "product. Unlike the other list commands this does not contact "
            "any host."
        ),
        examples=(
            "List every known product name:",
            "repose known-products",
            "Machine-readable known product names:",
            "repose known-products --format=json | jq -r '.name'",
        ),
        see_also=("list-products", "list-repos"),
    ),
}

# ENVIRONMENT and FILES are global to every page (the flags they document
# are global options honored by all commands).
_ENVIRONMENT_ENTRIES: tuple[tuple[str, str], ...] = (
    (
        "NO_COLOR",
        "When set (to any value), disable ANSI color in console output. "
        "Equivalent to --no-color. See https://no-color.org.",
    ),
    (
        "COLOR",
        "Legacy override for color detection: COLOR=always forces color "
        "on, COLOR=never forces it off. Overrides terminal detection.",
    ),
    (
        "SOURCE_DATE_EPOCH",
        "Consumed by repose-mangen when generating these manual pages; "
        "pins the date stamped into each page for reproducible builds.",
    ),
)

_FILES_ENTRIES: tuple[tuple[str, str], ...] = (
    (
        "/etc/repose/products.yml",
        "Default configuration file mapping product names to "
        "repositories. Override with -c/--config.",
    ),
    (
        "~/.ssh/known_hosts",
        "Default SSH known_hosts file consulted for host-key "
        "verification. Override with --known-hosts.",
    ),
    (
        "~/.ssh/config",
        "Standard OpenSSH client configuration; honored by both SSH backends.",
    ),
)


# ---------------------------------------------------------------------------
# roff helpers
# ---------------------------------------------------------------------------


def _neutralize_leading(line: str) -> str:
    """Prefix ``\\&`` if a line starts with a roff control char.

    A line whose first character is ``.`` or ``'`` is interpreted by
    roff as a request/macro. The zero-width ``\\&`` prefix forces it to
    be treated as text. Must run on the *final* line breaks, since
    wrapping can move a word like ``'accept-new'`` to the start of a
    line.
    """
    if line[:1] in (".", "'"):
        return "\\&" + line
    return line


def _roff_escape(text: str) -> str:
    """Escape text for safe inclusion in a roff/man source line.

    Backslashes become ``\\e`` (roff's escape for a literal backslash).
    Leading control characters are neutralized per line. Use this for
    runs that are *not* subsequently wrapped; wrapped runs go through
    ``_wrap`` (which re-neutralizes after breaking lines).
    """
    text = text.replace("\\", "\\e")
    return "\n".join(_neutralize_leading(line) for line in text.split("\n"))


def _wrap(text: str) -> str:
    """Escape, soft-wrap (~72 cols), and neutralize a roff text run.

    roff reflows running text regardless of source line breaks, so the
    wrap is cosmetic — it keeps the committed ``.1`` files readable and
    silences mandoc's >80-byte STYLE notes. Crucially, leading-control
    neutralization runs *after* wrapping, because a wrap can push a word
    such as ``'accept-new'`` to the start of a fresh line where roff
    would otherwise treat the leading ``'`` as a request.

    Accepts raw (unescaped) text; do not pre-escape before calling.
    """
    escaped = text.replace("\\", "\\e")
    wrapped = textwrap.fill(
        escaped,
        width=72,
        break_long_words=False,
        break_on_hyphens=False,
    )
    wrapped = wrapped or escaped
    return "\n".join(_neutralize_leading(line) for line in wrapped.split("\n"))


def _th_date() -> str:
    """Return the ``.TH`` date string from ``SOURCE_DATE_EPOCH``."""
    epoch = int(os.environ.get("SOURCE_DATE_EPOCH", _FALLBACK_SOURCE_DATE_EPOCH))
    return time.strftime("%Y-%m-%d", time.gmtime(epoch))


@contextmanager
def _pinned_source_date() -> Iterator[None]:
    """Temporarily set ``SOURCE_DATE_EPOCH`` if unset, restore on exit.

    Caller-provided values take precedence — distro builds frequently
    export this from a changelog timestamp and we don't want to clobber
    that.
    """
    had_value = "SOURCE_DATE_EPOCH" in os.environ
    prior = os.environ.get("SOURCE_DATE_EPOCH")
    if not had_value:
        os.environ["SOURCE_DATE_EPOCH"] = _FALLBACK_SOURCE_DATE_EPOCH
    try:
        yield
    finally:
        if had_value:
            # ``had_value`` is True iff the key was present at entry,
            # which means ``prior`` was bound to ``os.environ[...]``
            # and is therefore ``str`` (never ``None``).
            assert prior is not None
            os.environ["SOURCE_DATE_EPOCH"] = prior
        else:
            os.environ.pop("SOURCE_DATE_EPOCH", None)


# ---------------------------------------------------------------------------
# Page rendering
# ---------------------------------------------------------------------------


def _iter_option_params(command: click.Command) -> Iterator[click.Parameter]:
    """Yield option-like (non-argument, non-hidden) params of a command.

    Duck-typed rather than ``isinstance(p, click.Option)``: Typer >=0.26
    vendors click, so its ``TyperOption`` is not an instance of the
    top-level ``click.Option``. We detect arguments via the
    ``param_type_name`` attribute (``"argument"`` vs ``"option"``) which
    both real and vendored click expose, and skip hidden params.
    """
    for param in command.params:
        if getattr(param, "param_type_name", None) != "option":
            continue
        if getattr(param, "hidden", False):
            continue
        yield param


def _short_help(command: click.Command) -> str:
    """One-line summary: explicit short_help, else first help line."""
    short = getattr(command, "short_help", None)
    if short:
        return short
    help_text = command.help or ""
    return help_text.strip().split("\n", 1)[0]


def _section(name: str, body: str) -> str:
    """Render one ``.SH`` section (body already roff-formatted)."""
    return f".SH {name}\n{body}"


def _render_description(command_name: str, command: click.Command) -> str:
    """DESCRIPTION body: rich prose from the table, else click help."""
    doc = _ROOT_DOC if command_name == "repose" else _COMMAND_DOCS.get(command_name)
    text = (doc.description if doc and doc.description else command.help) or ""
    text = text.strip()
    # Blank lines become ``.PP`` paragraph breaks; each paragraph is
    # escaped + soft-wrapped (``_wrap`` handles both) for readable source.
    lines = []
    for line in text.split("\n"):
        lines.append(".PP" if line == "" else _wrap(line))
    return "\n".join(lines)


def _render_options(ctx: click.Context, command: click.Command) -> str | None:
    """OPTIONS body from each option's ``get_help_record``."""
    parts = []
    for param in _iter_option_params(command):
        record = param.get_help_record(ctx)
        if not record:
            continue
        opt, desc = record
        parts.append(".TP")
        parts.append(f"\\fB{_roff_escape(opt)}\\fP")
        parts.append(_wrap(desc) or "\\&")
    if not parts:
        return None
    return "\n".join(parts)


def _render_commands(command: click.Command) -> str | None:
    """COMMANDS body for the root group (subcommand summaries)."""
    commands = getattr(command, "commands", None)
    if not commands:
        return None
    parts = []
    for name, sub in commands.items():
        if getattr(sub, "hidden", False):
            continue
        parts.append(".TP")
        parts.append(f"\\fB{name}\\fP")
        parts.append(_wrap(_short_help(sub)))
        parts.append(".br")
        parts.append(f"See \\fBrepose-{name}(1)\\fP for full documentation.")
    if not parts:
        return None
    return "\n".join(parts)


def _render_examples(command_name: str) -> str | None:
    """EXAMPLES body. Command lines are indented; other lines describe."""
    doc = _ROOT_DOC if command_name == "repose" else _COMMAND_DOCS.get(command_name)
    if not doc or not doc.examples:
        return None
    parts = []
    for line in doc.examples:
        if line.startswith("repose"):
            # Command: bold, indented on its own line.
            parts.append(".RS 4")
            parts.append(f"\\fB{_roff_escape(line)}\\fP")
            parts.append(".RE")
        else:
            # Descriptive lead-in for the following command(s). Skip a
            # leading ``.PP`` (redundant right after ``.SH``; mandoc
            # warns on it).
            if parts:
                parts.append(".PP")
            parts.append(_wrap(line))
    return "\n".join(parts)


def _render_pairs(entries: tuple[tuple[str, str], ...]) -> str:
    """Render ``.TP`` term/definition pairs (ENVIRONMENT, FILES)."""
    parts = []
    for term, desc in entries:
        parts.append(".TP")
        parts.append(f"\\fB{_roff_escape(term)}\\fP")
        parts.append(_wrap(desc))
    return "\n".join(parts)


def _render_see_also(command_name: str) -> str:
    """SEE ALSO body cross-referencing sibling repose pages."""
    refs: list[str] = []
    if command_name == "repose":
        refs = [f"repose-{name}(1)" for name in _COMMAND_DOCS]
    else:
        doc = _COMMAND_DOCS.get(command_name)
        if doc:
            refs.extend(f"repose-{name}(1)" for name in doc.see_also)
        refs.append("repose(1)")
    refs.extend(["zypper(8)", "transactional-update(8)", "ssh(1)"])
    return ",\n".join(f"\\fB{ref}\\fP" for ref in refs)


def _render_page(command_name: str, page_name: str, ctx: click.Context) -> str:
    """Assemble the full roff source for one command's man page.

    Args:
        command_name: kebab-case command key (e.g. ``add``) or
            ``repose`` for the root.
        page_name: full command path used in headings (e.g.
            ``repose add``).
        ctx: a click ``Context`` bound to the command.
    """
    command = ctx.command
    upper = page_name.upper()
    name_dashed = page_name.replace(" ", r"\-")

    lines: list[str] = []
    lines.append(
        f'.TH "{upper}" "1" "{_th_date()}" "{__version__}" "{page_name} Manual"'
    )
    lines.append(
        _section("NAME", rf"{name_dashed} \- {_roff_escape(_short_help(command))}")
    )
    synopsis = " ".join(command.collect_usage_pieces(ctx))
    lines.append(_section("SYNOPSIS", f".B {page_name}\n{_roff_escape(synopsis)}"))
    lines.append(_section("DESCRIPTION", _render_description(command_name, command)))

    options = _render_options(ctx, command)
    if options:
        lines.append(_section("OPTIONS", options))

    commands = _render_commands(command)
    if commands:
        lines.append(_section("COMMANDS", commands))

    examples = _render_examples(command_name)
    if examples:
        lines.append(_section("EXAMPLES", examples))

    lines.append(_section("ENVIRONMENT", _render_pairs(_ENVIRONMENT_ENTRIES)))
    lines.append(_section("FILES", _render_pairs(_FILES_ENTRIES)))
    lines.append(_section("SEE ALSO", _render_see_also(command_name)))

    page = "\n".join(lines)
    if not page.endswith("\n"):
        page += "\n"
    return page


def main() -> None:
    """Regenerate ``docs/man/repose*.1`` from the Typer app."""
    out = Path(__file__).resolve().parent.parent / "docs" / "man"
    out.mkdir(parents=True, exist_ok=True)
    cli = typer.main.get_command(app)

    with _pinned_source_date():
        # Root page.
        root_ctx = click.Context(cli, info_name="repose")
        (out / "repose.1").write_text(_render_page("repose", "repose", root_ctx))

        # One page per subcommand.
        commands = getattr(cli, "commands", {})
        for name, command in commands.items():
            if getattr(command, "hidden", False):
                continue
            ctx = click.Context(command, info_name=name, parent=root_ctx)
            page_name = f"repose {name}"
            path = out / f"{page_name.replace(' ', '-')}.1"
            path.write_text(_render_page(name, page_name, ctx))


if __name__ == "__main__":
    main()
