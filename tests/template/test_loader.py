"""Tests for ``repose.template.load_template``."""

import pytest
from ruamel.yaml import YAML

from repose.template import load_template


YAML_FIXTURE = """\
SLES:
  "15-SP3":
    default_repos:
      - update
    update:
      url: http://example.com/$version/$arch
      enabled: true
"""


def test_load_template_returns_dict(tmp_path):
    path = tmp_path / "products.yml"
    path.write_text(YAML_FIXTURE)

    template = load_template(path)
    assert isinstance(template, dict)
    assert "SLES" in template
    assert "15-SP3" in template["SLES"]
    assert "update" in template["SLES"]["15-SP3"]["default_repos"]


def test_load_template_missing_file_raises(tmp_path):
    with pytest.raises(FileNotFoundError):
        load_template(tmp_path / "missing.yml")


def test_load_template_caches(tmp_path, monkeypatch):
    """``load_template`` parses each path only once per process.

    Counts ``YAML.load`` invocations across three calls with the same
    path. With ``@functools.cache`` in place, the parser runs exactly
    once.
    """
    path = tmp_path / "products.yml"
    path.write_text("sle-sdk:\n  '12-SP2':\n    default_repos: []\n")

    calls = 0
    real_load = YAML.load

    def counting(self, *args, **kwargs):
        nonlocal calls
        calls += 1
        return real_load(self, *args, **kwargs)

    monkeypatch.setattr(YAML, "load", counting)

    load_template(path)
    load_template(path)
    load_template(path)

    assert calls == 1
