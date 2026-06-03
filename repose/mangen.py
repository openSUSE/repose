"""Generate man pages for ``repose`` and its subcommands.

Walks the Typer/Click app object and emits one ``.1`` man page per
command into ``docs/man/``. The output is committed to the repo so
downstream packagers don't need ``click-man`` at build time; a CI job
regenerates and ``git diff --exit-code``s to catch drift.

Reproducibility: ``click-man`` stamps the current date into each
page's ``.TH`` header by default, which would cause spurious diffs
on every run. We pin the date via ``SOURCE_DATE_EPOCH``
(reproducible-builds standard) — ``click_man.man.ManPage`` honours
that env var natively. The ``write_man_pages`` ``date=`` keyword
argument is broken in click-man 0.5.1 (only forwarded to recursive
children, never applied to the page being generated), so we
manipulate the env instead and restore it on exit.

If callers already export ``SOURCE_DATE_EPOCH`` (e.g. distro builds),
their value wins.
"""

import os
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator

import typer
from click_man.core import write_man_pages

from repose import __version__
from repose.cli import app

# Fallback epoch (UTC ``2024-01-01 00:00:00``) used when callers
# haven't already exported ``SOURCE_DATE_EPOCH``. Pinning a constant
# keeps ``uv run repose-mangen`` byte-stable across machines and dates;
# CI uses this to catch drift via ``git diff --exit-code``.
_FALLBACK_SOURCE_DATE_EPOCH = "1704067200"  # 2024-01-01T00:00:00Z


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
            # and is therefore ``str`` (never ``None``). The assert
            # narrows the type for the static checker.
            assert prior is not None
            os.environ["SOURCE_DATE_EPOCH"] = prior
        else:
            os.environ.pop("SOURCE_DATE_EPOCH", None)


def main() -> None:
    """Regenerate ``docs/man/repose*.1`` from the Typer app."""
    out = Path(__file__).resolve().parent.parent / "docs" / "man"
    out.mkdir(parents=True, exist_ok=True)
    click_group = typer.main.get_command(app)
    with _pinned_source_date():
        write_man_pages(
            name="repose",
            cli=click_group,
            version=__version__,
            target_dir=str(out),
        )


if __name__ == "__main__":
    main()
