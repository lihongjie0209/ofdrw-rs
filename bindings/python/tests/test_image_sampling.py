from pathlib import Path

import pytest

from ofdrw_rs import Document, OfdError


def test_image_sampling_set_reset_and_roundtrip():
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/image-mask.ofd"
    with Document.open(fixture) as doc:
        baseline = doc.page_svg(0)
        for value, hint in [(False, 'optimizeSpeed'), (True, 'optimizeQuality')]:
            doc.set_page_image_interpolate(0, "5", value)
            assert f'image-rendering="{hint}"' in doc.page_svg(0)
        doc.set_page_image_interpolate(0, "5")
        assert doc.page_svg(0) == baseline
        before = doc.to_bytes()
        with pytest.raises(TypeError):
            doc.set_page_image_interpolate(0, "5", 1)
        with pytest.raises(IndexError):
            doc.set_page_image_interpolate(-1, "5", False)
        with pytest.raises(OfdError):
            doc.set_page_image_interpolate(0, "missing", False)
        assert doc.to_bytes() == before
        doc.set_page_image_interpolate(0, "5", False)
        data = doc.to_bytes()
    with Document.from_bytes(data) as reopened:
        assert 'image-rendering="optimizeSpeed"' in reopened.page_svg(0)
        assert reopened.validate_references() == []
    with pytest.raises(OfdError):
        doc.set_page_image_interpolate(0, "5", False)
