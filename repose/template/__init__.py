import functools
from pathlib import Path
from typing import Any

from ruamel.yaml import YAML


@functools.cache
def load_template(path: Path) -> dict[str, Any]:
    """Load and return the products YAML template file.

    The result is cached for the lifetime of the process keyed by
    ``path``. This is safe for the CLI (one invocation = one process)
    but tests that mutate the same path between cases MUST call
    ``load_template.cache_clear()`` to avoid stale data. The
    ``_clear_template_cache`` autouse fixture in ``tests/conftest.py``
    handles this globally.

    Args:
        path: Path to the products YAML configuration file.

    Returns:
        Dictionary mapping product names to their template definitions.
    """
    with path.open(mode="r", encoding="utf-8") as f:
        template: dict[str, Any] = YAML(typ="safe").load(f)
    return template
