import copy
import json
from pathlib import Path

import pytest
from ofdrw_rs import Document, OfdError

FIXTURES = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"


@pytest.mark.parametrize("shape", [False, True])
def test_text_boxes_match_all_reference_fragments_draw_and_continue(shape):
    fixture = json.loads((FIXTURES / "text-boxes.json").read_text())
    request = fixture["request"]
    boxes = request.pop("boxes")
    with Document.new() as doc:
        font = doc.add_embedded_font("DejaVu Sans", (FIXTURES / "dejavu-sans.ttf").read_bytes())
        boxes[0]["style"].update(font_id=font, shape=shape)
        original = copy.deepcopy(boxes)
        result = doc.add_text_boxes(0, boxes, **request)
        assert result == fixture["expected"]
        assert boxes == original
        assert doc.page_text(0) == ["A", "B", "C"]
        assert doc.page_text(1) == ["D"]
        reopened = Document.from_bytes(doc.to_bytes())
        try:
            assert reopened.page_text(1) == ["D"]
            assert reopened.page_png(0, 30)
        finally:
            reopened.close()
        continued = doc.add_text_boxes(0, [{**boxes[0], "spans": [{"text": "E"}]}],
                                       **request, start=result["next"])
        assert continued["pages"] == [1]
        assert doc.page_text(1) == ["D", "E"]


def test_invalid_text_boxes_are_atomic_and_wrapper_guards_apply():
    with Document.new() as doc:
        boxes = [{"layout": {"size": [8, 0]}, "spans": [{"text": "A"}]}]
        before = doc.to_bytes()
        for bad in [{}, {"layout": {"size": [0, 0]}, "spans": [{"text": "A"}]},
                    {**boxes[0], "spans": []}, {**boxes[0], "style": False},
                    {**boxes[0], "unknown": True}]:
            with pytest.raises(OfdError):
                doc.add_text_boxes(0, [*boxes, bad])
            assert doc.to_bytes() == before
        with pytest.raises(ValueError):
            doc.add_text_boxes(0, [{**boxes[0], "layout": {"size": [float("nan"), 0]}}])
        with pytest.raises(TypeError):
            doc.add_text_boxes(0, {})
        with pytest.raises(IndexError):
            doc.add_text_boxes(-1, boxes)
    with pytest.raises(OfdError):
        doc.add_text_boxes(0, boxes)
