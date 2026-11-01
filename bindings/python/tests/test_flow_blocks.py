import base64
import copy
import json
from pathlib import Path

import pytest
from ofdrw_rs import Document, OfdError


FIXTURES = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"


@pytest.mark.parametrize("shape", [False, True])
def test_mixed_flow_serializes_images_and_fonts_and_returns_a_reusable_cursor(shape):
    fixture = json.loads((FIXTURES / "flow-blocks.json").read_text())
    request = fixture["request"]
    blocks = request.pop("blocks")
    blocks[1]["data"] = base64.b64decode(blocks[1].pop("data_base64"))
    original = copy.deepcopy(blocks)
    with Document.new() as doc:
        request["font_id"] = doc.add_embedded_font("DejaVu Serif", (FIXTURES / "dejavu-serif.ttf").read_bytes())
        request["shape"] = shape
        result = doc.add_flow_blocks(0, blocks, **request)
        assert result == fixture["expected"]
        assert blocks == original
        assert doc.page_text(0) == ["A", "R1"]
        assert doc.page_text(1) == ["R2", "B"]
        assert doc.page_images(0)[0]["boundary"] == [7, 6, 6, 4]
        assert doc.validate_references() == []
        request["start"] = result["next"]
        result = doc.add_flow_blocks(0, [{"type": "paragraph", "text": "C"}], **request)
        assert result == {"pages": [1], "next": {"page_index": 1, "y": 13}}
        assert doc.page_text_objects(1)[-1]["boundary"][:2] == [2, 10]
        with Document.from_bytes(doc.to_bytes()) as reopened:
            assert reopened.page_text(1) == ["R2", "B", "C"]
            assert reopened.page_png(0, 72) == doc.page_png(0, 72)


def test_mixed_flow_errors_are_atomic_and_closed_documents_are_rejected():
    fixture = json.loads((FIXTURES / "flow-blocks.json").read_text())
    request = fixture["request"]
    blocks = request.pop("blocks")
    doc = Document.new()
    try:
        before = doc.to_bytes()
        for bad in [
            {"type": "paragraph", "text": "x", "style": {"font_size": 0}},
            {"type": "paragraph", "text": "x", "style": {"page_size": [20, 20]}},
            {"type": "image", "data": b"", "format": "PNG", "width": 2, "height": 2},
            {"type": "spacer", "height": 1e308},
            {"type": "paragraph", "text": "x", "spans": []},
        ]:
            with pytest.raises(OfdError):
                doc.add_flow_blocks(0, [*blocks, bad], **request)
            assert doc.to_bytes() == before
        with pytest.raises(ValueError):
            doc.add_flow_blocks(0, [{"type": "spacer", "height": float("nan")}])
        with pytest.raises(IndexError):
            doc.add_flow_blocks(-1, blocks)
        with pytest.raises(TypeError):
            doc.add_flow_blocks(0, {})
        cycle = {"type": "paragraph", "text": "x"}
        cycle["style"] = cycle
        with pytest.raises(ValueError, match="nesting"):
            doc.add_flow_blocks(0, [cycle])
        assert doc.add_flow_blocks(0, []) == {"pages": [], "next": None}
    finally:
        doc.close()
    with pytest.raises(OfdError):
        doc.add_flow_blocks(0, blocks)


def test_mixed_flow_paragraph_overrides_and_explicit_breaks():
    with Document.new() as doc:
        result = doc.add_flow_blocks(0, [
            {"type": "paragraph", "spans": [{"text": "A", "font_id": 1}], "style": {"font_size": 4}},
            {"type": "spacer", "height": 2},
            {"type": "paragraph", "text": "B"},
            {"type": "page_break"}, {"type": "page_break"},
        ], page_size=[20, 20], margins=[2] * 4, font_size=2, line_height=3, block_gap=1,
            font_id=None, language=None)
        assert result == {"pages": [0, 1, 2], "next": {"page_index": 2, "y": 2}}
        assert doc.page_text_objects(0)[1]["boundary"][:2] == [2, 10]
        assert doc.page_text(1) == []
