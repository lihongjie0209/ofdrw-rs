from pathlib import Path

import pytest
from ofdrw_rs import Document


@pytest.mark.parametrize("name", [
    "truetype",
    "affine",
    "reflected",
    "cff",
    "cff-matrix",
    "cid-cff-matrix",
    "truetype-stroke",
    "cff-stroke",
    "truetype-dashed-stroke",
    "cff-dashed-stroke",
    "cff-opentype",
    "truetype-matrix-stroke",
    "cff-matrix-stroke",
    "cff-pattern",
    "cff-pattern-matrix-stroke",
])

def test_empty_actual_text_keeps_paint_without_text_through_ofd_and_pdf(name):
    path = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / f"actual-text-empty-{name}.pdf"
    with Document.from_pdf_bytes(path.read_bytes()) as doc:
        assert doc.page_text(0) == []
        png = doc.page_png(0, 144)
        data = doc.to_bytes()
    with Document.from_bytes(data) as reopened:
        assert reopened.page_png(0, 144) == png
        assert reopened.validate_references() == []
        with Document.from_pdf_bytes(reopened.to_pdf()) as exported:
            assert exported.page_text(0) == []
            assert exported.validate_references() == []
