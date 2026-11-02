import copy
import json
from pathlib import Path

import pytest
from ofdrw_rs import Document, OfdError

ROOT = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/canvas-java"
REQUESTS = json.loads((ROOT / "requests.json").read_text())
REFERENCES = {case["name"]: case for case in json.loads((ROOT / "reference.json").read_text())["cases"]}


@pytest.mark.parametrize("case", REQUESTS["cases"], ids=lambda case: case["name"])
def test_canvas_matches_java_reference_geometry_and_pixels(case):
    reference = REFERENCES[case["name"]]
    file = reference.get("render_reference", case["name"] + ".ofd")
    commands = copy.deepcopy(case["commands"])
    with Document.new() as doc, Document.open(ROOT / file) as java:
        doc.add_blank_page(40, 30)
        doc.set_page_area(0, (0, 0, 40, 30), application_box=(0, 0, 40, 30))
        ids = doc.draw_canvas(0, REQUESTS["boundary"], commands)
        assert commands == case["commands"]
        assert len(ids) == len(reference["objects"])
        assert [path["id"] for path in doc.page_paths(0)] == ids
        assert all(isinstance(value, str) for value in ids)
        assert [path["style"]["line_width"] for path in doc.page_object_stroke_styles(0)] == [entry["line_width"] for entry in reference["objects"]]
        for dpi in [25.4, 101.6]:
            assert doc.page_png(0, dpi) == java.page_png(0, dpi)
        with Document.from_bytes(doc.to_bytes()) as reopened:
            assert reopened.validate_references() == []
            assert reopened.page_png(0, 101.6) == doc.page_png(0, 101.6)


def test_canvas_zero_width_opacity_clear_draw_parameters_and_empty_batches():
    with Document.new() as doc:
        doc.add_blank_page(40, 30)
        before = doc.to_bytes()
        assert doc.draw_canvas(0, (0, 0, 40, 30), []) == []
        assert doc.to_bytes() == before
        ids = doc.draw_canvas(0, (0, 0, 40, 30), [
            {"type": "set_draw_param", "style": {"line_width": 0, "cap": "round", "dash_sequence": [1, 2, 3], "stroke": {"color": [0, 0, 0]}}},
            {"type": "rotate", "angle": 0}, {"type": "rect", "rect": [2, 2, 12, 8]},
            {"type": "set_global_alpha", "alpha": 0}, {"type": "stroke"},
            {"type": "set_global_alpha", "alpha": None}, {"type": "stroke"},
            {"type": "set_global_alpha"}, {"type": "fill", "even_odd": True},
        ])
        assert len(ids) == 3
        assert [item["alpha"] for item in doc.page_objects(0)] == [0, 255, 255]
        assert all(item["style"]["line_width"] == 0 for item in doc.page_object_stroke_styles(0))
        assert 'fill-rule="evenodd"' in doc.page_svg(0)


def test_canvas_rejects_malformed_unknown_and_late_invalid_commands_atomically():
    with Document.new() as doc:
        doc.add_blank_page(40, 30)
        before = doc.to_bytes()
        for bad in [
            {"type": "unknown"}, {"type": "stroke", "extra": 1}, {"type": "begin_path", "point": [1, 2]},
            {"type": "move_to", "point": [1]}, {"type": "move_to", "point": [1, 2], "extra": True},
            {"type": "set_line_width", "width": -1}, {"type": "set_global_alpha", "alpha": 1.1},
            {"type": "set_global_alpha", "alpha": float("nan")}, {"type": "fill", "even_odd": 0},
            {"type": "set_draw_param", "style": {"extra": True}}, {"type": "set_fill_color", "color": [256, 0, 0]},
        ]:
            with pytest.raises((OfdError, ValueError)):
                doc.draw_canvas(0, (0, 0, 40, 30), [{"type": "rect", "rect": [1, 1, 4, 4]}, {"type": "stroke"}, bad])
            assert doc.to_bytes() == before
        with pytest.raises(TypeError):
            doc.draw_canvas(0, (0, 0, 40, 30), {})
        with pytest.raises(IndexError):
            doc.draw_canvas(-1, (0, 0, 40, 30), [])
        with pytest.raises(OfdError):
            doc.draw_canvas(9, (0, 0, 40, 30), [])
        with pytest.raises(OfdError):
            doc.draw_canvas(0, (0, 0, 0, 30), [])
    with pytest.raises(OfdError):
        doc.draw_canvas(0, (0, 0, 40, 30), [])


