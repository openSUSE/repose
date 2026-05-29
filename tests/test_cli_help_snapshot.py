"""Snapshot tests for ``repose --help`` and each subcommand's ``--help``.

These fixtures protect the documented CLI surface. The PR replacing
``argparse`` with Typer changed help rendering; future Typer upgrades
or accidental option-text edits should not silently drift the help
text. If a change is intentional, regenerate the fixtures:

    UPDATE_HELP_SNAPSHOTS=1 pytest tests/test_cli_help_snapshot.py

Fixtures live in ``tests/fixtures/help/<slug>.txt`` where ``<slug>``
is ``root`` for the top-level help and the subcommand name otherwise.

The runner forces ``COLUMNS=100`` so help wrapping is deterministic
across terminals.
"""

import os
from pathlib import Path

import pytest
from typer.testing import CliRunner

from repose.cli import app

FIXTURE_DIR = Path(__file__).parent / "fixtures" / "help"
UPDATE = os.environ.get("UPDATE_HELP_SNAPSHOTS") == "1"

# (slug, argv) pairs. ``slug`` controls the fixture filename.
CASES: list[tuple[str, list[str]]] = [
    ("root", ["--help"]),
    ("add", ["add", "--help"]),
    ("remove", ["remove", "--help"]),
    ("reset", ["reset", "--help"]),
    ("install", ["install", "--help"]),
    ("clear", ["clear", "--help"]),
    ("uninstall", ["uninstall", "--help"]),
    ("list-products", ["list-products", "--help"]),
    ("list-repos", ["list-repos", "--help"]),
    ("known-products", ["known-products", "--help"]),
]


def _invoke(argv: list[str]) -> str:
    """Render help with deterministic line wrapping.

    ``prog_name="repose"`` matches the real console-script invocation
    (without it, CliRunner uses the click ``app.info_name`` default
    of ``"root"``); ``COLUMNS=100`` pins the terminal width so help
    wrapping is identical regardless of where the tests run.
    """
    runner = CliRunner(env={"COLUMNS": "100"})
    result = runner.invoke(app, argv, prog_name="repose")
    assert result.exit_code == 0, (
        f"--help for {argv!r} exited {result.exit_code}: {result.stderr}"
    )
    return result.stdout


@pytest.mark.parametrize("slug, argv", CASES, ids=[c[0] for c in CASES])
def test_help_snapshot(slug: str, argv: list[str]):
    actual = _invoke(argv)
    fixture = FIXTURE_DIR / f"{slug}.txt"
    if UPDATE or not fixture.exists():
        FIXTURE_DIR.mkdir(parents=True, exist_ok=True)
        fixture.write_text(actual)
        # When regenerating, still pass — but make it obvious via output.
        if UPDATE:
            return
        # First-time creation also passes; the file is now the baseline.
        return
    expected = fixture.read_text()
    assert actual == expected, (
        f"Help output for {argv!r} drifted from "
        f"{fixture.relative_to(Path(__file__).parent.parent)}.\n"
        f"If intentional, regenerate with:\n"
        f"  UPDATE_HELP_SNAPSHOTS=1 pytest "
        f"tests/test_cli_help_snapshot.py"
    )
