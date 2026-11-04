import json
from pathlib import Path

import pytest
from ofdrw_rs import Document, OfdError

ROOT = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"
CASES = json.loads((ROOT / "canvas-font-style-java.json").read_text(encoding="utf-8"))["cases"]


def test_font_style_java_cases_reuse_resources_and_preserve_base():
    assert len(CASES) == 986
    with Document.new() as doc:
        doc.add_blank_page(40, 30)
        ids = {name: str(doc.add_embedded_font(name, (ROOT / "dejavu-sans.ttf").read_bytes()))
               for name in ["CanvasTest", "宋体"]}
        before = doc.to_bytes()
        for case in CASES:
            base = {"weight": case["base_weight"], "italic": case["base_italic"],
                    "letter_spacing": 0.25, "read_direction": 90, "char_direction": 270, "align": "end"}
            saved = dict(base)
            actual = doc.resolve_canvas_font_style(0, case["input"], base, case["pixels_per_mm"])
            expected = case["expected"]
            assert actual["size"] == pytest.approx(expected["size"], abs=1e-12)
            assert actual["font_id"] == ids[expected["family"]]
            assert actual["weight"] == expected["weight"]
            assert actual["italic"] == expected["italic"]
            assert all(actual[key] == value for key, value in base.items()
                       if key not in ["weight", "italic"])
            assert base == saved
        assert doc.to_bytes() == before
        font = doc.resolve_canvas_font_style(0, "bold 4mm CanvasTest")
        doc.draw_canvas(0, (0, 0, 40, 30), [{"type": "set_font", "font": font},
                        {"type": "fill_text", "text": "Canvas", "point": [2, 10]}])
        with Document.from_bytes(doc.to_bytes()) as reopened:
            assert reopened.validate_references() == []
            assert reopened.page_png(0, 101.6) == doc.page_png(0, 101.6)


def test_font_style_invalid_requests_are_atomic_and_closed_handles_fail():
    with Document.new() as doc:
        doc.add_blank_page(40, 30)
        before = doc.to_bytes()
        for style, font, ppm in [("0mm New", {}, 3.78), ("4mm New", {}, 0),
                                 ("4mm New", {"align": "bad"}, 3.78),
                                 ("4mm New", {"size": -1}, 3.78), ("4mm New\x00", {}, 3.78)]:
            with pytest.raises((OfdError, ValueError)):
                doc.resolve_canvas_font_style(0, style, font, ppm)
        with pytest.raises(TypeError):
            doc.resolve_canvas_font_style(0, 12)
        with pytest.raises(TypeError):
            doc.resolve_canvas_font_style(0, "4mm New", [])
        assert doc.to_bytes() == before
        first = doc.resolve_canvas_font_style(0, "4mm New")
        assert doc.resolve_canvas_font_style(0, "4mm New") == first
    with pytest.raises(Exception):
        doc.resolve_canvas_font_style(0, "4mm New")
