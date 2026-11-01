import json
from pathlib import Path

import pytest
from ofdrw_rs import Document, OfdError


FIXTURES = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"


@pytest.mark.parametrize("shape", [False, True])
def test_measurement_matches_font_reference_and_sizes_a_box_without_mutating_source(shape):
    fixture = json.loads((FIXTURES / "text-measurement.json").read_text())
    options = fixture["request"]
    spans = options.pop("spans")
    width = options.pop("content_width")
    with Document.new() as doc:
        options["font_id"] = doc.add_embedded_font("DejaVu Serif", (FIXTURES / fixture["font_fixture"]).read_bytes())
        options["shape"] = shape
        before = doc.to_bytes()
        metrics = doc.measure_flowed_spans(0, spans, width, **options)
        assert metrics == fixture["expected"]
        assert doc.to_bytes() == before
        assert doc.page_count == 0
        ignored = {**options, "page_size": [0, 0], "margins": [-1] * 4,
                   "start": {"page_index": 999, "y": -1}, "keep_paragraphs_together": True}
        assert doc.measure_flowed_spans(0, spans, width, **ignored) == metrics
        result = doc.add_box_layout(0, [{"type": "box", "size": [width, metrics["height"]],
            "padding": [1] * 4, "border": [0.2] * 4, "integrity": True}],
            page_size=[30, 80], margins=[2] * 4)
        placed = result["placements"][0]
        x, y, w, h = placed["content_box"]
        assert h == metrics["height"]
        draw = {**options, "page_size": [30, 80], "margins": [y, 30-x-w, 80-y-h, x],
                "start": {"page_index": placed["page_index"], "y": y}}
        assert doc.add_flowed_spans(0, spans, **draw) == [0]
        objects = doc.page_text_objects(0)
        visible = [line for line in metrics["lines"] if line["text"]]
        assert len(objects) == len(visible)
        for obj, line in zip(objects, visible):
            assert obj["boundary"][:2] == pytest.approx([x+line["x"], y+line["y"]])
            assert obj["boundary"][3] == line["height"]
        with Document.from_bytes(doc.to_bytes()) as reopened:
            assert reopened.page_text(0) == ["A", "B"]
            assert reopened.page_png(0, 144) == doc.page_png(0, 144)


def test_measurement_defaults_terminal_breaks_and_error_paths_are_read_only():
    doc = Document.new()
    try:
        before = doc.to_bytes()
        assert doc.measure_flowed_spans(0, [{"text": "A", "font_id": None}], 20,
            font_id=None, language=None)["height"] == 6
        assert len(doc.measure_flowed_spans(0, [{"text": "A\n\n"}], 20)["lines"]) == 1
        for width in [0, -1, float("nan"), float("inf")]:
            with pytest.raises((OfdError, ValueError)):
                doc.measure_flowed_spans(0, [{"text": "A"}], width)
        for spans in [[], [{"text": "A", "font_id": "missing"}], [{"text": "\n"}]]:
            with pytest.raises(OfdError):
                doc.measure_flowed_spans(0, spans, 20)
        with pytest.raises(TypeError):
            doc.measure_flowed_spans(0, {}, 20)
        with pytest.raises(IndexError):
            doc.measure_flowed_spans(-1, [{"text": "A"}], 20)
        assert doc.to_bytes() == before
    finally:
        doc.close()
    with pytest.raises(OfdError):
        doc.measure_flowed_spans(0, [{"text": "A"}], 20)
