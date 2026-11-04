import json
from pathlib import Path

import pytest
from ofdrw_rs import Document, OfdError

ROOT = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"
CASES = json.loads((ROOT / "canvas-pattern-java/reference.json").read_text(encoding="utf-8"))["cases"]
SOURCE = (ROOT / "canvas-images-java/source.png").read_bytes()


@pytest.mark.parametrize("case", CASES, ids=lambda case: case["name"])
def test_canvas_patterns_match_java_writer_and_preserve_image_ctm(case):
    with Document.new() as doc, Document.open(ROOT / f"canvas-pattern-java/{case['name']}.ofd") as java:
        doc.add_blank_page(40, 30)
        doc.set_page_area(0, (0, 0, 40, 30), application_box=(0, 0, 40, 30))
        pattern = doc.create_canvas_pattern(0, memoryview(SOURCE), "png", case["repetition"], case["ppm"],
                                            case.get("size"), case.get("transform"))
        assert pattern["width"] == case["expected"]["width"]
        assert pattern["height"] == case["expected"]["height"]
        assert pattern["cells"][0]["ctm"] == case["expected"]["image_ctm"]
        assert pattern["cells"][0]["boundary"] == case["expected"]["image_boundary"]
        assert doc.page_paths(0) == [] and doc.page_images(0) == []
        style = {"stroke_pattern" if case.get("stroke") else "fill_pattern": pattern}
        if case.get("stroke"):
            style["line_width"] = 1.2
        commands = [{"type": "set_draw_param", "style": style}]
        if "canvas_ctm" in case:
            commands.append({"type": "set_transform", "matrix": case["canvas_ctm"]})
        if "alpha" in case:
            commands.append({"type": "set_global_alpha", "alpha": case["alpha"]})
        commands += ([{"type": "rect", "rect": [2, 2, 18, 12]}, {"type": "stroke"}]
                     if case.get("stroke") else [{"type": "fill_rect", "rect": [2, 2, 18, 12]}])
        saved = json.dumps(pattern, sort_keys=True)
        doc.draw_canvas(0, (4, 3, 24, 20), commands)
        assert json.dumps(pattern, sort_keys=True) == saved
        with Document.from_bytes(doc.to_bytes()) as reopened:
            for dpi in [25.4, 101.6]:
                png = doc.page_png(0, dpi)
                assert png == java.page_png(0, dpi)
                assert reopened.page_png(0, dpi) == png
            assert reopened.validate_references() == []


def test_pattern_failures_roll_back_resources_and_closed_handles_fail():
    with Document.new() as doc:
        doc.add_blank_page(40, 30)
        before = doc.to_bytes()
        for options in [{"repetition": "repeat-x"}, {"pixels_per_mm": 0}, {"image_size": [0, 4]},
                        {"image_size": [4]}, {"ctm": [1, 2]}, {"ctm": [1, 0, 0, 1, float("nan"), 0]}]:
            with pytest.raises((OfdError, ValueError)):
                doc.create_canvas_pattern(0, SOURCE, "png", **options)
            assert doc.to_bytes() == before
        for data, format in [(b"bad", "png"), (b"", "png"), (SOURCE, "jpeg")]:
            with pytest.raises(OfdError):
                doc.create_canvas_pattern(0, data, format)
            assert doc.to_bytes() == before
        with pytest.raises(OfdError):
            doc.create_canvas_pattern(1, SOURCE, "png")
        assert doc.to_bytes() == before
    with pytest.raises(OfdError):
        doc.create_canvas_pattern(0, SOURCE, "png")
