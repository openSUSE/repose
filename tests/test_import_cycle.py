"""Guard against import cycles between ``repose.types`` and ``repose.target``.

``repose.types.repositories`` needs ``repose.target.parsers.Product`` and
``repose.target.__init__`` needs ``repose.types.repositories.Repositories``.
If either is imported at module scope on both sides, importing whichever
module first triggers an ``ImportError`` on a partially-initialised module.

These tests import each module *first* in a clean interpreter (a
subprocess, so the test session's already-populated ``sys.modules``
doesn't mask the cycle).
"""

import subprocess
import sys

import pytest


@pytest.mark.parametrize(
    "module",
    [
        "repose.types.repositories",
        "repose.types.system",
        "repose.target",
    ],
)
def test_module_imports_in_isolation(module):
    proc = subprocess.run(
        [sys.executable, "-c", f"import {module}"],
        capture_output=True,
        text=True,
    )
    assert proc.returncode == 0, proc.stderr
