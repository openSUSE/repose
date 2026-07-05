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

import logging
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


# Date rendered from _FALLBACK_SOURCE_DATE_EPOCH (2024-01-01T00:00:00Z).
# Hardcoded (not derived from the constant) so an accidental edit to the
# constant or the strftime format fails here instead of tracking along.
_FALLBACK_DATE = "2024-01-01"

# int()-parseable but unusable epochs: a nanosecond-epoch leak (e.g.
# ``date +%s%N``) raises OSError in time.gmtime on Linux, and a value
# beyond time_t raises OverflowError.
_OUT_OF_RANGE_EPOCHS = ["1704067200000000000", "99999999999999999999"]


@pytest.mark.parametrize("bad", ["", "abc", "12.5", *_OUT_OF_RANGE_EPOCHS])
def test_malformed_source_date_epoch_warns_and_falls_back(
    monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture, bad: str
):
    """Regression: a bad SOURCE_DATE_EPOCH used to abort generation.

    ``_pinned_source_date`` only injected the fallback when the variable
    was entirely absent, so a present-but-empty or non-integer value
    reached ``int()`` unguarded (ValueError), and an int-parseable but
    out-of-range value crashed ``time.gmtime()`` (OverflowError/OSError).
    The reproducible-builds convention is to warn and ignore invalid
    values, behaving exactly as if the variable were unset.
    """
    monkeypatch.setenv("SOURCE_DATE_EPOCH", bad)
    with caplog.at_level(logging.WARNING, logger="repose.mangen"):
        assert mangen._th_date() == _FALLBACK_DATE
    assert any("SOURCE_DATE_EPOCH" in rec.message for rec in caplog.records)


def test_absent_source_date_epoch_falls_back(monkeypatch: pytest.MonkeyPatch):
    monkeypatch.delenv("SOURCE_DATE_EPOCH", raising=False)
    assert mangen._th_date() == _FALLBACK_DATE


@pytest.mark.parametrize("bad", ["", "abc", *_OUT_OF_RANGE_EPOCHS])
def test_invalid_epoch_behaves_exactly_like_absent(
    monkeypatch: pytest.MonkeyPatch, bad: str
):
    monkeypatch.delenv("SOURCE_DATE_EPOCH", raising=False)
    absent = mangen._th_date()
    monkeypatch.setenv("SOURCE_DATE_EPOCH", bad)
    assert mangen._th_date() == absent


def test_valid_source_date_epoch_wins(
    monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
):
    monkeypatch.setenv("SOURCE_DATE_EPOCH", "1750000000")  # 2025-06-15 UTC
    with caplog.at_level(logging.WARNING, logger="repose.mangen"):
        assert mangen._th_date() == "2025-06-15"
    assert not caplog.records


def test_malformed_epoch_warns_once_per_run(
    monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
):
    """The malformed-value warning fires once per run, not once per page.

    ``main()`` renders ~10 pages inside one ``_pinned_source_date``
    block; resolution (and the warning) must happen there exactly once,
    with every per-page ``_th_date()`` reusing the validated value.
    """
    monkeypatch.setenv("SOURCE_DATE_EPOCH", "garbage")
    with caplog.at_level(logging.WARNING, logger="repose.mangen"):
        with mangen._pinned_source_date():
            for _ in range(3):
                assert mangen._th_date() == _FALLBACK_DATE
    warnings = [r for r in caplog.records if "SOURCE_DATE_EPOCH" in r.message]
    assert len(warnings) == 1


def test_generation_proceeds_with_malformed_epoch(monkeypatch: pytest.MonkeyPatch):
    """A full page renders (rather than crashing) under a bad value."""
    monkeypatch.setenv("SOURCE_DATE_EPOCH", "garbage")
    cli = typer.main.get_command(app)
    with mangen._pinned_source_date():
        root_ctx = click.Context(cli, info_name="repose")
        page = mangen._render_page("repose", "repose", root_ctx)
    assert page.startswith(f'.TH "REPOSE" "1" "{_FALLBACK_DATE}"')


def test_main_prunes_orphaned_subcommand_pages(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
):
    """Regression: pages of removed/renamed subcommands are deleted.

    ``main`` used to only write pages for currently-registered commands,
    so a page like ``repose-oldcmd.1`` left behind by a rename lingered
    untouched — invisible to the ``git diff --exit-code -- docs/man/``
    CI drift gate. Pruning turns the orphan into a deletion the gate
    catches, and each deletion is announced on stderr (repose-mangen
    never configures logging, so a logger call would be silent).
    """
    stale = tmp_path / "repose-oldcmd.1"
    stale.write_text('.TH "REPOSE OLDCMD" "1"\n')

    mangen.main(tmp_path)

    assert not stale.exists(), "orphaned repose-oldcmd.1 was not pruned"
    err = capsys.readouterr().err
    assert "repose-oldcmd.1" in err, "deletion must be announced on stderr"


def test_main_prune_spares_root_page_and_unrelated_files(tmp_path: Path):
    """Pruning only targets ``repose-<sub>.1``; everything else survives.

    ``repose.1`` (the root page, regenerated every run) and files not
    matching the naming scheme must never be deleted.
    """
    unrelated = tmp_path / "notes.1"
    unrelated.write_text("not a repose page\n")

    mangen.main(tmp_path)

    assert unrelated.exists(), "non-matching file was wrongly deleted"
    assert (tmp_path / "repose.1").exists(), "root page missing after prune"
    for name in SUBCOMMANDS:
        page = tmp_path / f"repose-{name}.1"
        assert page.exists(), f"generated page {page.name} missing after prune"


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
