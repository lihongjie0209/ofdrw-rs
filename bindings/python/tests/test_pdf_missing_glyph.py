"""Verify the native export path without any installed CJK font."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
from xml.etree import ElementTree

import pytest


@pytest.mark.skipif(sys.platform != "linux", reason="fontconfig isolation requires Linux")
@pytest.mark.parametrize("name,expected", [
    ("outline-tounicode", "中文"),
    ("type0-embedded-collection-unicode", "中文"),
    ("type0-supplementary-collection-unicode", "\U00031350"),
])
def test_exports_missing_glyph_unicode(tmp_path, name, expected):
    reader = shutil.which("pdftotext")
    if reader is None:
        pytest.skip("Poppler is required for independent ActualText extraction")
    root = Path(__file__).resolve().parents[3]
    fixtures = root / "crates" / "ofdrw" / "tests" / "fixtures"
    fonts = tmp_path / "fonts"
    fonts.mkdir()
    shutil.copyfile(fixtures / "dejavu-sans.ttf", fonts / "latin.ttf")
    config = tmp_path / "fonts.conf"
    tree = ElementTree.Element("fontconfig")
    ElementTree.SubElement(tree, "dir").text = str(fonts)
    ElementTree.ElementTree(tree).write(config)
    output = tmp_path / "export.pdf"
    program = """
from pathlib import Path
import sys
from ofdrw_rs import Document
with Document.from_pdf_bytes(Path(sys.argv[1]).read_bytes()) as source:
    data = source.to_bytes()
with Document.from_bytes(data) as reopened:
    exported = reopened.to_pdf()
for _ in range(2):
    with Document.from_pdf_bytes(exported) as imported:
        assert "".join(imported.page_text(0)) == sys.argv[3]
        assert imported.validate_references() == []
        exported = imported.to_pdf()
Path(sys.argv[2]).write_bytes(exported)
"""
    subprocess.run(
        [sys.executable, "-c", program, str(fixtures / f"{name}.pdf"), str(output), expected],
        env=dict(os.environ, FONTCONFIG_FILE=str(config)), check=True,
        capture_output=True, text=True,
    )
    result = subprocess.run([reader, "-raw", "-enc", "UTF-8", str(output), "-"],
                            check=True, capture_output=True, text=True)
    assert "".join(result.stdout.split()) == expected


def test_imports_actual_text_with_native_cff_outlines():
    from ofdrw_rs import Document
    fixture = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/actual-text-cff.pdf"
    with Document.from_pdf_bytes(fixture.read_bytes()) as document:
        assert "".join(document.page_text(0)) == "中文"
        assert document.validate_references() == []
        data = document.to_bytes()
    with Document.from_bytes(data) as reopened:
        assert "".join(reopened.page_text(0)) == "中文"
        exported = reopened.to_pdf()
    with Document.from_pdf_bytes(exported) as imported:
        assert "".join(imported.page_text(0)) == "中文"
        assert imported.validate_references() == []
