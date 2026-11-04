import json
from pathlib import Path

import pytest
from ofdrw_rs import parse_canvas_color

ROOT = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"


def test_canvas_colors_match_all_java_reference_cases():
    cases = json.loads((ROOT / "canvas-color-java.json").read_text(encoding="utf-8"))["cases"]
    assert len(cases) == 461
    for case in cases:
        values = case["expected"]
        expected = None if values is None else {"color": values[:3], "alpha": values[3] if len(values) == 4 else 255}
        assert parse_canvas_color(case["input"]) == expected, case


def test_canvas_colors_reject_nul_and_wrong_types_and_return_none_for_invalid_colors():
    with pytest.raises(ValueError):
        parse_canvas_color("red\0blue")
    for value in [None, 1, b"red", []]:
        with pytest.raises(TypeError):
            parse_canvas_color(value)
    for value in ["rgb(999,0,0)", "rgba(1,2,3,NaN)", "x" * 4097]:
        assert parse_canvas_color(value) is None
