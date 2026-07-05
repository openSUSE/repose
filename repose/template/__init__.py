import functools
from pathlib import Path
from typing import Any

from ruamel.yaml import YAML


class TemplateError(ValueError):
    """The products template file is structurally invalid.

    Raised by :func:`load_template` when the top-level YAML node is not
    a mapping. Subclasses ``ValueError`` so broad handlers keep working,
    while letting ``repose.cli._dispatch`` translate it into a friendly
    one-liner without also swallowing unrelated ``ValueError``s raised
    deep inside command execution (e.g. unresolvable REPAs or host
    repository read failures).
    """


@functools.cache
def load_template(path: Path) -> dict[str, Any]:
    """Load and return the products YAML template file.

    An empty or comment-only file loads as ``None`` and is normalized
    to an empty mapping so consumers can iterate ``.keys()``/``.items()``
    without a ``None`` guard. Any other non-mapping top level (e.g. a
    YAML sequence or a bare scalar) is a configuration error and raises
    :class:`TemplateError` instead of surfacing later as
    ``AttributeError``.

    The result is cached for the lifetime of the process keyed by
    ``path``. This is safe for the CLI (one invocation = one process)
    but tests that mutate the same path between cases MUST call
    ``load_template.cache_clear()`` to avoid stale data. The
    ``_clear_template_cache`` autouse fixture in ``tests/conftest.py``
    handles this globally.

    Args:
        path: Path to the products YAML configuration file.

    Returns:
        Dictionary mapping product names to their template definitions;
        empty if the file has no YAML content.

    Raises:
        TemplateError: If the file's top-level YAML node is not a
            mapping.
    """
    with path.open(mode="r", encoding="utf-8") as f:
        template = YAML(typ="safe").load(f)
    if template is None:
        return {}
    if not isinstance(template, dict):
        raise TemplateError(
            f"template {path} must be a YAML mapping, got {type(template).__name__}"
        )
    return template
