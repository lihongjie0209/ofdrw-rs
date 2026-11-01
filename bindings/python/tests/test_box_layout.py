import json
from pathlib import Path

import pytest
from ofdrw_rs import Document, OfdError


FIXTURES = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"


def test_box_layout_returns_complete_fragments_and_a_reusable_cursor():
    fixture = json.loads((FIXTURES / "box-layout.json").read_text())
    request = fixture["request"]
    items = request.pop("items")
    with Document.new() as doc:
        result = doc.add_box_layout(0, items, **request)
        assert result == fixture["expected"]
        assert len(doc.page_paths(0)) == 1
        assert doc.page_paths(1) == []
        request["start"] = result["next"]
        continued = doc.add_box_layout(0, [{"type": "box", "size": [5, 4],
            "margin": [0.5] * 4, "border": [0.25] * 4, "padding": [0.5] * 4,
            "background_color": [230, 240, 250], "offset": [1, 0]}], **request)
        assert continued["pages"] == [1]
        placed = continued["placements"][0]
        assert placed["allocation"] == [2, 8, 7.5, 6.5]
        assert placed["content_box"] == [4.25, 9.25, 5, 4]
        doc.add_text_object(placed["page_index"], "Hi", placed["content_box"], size=2)
        assert doc.page_text(1) == ["Hi"]
        with Document.from_bytes(doc.to_bytes()) as reopened:
            assert reopened.page_text(1) == ["Hi"]
            assert reopened.page_png(1, 144) == doc.page_png(1, 144)
            assert reopened.validate_references() == []


def test_box_layout_defaults_breaks_and_nonzero_canvas_origin():
    with Document.new() as doc:
        assert doc.add_box_layout(0, []) == {"pages": [], "placements": [], "next": None}
        assert doc.add_box_layout(0, [{"type": "line_break"}])["pages"] == []
        result = doc.add_box_layout(0, [{"type": "page_break"}, {"type": "page_break"}])
        assert result["pages"] == [0, 1]
        assert result["next"] == {"page_index": 1, "y": 25.4}
    with Document.new() as doc:
        doc.add_blank_page(20, 20)
        doc.set_page_area(0, [10, 20, 20, 20])
        result = doc.add_box_layout(0, [{"type": "box", "size": [4, 8], "float": "right", "clear": "none"}],
            page_size=[20, 20], margins=[2] * 4, start={"page_index": 0, "y": 14})
        assert [p["allocation"] for p in result["placements"]] == [[24, 34, 4, 4], [14, 2, 4, 4]]
        assert result["next"] == {"page_index": 1, "y": 6}


def test_box_layout_rejects_invalid_inputs_and_preserves_source():
    doc = Document.new()
    try:
        before = doc.to_bytes()
        valid = {"type": "box", "size": [4, 4], "background_color": [255, 0, 0]}
        for invalid in [
            {"type": "box", "size": [-1, 3]}, {"type": "box", "size": [1, 1e308]},
            {"type": "box", "size": [1, 1], "float": "justify"},
            {"type": "line_break", "size": [1, 1]}, {"type": "box"},
            {"type": "box", "size": [1, 1], "alpha": 256},
        ]:
            with pytest.raises(OfdError):
                doc.add_box_layout(0, [valid, invalid])
            assert doc.to_bytes() == before
        with pytest.raises(OfdError):
            doc.add_box_layout(0, [], font_size=2)
        with pytest.raises(ValueError):
            doc.add_box_layout(0, [{"type": "box", "size": [float("nan"), 1]}])
        with pytest.raises(TypeError):
            doc.add_box_layout(0, {})
        with pytest.raises(IndexError):
            doc.add_box_layout(-1, [])
    finally:
        doc.close()
    with pytest.raises(OfdError):
        doc.add_box_layout(0, [])