IMAGE_ROOT = ROOT.parent / "canvas-images-java"
IMAGE_REQUESTS = json.loads((IMAGE_ROOT / "requests.json").read_text())


@pytest.mark.parametrize("case", IMAGE_REQUESTS["cases"], ids=lambda case: case["name"])
def test_canvas_image_bytes_and_base64_match_java(case):
    import base64
    def strided_view(data):
        padded = bytearray(len(data) * 2)
        padded[::2] = data
        return memoryview(padded)[::2]

    for kind in (bytes, bytearray, memoryview, strided_view, None):
        commands = copy.deepcopy(case["commands"])
        original = []
        for command in commands:
            if "data_base64" in command and kind is not None:
                data = kind(base64.b64decode(command.pop("data_base64")))
                command["data"] = data
                original.append((command, data, bytes(data)))
        with Document.new() as doc, Document.open(IMAGE_ROOT / (case["name"] + ".ofd")) as java:
            doc.add_blank_page(40, 30)
            doc.set_page_area(0, (0, 0, 40, 30), application_box=(0, 0, 40, 30))
            ids = doc.draw_canvas(0, IMAGE_REQUESTS["boundary"], commands)
            assert len(ids) == 1
            assert [item["id"] for item in doc.page_images(0)] == ids
            for command, data, before in original:
                assert command["data"] is data
                assert bytes(data) == before
                assert "data_base64" not in command
            for dpi in (25.4, 101.6):
                assert doc.page_png(0, dpi) == java.page_png(0, dpi)
            with Document.from_bytes(doc.to_bytes()) as reopened:
                assert reopened.validate_references() == []
                assert reopened.page_png(0, 101.6) == doc.page_png(0, 101.6)


def test_canvas_image_rejects_invalid_bytes_and_conflicting_encodings_atomically():
    data = (IMAGE_ROOT / "source.png").read_bytes()
    valid = {"type": "draw_image", "data": data, "format": "png", "rect": [1, 1, 6, 4]}
    with Document.new() as doc:
        doc.add_blank_page(40, 30)
        before = doc.to_bytes()
        for bad in [
            {**valid, "data": "not bytes"}, {**valid, "data": None},
            {**valid, "data_base64": ""}, {**valid, "data": b"bad"},
            {"type": "draw_image_at", "data": data, "format": "png", "point": [0, 0], "pixels_per_mm": 0},
        ]:
            with pytest.raises((TypeError, ValueError, OfdError)):
                doc.draw_canvas(0, (0, 0, 40, 30), [valid, bad])
            assert doc.to_bytes() == before


TEXT_ROOT = ROOT.parent / "canvas-text-java"
TEXT_REQUESTS = json.loads((TEXT_ROOT / "requests.json").read_text())


def test_canvas_text_metrics_and_drawing_match_all_java_directions():
    with Document.new() as doc, Document.open(TEXT_ROOT / "directions.ofd") as java:
        doc.add_blank_page(90, 90)
        doc.set_page_area(0, (0, 0, 90, 90), application_box=(0, 0, 90, 90))
        font_id = doc.add_embedded_font("CanvasTest", (TEXT_ROOT.parent / "dejavu-sans.ttf").read_bytes())
        before = doc.to_bytes()
        commands = [{"type": "translate", "offset": [1, 1]}, {"type": "set_global_alpha", "alpha": 0.5}]
        for case in TEXT_REQUESTS["cases"]:
            font = {**case["font"], "font_id": font_id}
            snapshot = copy.deepcopy(font)
            assert doc.measure_canvas_text(0, TEXT_REQUESTS["text"], font) == case["metrics"]
            assert font == snapshot
            commands.extend([
                {"type": "set_font", "font": font},
                {"type": "fill_text", "text": TEXT_REQUESTS["text"], "point": case["point"]},
            ])
        controls = json.loads((TEXT_ROOT / "controls.json").read_text())
        for case in controls["cases"]:
            text = "A" + chr(case["code"]) + "B"
            assert doc.measure_canvas_text(0, text, {"font_id": font_id, "size": 4}) == case["metrics"]
        assert doc.to_bytes() == before
        snapshot = copy.deepcopy(commands)
        ids = doc.draw_canvas(0, TEXT_REQUESTS["boundary"], commands)
        assert commands == snapshot
        assert len(ids) == 16
        assert [item["id"] for item in doc.page_text_objects(0)] == ids
        for dpi in (25.4, 101.6):
            assert doc.page_png(0, dpi) == java.page_png(0, dpi)
        with Document.from_bytes(doc.to_bytes()) as reopened:
            assert reopened.validate_references() == []
            assert reopened.page_png(0, 101.6) == doc.page_png(0, 101.6)


