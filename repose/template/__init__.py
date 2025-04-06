from typing import Any

from ruamel.yaml import YAML


def load_template(path) -> Any:
    with path.open(mode="r", encoding="utf-8") as f:
        template = YAML(typ="safe").load(f)
    return template
