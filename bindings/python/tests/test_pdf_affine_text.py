"""Exercise affine PDF text pens through the Python C ABI wrapper."""
import json
from pathlib import Path

import pytest
from ofdrw_rs import Document

ROOT = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/pdf-text-affine"
CASES = json.loads((ROOT / "reference.json").read_text(encoding="utf-8"))["cases"]


@pytest.mark.parametrize("case", CASES, ids=lambda case: case["name"])
def test_affine_text_preserves_vectors_semantics_and_ofd_round_trip(case):
    with Document.from_pdf_bytes((ROOT / case["file"]).read_bytes()) as document:
        assert "".join(document.page_text(0)) == case["text"]
        assert document.page_images(0) == []
        assert document.page_paths(0)
        png = document.page_png(0, 144)
        with Document.from_bytes(document.to_bytes()) as reopened:
            assert reopened.page_text(0) == document.page_text(0)
            assert reopened.page_png(0, 144) == png
            assert reopened.validate_references() == []
            with Document.from_pdf_bytes(reopened.to_pdf()) as exported:
                assert "".join(exported.page_text(0)) == case["text"]
                assert exported.validate_references() == []
