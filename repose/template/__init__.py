

from ruamel.yaml import YAML

def load_template(path):
    with path.open(mode='r', encoding='utf-8') as f:
        template = YAML(typ='safe').load(f)
    return template
