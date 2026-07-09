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


# ---------------------------------------------------------------------------
# _render_pairs: exact ``.TP`` term/definition blocks (ENVIRONMENT, FILES).
# ---------------------------------------------------------------------------


def test_render_pairs_renders_exact_tp_blocks():
    """Each entry becomes ``.TP`` / bold term / wrapped desc, ``\\n``-joined.

    Pins the literal ``.TP`` control line, the ``\\fB...\\fP`` bold term,
    and the newline join so string/case mutations of any of them fail.
    """
    out = mangen._render_pairs((("FOO", "bar baz"), ("QUX", "quux")))
    assert out == ".TP\n\\fBFOO\\fP\nbar baz\n.TP\n\\fBQUX\\fP\nquux"


# ---------------------------------------------------------------------------
# _render_page: exact per-section roff so string/None/case mutants fail.
# ---------------------------------------------------------------------------


def test_render_page_description_section_has_rich_prose(man_dir: Path):
    """DESCRIPTION uses the rich table prose, not the terse click help."""
    page = _read(man_dir, "repose-add")
    assert (
        ".SH DESCRIPTION\nAdd the repositories selected by one or more REPA patterns"
        in page
    )
    assert "REPA (REpository PAttern)" in page


def test_render_page_options_section_header_and_tp(man_dir: Path):
    """OPTIONS header is exact and its body opens with a ``.TP`` block."""
    page = _read(man_dir, "repose-add")
    assert ".SH OPTIONS\n.TP\n" in page


def test_render_page_root_commands_section_exact(man_dir: Path):
    """Root COMMANDS body opens with a ``.TP`` block for ``add``."""
    page = _read(man_dir, "repose")
    assert ".SH COMMANDS\n.TP\n\\fBadd\\fP\n" in page


def test_render_page_environment_and_files_sections_exact(man_dir: Path):
    """ENVIRONMENT and FILES headers are exact and open with ``.TP`` blocks."""
    page = _read(man_dir, "repose-add")
    assert ".SH ENVIRONMENT\n.TP\n\\fBNO_COLOR\\fP\n" in page
    assert ".SH FILES\n.TP\n\\fB/etc/repose/products.yml\\fP\n" in page


def test_render_page_see_also_section_exact(man_dir: Path):
    """SEE ALSO header is exact and leads with the sibling cross-reference."""
    page = _read(man_dir, "repose-install")
    assert ".SH SEE ALSO\n\\fBrepose-uninstall(1)\\fP," in page


def test_render_page_joins_sections_with_newlines_and_ends_with_newline(man_dir: Path):
    """Sections are ``\\n``-joined and the page has exactly one trailing NL."""
    page = _read(man_dir, "repose-add")
    assert "\n.SH NAME\n" in page
    assert page.endswith("\n")
    assert not page.endswith("\n\n")


# ---------------------------------------------------------------------------
# main: the pages it writes carry the right command_name/page_name, and it
# creates missing parent directories.
# ---------------------------------------------------------------------------


def test_main_root_page_has_correct_name_and_rich_sections(tmp_path: Path):
    """The written ``repose.1`` uses command_name ``repose`` / page ``repose``."""
    mangen.main(tmp_path)
    page = (tmp_path / "repose.1").read_text()
    assert page.startswith('.TH "REPOSE" "1"')
    assert "\n.B repose\n" in page
    assert ".SH COMMANDS" in page
    assert ".SH EXAMPLES" in page
    assert "Repose queries and manipulates" in page


def test_main_subcommand_page_has_examples_and_rich_description(tmp_path: Path):
    """The written ``repose-add.1`` is rendered with command_name ``add``."""
    mangen.main(tmp_path)
    page = (tmp_path / "repose-add.1").read_text()
    assert ".SH EXAMPLES" in page
    assert "REPA (REpository PAttern)" in page


def test_main_creates_nested_output_directory(tmp_path: Path):
    """A non-existent nested out_dir is created (mkdir parents=True)."""
    nested = tmp_path / "a" / "b" / "man"
    mangen.main(nested)
    assert (nested / "repose.1").exists()
    for name in SUBCOMMANDS:
        assert (nested / f"repose-{name}.1").exists(), f"missing repose-{name}.1"


# ---------------------------------------------------------------------------
# _render_page: robust (help-text-independent) anchors for the space->\\-
# escape and the single-space usage join, so a benign CLI help/arg reword
# cannot make these mutants slip through on incidental-text drift.
# ---------------------------------------------------------------------------


