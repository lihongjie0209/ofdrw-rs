from pathlib import Path

import pytest
from ofdrw_rs import Document


@pytest.mark.parametrize("name", ["matrix", "widget", "link", "image"])
def test_pdf_normal_appearance_survives_ofd_roundtrip(name):
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures" / f"annotation-ap-{name}.pdf"
    with Document.from_pdf_bytes(fixture.read_bytes()) as doc:
        assert doc.page_count == 1
        annotations = doc.annotations()
        assert len(annotations) == 1
        assert annotations[0]["remark"] == "custom appearance"
        if name == "link":
            assert annotations[0]["actions"][0]["attributes"]["URI"] == "https://example.com/appearance"
        png = doc.page_png(0, 144)
        assert doc.validate_references() == []
        data = doc.to_bytes()
    with Document.from_bytes(data) as reopened:
        assert reopened.page_png(0, 144) == png
        assert reopened.to_pdf().startswith(b"%PDF-")
        assert reopened.validate_references() == []
