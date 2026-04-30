"""Tests for ``repose.types.refhost.transformations``."""

import pytest
from hypothesis import given, strategies as st

from repose.types.refhost.transformations import transform_version_partialy


@pytest.mark.parametrize(
    "version,expected",
    [
        ("15-SP3", {"major": 15, "minor": "SP3"}),
        ("12-SP5", {"major": 12, "minor": "SP5"}),
        ("15.3", {"major": 15, "minor": 3}),
        ("ALL", {"major": "ALL"}),
        ("15", {"major": 15}),
        ("7", {"major": 7}),
    ],
)
def test_transformations_table(version, expected):
    assert transform_version_partialy(version) == expected


@given(major=st.integers(min_value=1, max_value=99))
def test_plain_integer_string(major):
    assert transform_version_partialy(str(major)) == {"major": major}


@given(
    major=st.integers(min_value=1, max_value=99),
    sp=st.integers(min_value=0, max_value=9),
)
def test_sp_form_property(major, sp):
    result = transform_version_partialy(f"{major}-SP{sp}")
    assert result == {"major": major, "minor": f"SP{sp}"}