def _render_page_synopsis_body(command_name: str) -> str:
    """Reconstruct the exact roff-escaped SYNOPSIS body via the real API.

    Derived from ``collect_usage_pieces`` (not hardcoded) so it tracks any
    benign arg/metavar reword, while still pinning the single-space join.
    """
    cli = typer.main.get_command(app)
    root_ctx = click.Context(cli, info_name="repose")
    command = cli.commands[command_name]
    ctx = click.Context(command, info_name=command_name, parent=root_ctx)
    return mangen._roff_escape(" ".join(command.collect_usage_pieces(ctx)))


def test_render_page_name_dash_escape_is_help_text_independent(man_dir: Path):
    """NAME escapes the space in a multiword page name to ``repose\\-add``.

    Anchors only the ``.SH NAME`` header and the ``page_name.replace`` output
    up to the `` \\- `` separator, not the incidental short-help that follows,
    so a help reword cannot mask a broken space->\\- escape.
    """
    page = _read(man_dir, "repose-add")
    assert ".SH NAME\nrepose\\-add \\- " in page


def test_render_page_synopsis_single_space_join_is_arg_text_independent(
    man_dir: Path,
):
    """SYNOPSIS joins usage pieces with exactly one space (derived, not pinned).

    The expected body comes from the live ``collect_usage_pieces``, so the
    test survives an arg/metavar reword yet still fails if the join spacing
    changes.
    """
    page = _read(man_dir, "repose-add")
    body = _render_page_synopsis_body("add")
    assert f".SH SYNOPSIS\n.B repose add\n{body}\n" in page


# ---------------------------------------------------------------------------
# main: the subcommand loop's hidden-skip branch. The app registers zero
# hidden subcommands, so this branch is otherwise never exercised; inject a
# synthetic hidden command (via the get_command call main makes) to guard it.
# ---------------------------------------------------------------------------


