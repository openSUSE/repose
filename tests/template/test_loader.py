"""Tests for ``repose.template.load_template``."""

import pytest

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
