"""Tests for the man-page generator (``repose.mangen``).

These lock in the fix for two regressions that left the committed man
pages nearly empty:

1. ``click-man`` dropped every option because Typer >=0.26 vendors its
   own click, so ``TyperOption`` is not an instance of the top-level
   ``click.Option`` that ``click-man`` filtered on. The generator now
   collects options by duck-typing and renders an OPTIONS section.
2. The DESCRIPTION was a one-line echo of NAME. The generator now
   injects rich prose plus EXAMPLES / ENVIRONMENT / FILES / SEE ALSO.

A Typer or click(-man) upgrade that re-breaks option collection, or an
edit that strips the enriched sections, should fail here rather than
silently shipping unhelpful pages.
"""

from pathlib import Path

import click
import pytest
import typer

from repose import mangen
from repose.cli import app

# Every command that must get a man page (root + subcommands).
SUBCOMMANDS = [
    "add",
    "remove",
    "reset",
    "install",
    "clear",
    "uninstall",
    "list-products",
    "list-repos",
    "known-products",
]


@pytest.fixture(scope="module")
def man_dir(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Generate all man pages once into a temp dir and return it.

    Drives the renderer directly (rather than ``mangen.main``) so the
    test is independent of the on-disk repo layout, while still pinning
    ``SOURCE_DATE_EPOCH`` exactly as ``main`` does.
    """
    out = tmp_path_factory.mktemp("man")
    cli = typer.main.get_command(app)
    with mangen._pinned_source_date():
        root_ctx = click.Context(cli, info_name="repose")
        (out / "repose.1").write_text(mangen._render_page("repose", "repose", root_ctx))
        for name, command in cli.commands.items():
            ctx = click.Context(command, info_name=name, parent=root_ctx)
            page_name = f"repose {name}"
            (out / f"{page_name.replace(' ', '-')}.1").write_text(
                mangen._render_page(name, page_name, ctx)
            )
    return out


def _read(man_dir: Path, slug: str) -> str:
    return (man_dir / f"{slug}.1").read_text()


def test_all_pages_generated(man_dir: Path):
    expected = {"repose"} | {f"repose-{n}" for n in SUBCOMMANDS}
    actual = {p.stem for p in man_dir.glob("*.1")}
    assert actual == expected


# known-products is the one subcommand with no command-specific options
# (it contacts no host, so no ``-t/--target``). Every other subcommand
# must render an OPTIONS section listing ``--target``.
SUBCOMMANDS_WITH_OPTIONS = [n for n in SUBCOMMANDS if n != "known-products"]


@pytest.mark.parametrize("name", SUBCOMMANDS_WITH_OPTIONS)
def test_options_section_present(man_dir: Path, name: str):
    """Regression: subcommands with options must render them.

    This is the core bug — the vendored-click ``isinstance`` mismatch
    used to drop all options, leaving no OPTIONS section at all.
    """
    page = _read(man_dir, f"repose-{name}")
    assert ".SH OPTIONS" in page, f"{name}: OPTIONS section missing"
    assert "--target" in page, f"{name}: --target option not rendered"


def test_add_lists_all_its_options(man_dir: Path):
    page = _read(man_dir, "repose-add")
    for flag in ("--target", "--probe-timeout", "--no-probe"):
        assert flag in page, f"add: {flag} missing from OPTIONS"


def test_root_lists_global_options(man_dir: Path):
    page = _read(man_dir, "repose")
    for flag in ("--print", "--config", "--strict-host-key-checking", "--ssh-backend"):
        assert flag in page, f"root: global option {flag} missing"


@pytest.mark.parametrize("name", SUBCOMMANDS)
def test_rich_sections_present(man_dir: Path, name: str):
    page = _read(man_dir, f"repose-{name}")
    for section in (
        ".SH DESCRIPTION",
        ".SH EXAMPLES",
        ".SH ENVIRONMENT",
        ".SH FILES",
        ".SH SEE ALSO",
    ):
        assert section in page, f"{name}: {section} missing"


def _name_summary(command_name: str) -> str:
    """The one-line help shown on the NAME line, for comparison."""
    cli = typer.main.get_command(app)
    command = cli.commands[command_name]
    return mangen._short_help(command)


@pytest.mark.parametrize("name", SUBCOMMANDS)
def test_description_is_richer_than_the_one_liner(man_dir: Path, name: str):
    """DESCRIPTION must say more than the NAME one-liner.

    Pre-fix, DESCRIPTION was a verbatim echo of the one-line help. The
    enriched body is always longer (and usually multi-paragraph, though
    a few commands like ``clear`` are a single richer paragraph).
    """
    page = _read(man_dir, f"repose-{name}")
    desc = page.split(".SH DESCRIPTION", 1)[1].split(".SH ", 1)[0].strip()
    summary = _name_summary(name)
    assert desc != summary, f"{name}: DESCRIPTION just echoes the one-liner"
    assert len(desc) > len(summary), f"{name}: DESCRIPTION no richer than NAME"


def test_environment_documents_color_and_epoch(man_dir: Path):
    page = _read(man_dir, "repose-add")
    for var in ("NO_COLOR", "COLOR", "SOURCE_DATE_EPOCH"):
        assert var in page


def test_files_documents_config_path(man_dir: Path):
    page = _read(man_dir, "repose-add")
    assert "/etc/repose/products.yml" in page


def test_see_also_cross_references(man_dir: Path):
    page = _read(man_dir, "repose-install")
    assert "repose-uninstall(1)" in page
    assert "repose(1)" in page


def test_th_header_uses_pinned_date_and_version(man_dir: Path):
    page = _read(man_dir, "repose-add")
    first = page.splitlines()[0]
    assert first.startswith('.TH "REPOSE ADD" "1"')
    # Version is stamped from repose.__version__.
    from repose import __version__

    assert f'"{__version__}"' in first


def test_leading_control_chars_are_neutralized(man_dir: Path):
    """No rendered text line may start with a bare ``.`` or ``'``.

    The host-key-policy help contains ``'accept-new'``; wrapping once
    pushed it to the start of a line where roff treated the leading
    quote as a request. Guard against that class of bug everywhere.
    """
    for page_path in man_dir.glob("*.1"):
        for lineno, line in enumerate(page_path.read_text().splitlines(), 1):
            if line.startswith("'"):
                pytest.fail(
                    f"{page_path.name}:{lineno} starts with a bare quote: {line!r}"
                )