def test_main_skips_hidden_subcommand_and_keeps_going(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    """A hidden subcommand gets no page, and later commands still render.

    Injecting the hidden command *first* in iteration order makes the
    skip observable two ways: no ``repose-<hidden>.1`` is written (guards the
    ``getattr(command, "hidden", ...)`` predicate), and every real
    subcommand page after it still appears (guards ``continue`` vs ``break``).
    """
    real_get_command = typer.main.get_command

    def _inject_hidden(a: object) -> click.Command:
        grp = real_get_command(a)
        grp.commands = {
            "zz-hidden": click.Command("zz-hidden", hidden=True),
            **grp.commands,
        }
        return grp

    monkeypatch.setattr(typer.main, "get_command", _inject_hidden)

    mangen.main(tmp_path)

    assert not (tmp_path / "repose-zz-hidden.1").exists(), (
        "hidden subcommand must not get a man page"
    )
    for name in SUBCOMMANDS:
        page = tmp_path / f"repose-{name}.1"
        assert page.exists(), (
            f"{page.name} missing: loop stopped early instead of skipping hidden"
        )


# ---------------------------------------------------------------------------
# _render_description: rich table prose vs click fallback, paragraph breaks,
# and the empty/unknown-command fallbacks (mutation coverage).
# ---------------------------------------------------------------------------


def test_render_description_root_uses_rich_root_doc():
    """Root DESCRIPTION comes from _ROOT_DOC, not the terse click help.

    Guards the ``command_name == "repose"`` branch (and ``doc = None``):
    the root help is only ``"Repository manipulation tool for QAM"``, so
    the rich prose phrase can only appear via _ROOT_DOC.
    """
    cli = typer.main.get_command(app)
    out = mangen._render_description("repose", cli)
    assert "Repose queries and manipulates" in out


def test_render_description_subcommand_uses_table_prose_and_paragraphs():
    """Subcommand DESCRIPTION uses table prose and ``.PP`` paragraph breaks.

    Pins the rich prose (guards ``_COMMAND_DOCS.get(None)`` and the
    ``doc.description``/``or ""`` selection) and the exact ``\\n.PP\\n``
    separator (guards the ``"\\n"`` split, the ``.PP`` literal/case, the
    ``line == ""`` test, and the ``"\\n"`` join).
    """
    cli = typer.main.get_command(app)
    out = mangen._render_description("add", cli.commands["add"])
    assert "A REPA (REpository PAttern)" in out
    assert "\n.PP\n" in out
    assert not out.startswith(".PP")


def test_render_description_unknown_command_falls_back_to_help():
    """A command not in the table renders its (escaped, wrapped) click help.

    Guards the ``doc and doc.description`` selection: with ``doc`` None the
    ``doc or doc.description`` mutant dereferences ``None.description``.
    """
    faux = click.Command("faux", help="Some prose here.")
    assert mangen._render_description("faux", faux) == "Some prose here."


def test_render_description_empty_help_yields_single_paragraph_break():
    """Empty help + no table entry renders exactly ``.PP``.

    Guards the ``or ""`` fallback (a ``or "XXXX"`` mutant would render
    ``XXXX``) and again the ``doc or doc.description`` None dereference.
    """
    faux = click.Command("faux", help="")
    assert mangen._render_description("faux", faux) == ".PP"


# ---------------------------------------------------------------------------
# _render_options: exact ``.TP`` blocks, empty-desc ``\\&`` placeholder, and
# the None-when-no-options path (mutation coverage).
# ---------------------------------------------------------------------------


def test_render_options_exact_tp_blocks_and_empty_desc_placeholder():
    """Each option becomes ``.TP`` / bold flag / wrapped desc (or ``\\&``).

    Pins the literal ``.TP``, the ``\\fB...\\fP`` bold flag, the ``\\n``
    join, and the ``or "\\&"`` placeholder used when an option has no help
    text. Kills the record-None, ``if not record`` inversion, ``.TP``
    string/case, ``_wrap(desc) and``, ``"\\&"`` string, and join mutants.
    """
    cmd = click.Command(
        "x",
        params=[
            click.Option(["--foo"], is_flag=True, help="Do the foo thing."),
            click.Option(["--bar"], is_flag=True),
        ],
    )
    ctx = click.Context(cmd)
    out = mangen._render_options(ctx, cmd)
    assert out == (".TP\n\\fB--foo\\fP\nDo the foo thing.\n.TP\n\\fB--bar\\fP\n\\&")


def test_render_options_returns_none_without_options():
    """A command with no option-like params renders no OPTIONS body.

    Guards the ``if not parts: return None`` guard against an ``if parts``
    inversion (which would return ``""`` instead of ``None``).
    """
    cmd = click.Command("x")
    ctx = click.Context(cmd)
    assert mangen._render_options(ctx, cmd) is None


# ---------------------------------------------------------------------------
# _render_commands: exact per-subcommand block, hidden-skip, and the
# None-when-no-subcommands path (mutation coverage).
# ---------------------------------------------------------------------------


def test_render_commands_exact_block_skips_hidden_and_continues():
    """Visible subcommands render an exact block; hidden ones are skipped.

    The hidden entry is listed first so the ``continue`` (vs ``break``)
    and the ``getattr(sub, "hidden", ...)`` predicate are both exercised:
    ``alpha`` must still render and ``zz-hidden`` must not appear. Pins the
    literal ``.TP``/``.br`` controls, the ``\\fB...\\fP`` bold name, the
    cross-reference line, and the ``\\n`` join.
    """
    grp = click.Group(
        "g",
        commands={
            "zz-hidden": click.Command(
                "zz-hidden", hidden=True, short_help="hidden cmd"
            ),
            "alpha": click.Command("alpha", short_help="alpha help"),
        },
    )
    out = mangen._render_commands(grp)
    assert out == (
        ".TP\n\\fBalpha\\fP\nalpha help\n.br\n"
        "See \\fBrepose-alpha(1)\\fP for full documentation."
    )
    assert "zz-hidden" not in out


def test_render_commands_returns_none_for_plain_command():
    """A leaf command (no ``.commands``) renders no COMMANDS body."""
    assert mangen._render_commands(click.Command("x")) is None


def test_render_commands_all_hidden_group_returns_none():
    """A group whose every subcommand is hidden renders no COMMANDS body.

    ``commands`` is non-empty (so the early ``if not commands`` guard
    passes), but the loop skips every entry, leaving ``parts`` empty.
    Guards the trailing ``if not parts: return None``: an ``if parts:``
    mutant would fall through and return an empty string instead of None.
    """
    grp = click.Group(
        "g",
        commands={"zz-hidden": click.Command("zz-hidden", hidden=True)},
    )
    assert mangen._render_commands(grp) is None


# ---------------------------------------------------------------------------
# _render_examples: exact command/description lines, root vs subcommand
# selection, paragraph breaks, and the unknown-command None path.
# ---------------------------------------------------------------------------


def test_render_examples_root_present_and_subcommand_paragraph_break():
    """Root examples render, and multi-example bodies use ``.PP`` breaks.

    ``_render_examples("repose")`` must not be None (guards the ``repose``
    literal/case in the branch selector), and ``add`` (multiple examples)
    must separate them with the exact ``\\n.PP\\n`` (guards the ``.PP``
    literal/case).
    """
    assert mangen._render_examples("repose") is not None
    assert "\n.PP\n" in mangen._render_examples("add")


def test_render_examples_unknown_command_returns_none():
    """A command not in the table (and no examples) yields None.

    Guards the ``not doc or not doc.examples`` short-circuit: an ``and``
    mutant would dereference ``None.examples`` on an unknown command.
    """
    assert mangen._render_examples("faux") is None


# ---------------------------------------------------------------------------
# _render_see_also: exact cross-reference list for subcommand and root.
# ---------------------------------------------------------------------------


def test_render_see_also_root_lists_sibling_pages_not_self():
    """Root SEE ALSO lists every ``repose-<sub>(1)`` and no bare ``repose(1)``.

    Guards the ``command_name == "repose"`` branch: the mutants fall into
    the else branch, which drops the per-subcommand refs and instead
    appends a bare ``repose(1)``.
    """
    out = mangen._render_see_also("repose")
    assert "\\fBrepose-add(1)\\fP" in out
    assert "\\fBrepose(1)\\fP" not in out


# ---------------------------------------------------------------------------
# Data-independent companions for the two exact-equality tests the reviewer
# flagged as brittle (they pin real _COMMAND_DOCS content: install's see_also
# order and remove's literal example). These inject a SYNTHETIC CommandDoc so
# the structural roff invariants are pinned without coupling to the real docs
# table -- a benign reword/reorder of the shipped prose leaves them green while
# they still kill every rendering mutant (branch selector, .PP/.RS/.RE/.TP
# literals, repose(1) append, the static zypper/transactional-update/ssh trio,
# and the join separators).
# ---------------------------------------------------------------------------


def test_render_examples_structure_is_docs_content_independent(
    monkeypatch: pytest.MonkeyPatch,
):
    """EXAMPLES roff structure, pinned via a synthetic (not shipped) doc.

    A descriptive line renders verbatim; a ``repose ...`` line is wrapped in
    ``.RS 4``/``.RE``; a second descriptive line is prefixed with ``.PP``.
    Uses a made-up command absent from the real table, so a reword of the
    shipped ``remove`` example cannot break it, yet it still fails on any
    ``.RS 4``/``.RE``/``.PP`` literal, ``startswith("repose")``, or join
    mutation.
    """
    doc = mangen.CommandDoc(
        examples=("Lead in:", "repose foo -t h", "Then:", "repose bar baz")
    )
    monkeypatch.setitem(mangen._COMMAND_DOCS, "synthetic-ex", doc)
    out = mangen._render_examples("synthetic-ex")
    assert out == (
        "Lead in:\n"
        ".RS 4\n\\fBrepose foo -t h\\fP\n.RE\n"
        ".PP\nThen:\n"
        ".RS 4\n\\fBrepose bar baz\\fP\n.RE"
    )


def test_render_see_also_subcommand_structure_is_docs_content_independent(
    monkeypatch: pytest.MonkeyPatch,
):
    """Subcommand SEE ALSO structure, pinned via a synthetic (not shipped) doc.

    Table refs become ``repose-<name>(1)`` in order, followed by ``repose(1)``
    and the static ``zypper(8)``/``transactional-update(8)``/``ssh(1)`` trio,
    comma+newline joined. Uses a made-up command with made-up ``see_also``
    entries, so a reorder of the real ``install`` cross-references cannot break
    it, yet it still fails on the branch selector, the ``repose(1)`` append,
    any static-entry edit, or a join mutation.
    """
    doc = mangen.CommandDoc(see_also=("alpha", "beta"))
    monkeypatch.setitem(mangen._COMMAND_DOCS, "synthetic-sa", doc)
    out = mangen._render_see_also("synthetic-sa")
    assert out == (
        "\\fBrepose-alpha(1)\\fP,\n\\fBrepose-beta(1)\\fP,\n"
        "\\fBrepose(1)\\fP,\n\\fBzypper(8)\\fP,\n"
        "\\fBtransactional-update(8)\\fP,\n\\fBssh(1)\\fP"
    )


# ---------------------------------------------------------------------------
# _neutralize_leading: a line beginning with a bare roff control char (``.``
# or ``'``) is prefixed with the zero-width ``\\&``; anything else is passed
# through unchanged. Exact equality pins the prefix string and the two-char
# membership set against slice/inversion/string mutants.
# ---------------------------------------------------------------------------


def test_neutralize_leading_prefixes_dot_and_quote_lines():
    """Lines starting with ``.`` or ``'`` gain a literal ``\\&`` prefix.

    Kills the ``line[:1]`` -> ``line[:2]`` slice mutant (a two-char slice
    never equals a one-char member), the ``in`` -> ``not in`` inversion, the
    ``"."``/``"'"`` member-string mutants, and the ``"\\&"`` prefix mutant.
    """
    assert mangen._neutralize_leading(".foo") == "\\&.foo"
    assert mangen._neutralize_leading("'foo") == "\\&'foo"


def test_neutralize_leading_passes_ordinary_lines_through():
    """A line not starting with a control char is returned verbatim.

    Kills the ``in`` -> ``not in`` inversion (which would wrongly prefix
    ordinary text) and confirms no spurious ``\\&`` is added.
    """
    assert mangen._neutralize_leading("hello") == "hello"
    assert mangen._neutralize_leading("") == ""


# ---------------------------------------------------------------------------
# _roff_escape: backslashes become ``\\e`` and each ``\n``-split line is
# neutralized then re-joined with ``\n``. Exact equality pins the replace
# args, the split delimiter, and the join separator.
# ---------------------------------------------------------------------------


def test_roff_escape_backslash_becomes_backslash_e():
    """A literal backslash is rewritten to roff's ``\\e``.

    Kills the ``replace("\\", ...)`` -> ``replace("XX\\XX", ...)`` (wrong
    needle, no substitution) and the ``"\\e"`` -> ``"XX\\eXX"`` replacement
    mutants.
    """
    assert mangen._roff_escape("a\\b") == "a\\eb"


def test_roff_escape_neutralizes_per_line_and_joins_with_newline():
    """Each line is neutralized independently, then ``\n``-joined.

    A non-leading line beginning with ``.`` must be neutralized while the
    space-containing first line is preserved intact. Kills the ``\n`` join
    mutant, the ``split("\n")`` -> ``split(None)`` (would drop the space and
    re-tokenize) and ``split("XX\nXX")`` (would neutralize the whole blob as
    one line, missing the interior ``.c``) mutants.
    """
    assert mangen._roff_escape("a b\n.c") == "a b\n\\&.c"


# ---------------------------------------------------------------------------
# _wrap: escape backslashes, soft-wrap at exactly 72 cols with
# break_long_words/break_on_hyphens both False, then neutralize each wrapped
# line and ``\n``-join. Inputs are built from fixed-width filler words so the
# wrap points are deterministic and the full output can be pinned exactly.
# ---------------------------------------------------------------------------

# 14 four-char words + single spaces = 69 cols, the widest that fits in 72.
_WRAP_FILLER = " ".join(["word"] * 14)


def test_wrap_escapes_backslash():
    """``_wrap`` escapes backslashes to ``\\e`` before wrapping.

    Short input (no wrap) isolates the escape step. Kills the
    ``replace("XX\\XX", ...)`` and ``replace("\\", "XX\\eXX")`` mutants.
    """
    assert mangen._wrap("a\\b") == "a\\eb"


def test_wrap_wraps_and_neutralizes_pushed_control_line():
    """A word wrapped to the start of a line is neutralized after wrapping.

    ``.foo`` overflows line one and lands at the head of line two, where it
    must gain the ``\\&`` prefix. This single exact assertion kills: the
    ``wrapped = None`` and ``wrapped or escaped`` -> ``and`` mutants (both
    collapse to unwrapped single-line output with no ``\\&``), the ``\n``
    join mutant, the ``split(None)`` mutant (would re-tokenize on spaces),
    and the ``split("XX\nXX")`` mutant (would treat the whole blob as one
    line and miss the ``.foo`` control char).
    """
    assert mangen._wrap(_WRAP_FILLER + " .foo bar") == _WRAP_FILLER + "\n\\&.foo bar"


def test_wrap_width_is_exactly_72_not_narrower():
    """A word ending at column 72 stays on line one (width is 72, not 70).

    ``ab`` ends exactly at column 72 so it belongs on line one; at the
    dropped-default width 70 it would spill to line two. Kills the
    ``width=72`` -> removed (default 70) mutant.
    """
    assert mangen._wrap(_WRAP_FILLER + " ab cd") == f"{_WRAP_FILLER} ab\ncd"


def test_wrap_width_is_exactly_72_not_wider():
    """A word reaching column 73 spills to line two (width is 72, not 73).

    ``abc`` would end at column 73, one past the limit, so it must wrap.
    Kills the ``width=72`` -> ``width=73`` mutant.
    """
    assert mangen._wrap(_WRAP_FILLER + " abc de") == f"{_WRAP_FILLER}\nabc de"


def test_wrap_does_not_break_long_words():
    """An over-width unbreakable word is kept whole, overflowing the column.

    Kills the ``break_long_words=False`` -> removed/``True`` mutants, which
    would split the 80-char run at column 72.
    """
    assert mangen._wrap("x" * 80) == "x" * 80


def test_wrap_does_not_break_on_hyphens():
    """A hyphenated word is not split at its hyphen to fill a line.

    ``aaaaa-bbbbb`` overflows line one and moves whole to line two; with
    hyphen-breaking it would leave ``aaaaa-`` on line one. Kills the
    ``break_on_hyphens=False`` -> removed/``True`` mutants.
    """
    filler13 = " ".join(["word"] * 13)
    assert mangen._wrap(filler13 + " aaaaa-bbbbb cd") == f"{filler13}\naaaaa-bbbbb cd"


# ---------------------------------------------------------------------------
# _iter_option_params: yields non-argument, non-hidden option params in
# declaration order. Built from bare click.Command/Option/Argument objects so
# the param list and its order are fully controlled.
# ---------------------------------------------------------------------------


def test_iter_option_params_yields_only_options():
    """A visible option is yielded; the duck-typed attribute name matters.

    Kills the ``getattr(None, ...)`` receiver mutant, the
    ``"param_type_name"`` name mutants, and the ``"option"`` literal mutants
    -- each of which makes the type check never match, yielding nothing.
    """
    cmd = click.Command("c", params=[click.Option(["--foo"])])
    assert [p.name for p in mangen._iter_option_params(cmd)] == ["foo"]


def test_iter_option_params_skips_argument_before_option():
    """An argument preceding an option is skipped, not yielded or fatal.

    Kills the ``!=`` -> ``==`` inversion (would yield the argument and drop
    the option) and the first ``continue`` -> ``break`` mutant (would stop at
    the argument and never reach the option).
    """
    cmd = click.Command("c", params=[click.Argument(["arg"]), click.Option(["--foo"])])
    assert [p.name for p in mangen._iter_option_params(cmd)] == ["foo"]


def test_iter_option_params_skips_hidden_option_before_visible():
    """A hidden option preceding a visible one is skipped; iteration continues.

    Kills the ``getattr(None, "hidden", ...)`` receiver mutant and the
    ``"hidden"`` attribute-name mutants (all make the hidden check never
    fire, leaking the hidden option), plus the hidden-branch ``continue`` ->
    ``break`` mutant (would stop before the visible option).
    """
    cmd = click.Command(
        "c",
        params=[
            click.Option(["--sekret"], hidden=True),
            click.Option(["--foo"]),
        ],
    )
    assert [p.name for p in mangen._iter_option_params(cmd)] == ["foo"]


# ---------------------------------------------------------------------------
# _short_help: explicit short_help wins; otherwise the first line of help,
# else the empty string.
# ---------------------------------------------------------------------------


def test_short_help_prefers_explicit_short_help():
    """An explicit ``short_help`` is returned verbatim over the help text.

    Kills the ``short = None`` / ``getattr(None, ...)`` mutants and the
    ``"short_help"`` attribute-name mutants, all of which fall through to the
    first help line (``"Long help."``) instead.
    """
    cmd = click.Command("c", short_help="Explicit summary.", help="Long help.\nMore.")
    assert mangen._short_help(cmd) == "Explicit summary."


def test_short_help_falls_back_to_first_help_line():
    """With no short_help, the first line of help (whole line) is returned.

    Kills the ``help or ""`` -> ``help and ""`` mutant (would return ""), the
    ``split("\n", 1)`` -> ``split(None, 1)`` mutant (would return just
    ``"first"``), the ``rsplit`` mutant (would return the first two lines),
    and the ``split("XX\nXX", 1)`` mutant (would return the whole help text).
    """
    cmd = click.Command("c", help="first line\nsecond\nthird")
    assert mangen._short_help(cmd) == "first line"


def test_short_help_empty_when_no_help_or_short_help():
    """No short_help and no help yields the empty string, not a placeholder.

    Kills the ``or ""`` -> ``or "XXXX"`` mutant, which would surface the
    marker text ``"XXXX"``.
    """
    assert mangen._short_help(click.Command("c")) == ""
