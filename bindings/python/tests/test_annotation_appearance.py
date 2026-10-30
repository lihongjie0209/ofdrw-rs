from pathlib import Path

import pytest
from ofdrw_rs import Document


@pytest.mark.parametrize("name", ["matrix", "widget", "link", "image", "nonprint", "no-view"])
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
        pdf = reopened.to_pdf()
        with Document.from_pdf_bytes(pdf) as converted:
            annotations = converted.annotations()
            assert len(annotations) == 1
            assert annotations[0]["remark"] == "custom appearance"
            if name == "nonprint":
                assert annotations[0]["attributes"]["Print"] == "false"
                assert "#ff0000" in converted.page_svg(0)
            elif name == "no-view":
                assert annotations[0]["attributes"]["Visible"] == "false"
                assert annotations[0]["attributes"].get("Print", "true") == "true"
                assert "#ff0000" not in converted.page_svg(0)
        assert reopened.validate_references() == []
