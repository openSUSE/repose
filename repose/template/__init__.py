from pathlib import Path
from typing import Any

from ruamel.yaml import YAML


def load_template(path: Path) -> dict[str, Any]:
    """Load and return the products YAML template file.

    Args:
        path: Path to the products YAML configuration file.

    Returns:
        Dictionary mapping product names to their template definitions.
    """
    with path.open(mode="r", encoding="utf-8") as f:
        template: dict[str, Any] = YAML(typ="safe").load(f)
    return template