def test_canvas_text_defaults_empty_input_invalid_settings_and_closed_document():
    with Document.new() as doc:
        doc.add_blank_page(40, 30)
        before = doc.to_bytes()
        assert doc.measure_canvas_text(0, "") == {"width": 0, "first_offset": [0, 0], "offsets": []}
        assert doc.measure_canvas_text(0, "x")["width"] > 0
        assert doc.draw_canvas(0, (0, 0, 40, 30), [{"type": "fill_text", "text": "", "point": [0, 0]}]) == []
        assert doc.to_bytes() == before
        for font in [{"size": 0}, {"size": float("nan")}, {"weight": 450}, {"align": "bad"},
                     {"read_direction": 1}, {"extra": 1}, {"font_id": "99999"}]:
            with pytest.raises((OfdError, ValueError)):
                doc.measure_canvas_text(0, "text", font)
            with pytest.raises((OfdError, ValueError)):
                doc.draw_canvas(0, (0, 0, 40, 30), [
                    {"type": "fill_text", "text": "first", "point": [1, 5]},
                    {"type": "set_font", "font": font},
                    {"type": "fill_text", "text": "last", "point": [1, 10]},
                ])
            assert doc.to_bytes() == before
        with pytest.raises(IndexError):
            doc.measure_canvas_text(-1, "text")
        with pytest.raises(TypeError):
            doc.measure_canvas_text(0, 1)
        with pytest.raises(TypeError):
            doc.measure_canvas_text(0, "text", [])
    with pytest.raises(OfdError):
        doc.measure_canvas_text(0, "text")


def test_canvas_text_area_matches_all_java_reference_cases():
    cases = json.loads((TEXT_ROOT / "areas.json").read_text())["cases"]
    assert len(cases) == 960
    with Document.new() as doc:
        doc.add_blank_page(40, 30)
        fonts = {name: doc.add_embedded_font(name, (TEXT_ROOT.parent / name).read_bytes())
                 for name in {case["font"] for case in cases}}
        before = doc.to_bytes()
        for case in cases:
            font = {"font_id": fonts[case["font"]], "size": 4,
                    **{key: case[key] for key in ("letter_spacing", "read_direction", "char_direction")}}
            snapshot = copy.deepcopy(font)
            area = doc.measure_canvas_text_area(0, case["text"], font)
            assert font == snapshot
            tolerance = 0.0001 if case["font"] == "test-name-cff.otf" else 0
            assert area["letter_spacing"] == case["effective_letter_spacing"]
            count = max(len(area["char_areas"]), 1)
            for key in ("width", "height"):
                assert abs(area[key] - case[key]) <= tolerance * count, case
            assert len(area["char_areas"]) == len(case["char_areas"])
            for actual, expected in zip(area["char_areas"], case["char_areas"]):
                assert all(abs(a - b) <= tolerance for a, b in zip(actual, expected)), case
        assert doc.to_bytes() == before
        with Document.from_bytes(before) as reopened:
            assert reopened.measure_canvas_text_area(0, "A😀", {"font_id": fonts["dejavu-sans.ttf"]}) == doc.measure_canvas_text_area(0, "A😀", {"font_id": fonts["dejavu-sans.ttf"]})


def test_canvas_text_area_validates_inputs_and_closed_documents():
    with Document.new() as doc:
        doc.add_blank_page(40, 30)
        before = doc.to_bytes()
        assert doc.measure_canvas_text_area(0, "") == {"width": 0, "height": 0, "letter_spacing": 0, "char_areas": []}
        assert len(doc.measure_canvas_text_area(0, "A😀")["char_areas"]) == 2
        for font in [{"size": 0}, {"extra": 1}, {"letter_spacing": float("nan")}, {"letter_spacing": 1e308}, {"font_id": "999999"}]:
            with pytest.raises((OfdError, ValueError)):
                doc.measure_canvas_text_area(0, "ABC", font)
        for page in [-1, 1]:
            with pytest.raises((IndexError, OfdError)):
                doc.measure_canvas_text_area(page, "x")
        with pytest.raises(TypeError):
            doc.measure_canvas_text_area(0, 1)
        with pytest.raises(TypeError):
            doc.measure_canvas_text_area(0, "x", [])
        assert doc.to_bytes() == before
    with pytest.raises(OfdError):
        doc.measure_canvas_text_area(0, "")
