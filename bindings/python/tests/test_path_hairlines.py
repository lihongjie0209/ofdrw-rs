import pytest

from ofdrw_rs import Document, OfdError


@pytest.mark.parametrize("writer", ["add_path_object", "add_composite_path_object", "add_path_annotation"])
def test_path_writers_preserve_zero_width_and_roundtrip_rendering(writer):
    with Document.new() as doc:
        doc.add_blank_page(20, 20)
        object_id = getattr(doc, writer)(0, "M 2 5 L 18 5", (0, 0, 20, 20), line_width=0)
        if writer == "add_path_object":
            doc.set_page_object_ctm(0, str(object_id), [1, 0, 0, 3, 0, 0])
            assert doc.page_object_stroke_styles(0)[0]["style"]["line_width"] == 0
        svg = doc.page_svg(0)
        assert 'vector-effect="non-scaling-stroke"' in svg
        assert 'ofd-device-hairline-' in svg
        with Document.from_bytes(doc.to_bytes()) as reopened:
            assert reopened.validate_references() == []
            for dpi in [25.4, 101.6, 300]:
                assert reopened.page_png(0, dpi) == doc.page_png(0, dpi)


def test_zero_width_updates_support_dashes_and_negative_widths_roll_back():
    with Document.new() as doc:
        doc.add_blank_page(20, 20)
        object_id = str(doc.add_path_object(0, "M 2 5 L 18 5", (0, 0, 20, 20)))
        doc.set_page_path_stroke_style(0, object_id, {"line_width": 0, "dash_pattern": [2, 2]})
        assert doc.page_object_stroke_styles(0)[0]["style"]["line_width"] == 0
        assert 'ofd-device-hairline-' in doc.page_svg(0)
        before = doc.to_bytes()
        with pytest.raises(OfdError):
            doc.set_page_path_stroke_style(0, object_id, {"line_width": -1})
        with pytest.raises(OfdError):
            doc.add_path_object(0, "M 2 5 L 18 5", (0, 0, 20, 20), line_width=-1)
        assert doc.to_bytes() == before
