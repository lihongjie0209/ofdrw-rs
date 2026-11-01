from pathlib import Path

import pytest
from ofdrw_rs import Document, OfdError


@pytest.mark.parametrize("merged,y,pages,first,last", [
    (False, 10, [0, 1], ["seed", "H", "A"], ["H", "B"]),
    (False, 12, [1], ["seed"], ["H", "A", "B"]),
    (True, 4, [0, 1], ["seed", "H", "M"], ["H", "B"]),
    (True, 8, [1], ["seed"], ["H", "M", "B"]),
])
def test_table_continues_without_orphan_headers_or_split_merged_rows(merged, y, pages, first, last):
    font = (Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/dejavu-serif.ttf").read_bytes()
    with Document.new() as doc:
        doc.add_text_page("seed", 20, 20)
        font_id = doc.add_embedded_font("DejaVu Serif", font)
        options = dict(page_size=(20, 20), margins=(2, 2, 2, 2), row_height=4,
                       font_size=2, line_height=3, padding=0.5, font_id=font_id,
                       repeat_header=True, start={"page_index": 0, "y": y})
        cells = [{"row": 0, "column": 0, "text": "H"},
                 {"row": 1, "column": 0, "row_span": 2, "text": "M"},
                 {"row": 3, "column": 0, "text": "B"}]

        def write():
            if merged:
                return doc.add_table_cells(0, 4, cells, [16], **options)
            return doc.add_table(0, [["H"], ["A"], ["B"]], [16], **options)

        assert write() == pages
        assert doc.page_text(0) == first
        assert doc.page_text(1) == last
        before = doc.to_bytes()
        options["start"]["y"] = 19
        with pytest.raises(OfdError):
            write()
        assert doc.to_bytes() == before
        with Document.from_bytes(before) as reopened:
            assert reopened.page_text(0) == first
            assert reopened.page_text(1) == last
            assert reopened.validate_references() == []
            assert reopened.page_png(1, 72) == doc.page_png(1, 72)
