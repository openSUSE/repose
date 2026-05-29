"""Console-script entry-point shim.

The real CLI lives in :mod:`repose.cli` as a Typer app. ``pyproject``
points the ``repose`` console script directly at ``repose.cli:app``;
this module is retained so downstream callers that do
``from repose.main import main`` (distro packaging, .spec files,
ad-hoc scripts) keep working unchanged.
"""

from repose.cli import app


def main() -> None:
    app()
