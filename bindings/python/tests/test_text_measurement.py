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


def test_natural_zero_advance_survives_measurement_and_flow_without_mutating_source():
    with Document.new() as doc:
        font = doc.add_embedded_font("DejaVu Sans", (FIXTURES / "dejavu-sans.ttf").read_bytes())
        options = {"font_id": font, "font_size": 2, "line_height": 3}
        spans = [{"text": "\u0301"}]
        before = doc.to_bytes()
        measured = doc.measure_flowed_spans(0, spans, 20, **options)
        assert measured["width"] == 0
        assert measured["height"] == 3
        assert measured["lines"][0]["text"] == "\u0301"
        assert doc.to_bytes() == before
        with pytest.raises(OfdError):
            doc.measure_flowed_spans(0, [{"text": "\u0301", "letter_spacing": -0.1}], 20, **options)
        assert doc.to_bytes() == before
        assert doc.add_flowed_spans(0, spans, **options) == [0]
        assert doc.page_text(0) == ["\u0301"]
        assert doc.page_png(0, 30)


@pytest.mark.parametrize("shape", [False, True])
@pytest.mark.parametrize("case", json.loads((FIXTURES / "inline-placeholders-java.json").read_text())["cases"], ids=lambda c: c["name"])
def test_inline_placeholders_match_java_without_allocating_fonts_or_objects(case, shape):
    with Document.new() as doc:
        before = doc.to_bytes()
        options = {**case["style"], "shape": shape}
        metrics = doc.measure_flowed_spans(0, case["spans"], case["content_width"], **options)
        assert len(metrics["lines"]) == len(case["lines"])
        for line, expected in zip(metrics["lines"], case["lines"]):
            assert {key: line[key] for key in expected} == expected
        assert metrics["width"] == max(line["width"] for line in case["lines"])
        assert metrics["height"] == sum(line["height"] for line in case["lines"])
        assert doc.to_bytes() == before
        doc.add_flowed_spans(0, case["spans"], **options,
            page_size=[case["content_width"] + 4, 50], margins=[2] * 4)
        assert doc.page_count == 1
        assert doc.page_text(0) == []
        assert doc.page_objects(0) == []
        with Document.from_bytes(doc.to_bytes()) as reopened:
            assert reopened.page_text(0) == []
            assert reopened.page_png(0, 30)


@pytest.mark.parametrize("shape", [False, True])
def test_placeholder_spans_reach_every_container_writer_and_invalid_input_rolls_back(shape):
    spans = [{"text": "", "placeholder_width": 3, "font_size": 5},
             {"text": "", "placeholder_width": 2, "font_size": 2}]
    style = {"font_size": 2, "line_height": 3, "shape": shape}
    root = {"layout": {"size": [10, 0]}, "children": [{"type": "paragraph", "spans": spans, "style": style}]}
    options = {"page_size": [30, 20], "margins": [2] * 4}
    with Document.new() as doc:
        doc.add_blank_page(30, 20)
        fixed = doc.add_fixed_containers(0, [{"position": [2, 2], "container": root}])
        boxes = doc.add_text_boxes(0, [{"layout": root["layout"], "spans": spans, "style": style}], **options)
        flowed = doc.add_flow_containers(0, [root], **options)
        assert fixed[0]["content_box"] == boxes["placements"][0]["content_box"] == flowed["placements"][0]["content_box"] == [2, 2, 10, 6]
        doc.add_flow_blocks(0, [{"type": "paragraph", "spans": spans}], **style, **options)
        assert doc.page_count == 4
        assert all(doc.page_objects(page) == [] for page in range(4))
        before = doc.to_bytes()
        for width in [-1, 27, None, True, "wide", float("nan"), float("inf")]:
            bad = {"text": "", "placeholder_width": width, "font_size": 2}
            with pytest.raises((OfdError, ValueError)):
                doc.measure_flowed_spans(0, [bad], 26, **style)
            with pytest.raises((OfdError, ValueError)):
                doc.add_flowed_spans(0, [*spans, bad], **style, **options)
            assert doc.to_bytes() == before
        with pytest.raises(OfdError):
            doc.add_flowed_spans(0, [{"text": "hidden", "placeholder_width": 1}], **style, **options)
        assert doc.to_bytes() == before
