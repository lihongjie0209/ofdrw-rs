from pathlib import Path

import pytest
from ofdrw_rs import Document, OfdError


@pytest.mark.parametrize("styled,shape", [(False, False), (True, False), (False, True), (True, True)])
def test_flow_continues_on_an_existing_page_and_preserves_the_source_on_error(styled, shape):
    font = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/dejavu-serif.ttf").read_bytes()
    with Document.new() as doc:
        doc.add_text_page("seed", 20, 20)
        font_id = doc.add_embedded_font("DejaVu Serif", font)
        options = dict(page_size=(20, 20), margins=(2, 2, 2, 2), font_size=2,
                       line_height=3, font_id=font_id, shape=shape,
                       start={"page_index": 0, "y": 10})
        content = [{"text": "A\nB\nC\nD"}] if styled else "A\nB\nC\nD"
        write = doc.add_flowed_spans if styled else doc.add_flowed_text
        assert write(0, content, **options) == [0, 1]
        assert doc.page_text(0) == ["seed", "A", "B"]
        assert doc.page_text(1) == ["C", "D"]
        assert doc.page_text_objects(0)[1]["boundary"][:2] == [2, 10]
        assert doc.page_text_objects(1)[0]["boundary"][:2] == [2, 2]
        before = doc.to_bytes()
        options["start"] = {"page_index": 0, "y": 19}
        with pytest.raises(OfdError):
            write(0, content, **options)
        assert doc.to_bytes() == before
        with Document.from_bytes(before) as reopened:
            assert reopened.page_text(0) == ["seed", "A", "B"]
            assert reopened.validate_references() == []
            assert reopened.page_png(0, 72) == doc.page_png(0, 72)
