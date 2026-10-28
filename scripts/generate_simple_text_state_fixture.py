"""Generate equivalent text with indirect simple widths and a default BT origin."""
from pathlib import Path
import re

from generate_nested_stitching_fixture import pdf_bytes, stream


if __name__ == "__main__":
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    objects = re.findall(rb"\d+ 0 obj\n(.*?)\nendobj", (fixtures / "type1-tounicode.pdf").read_bytes(), re.S)
    assert len(objects) == 6
    indirect = objects.copy()
    indirect[4] = b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding " \
        b"/ToUnicode 6 0 R /FirstChar 7 0 R /LastChar 8 0 R /Widths 9 0 R >>"
    indirect.extend([b"65", b"66", b"[10 0 R 11 0 R]", b"600", b"500"])
    (fixtures / "simple-indirect-widths.pdf").write_bytes(pdf_bytes(indirect))
    origin = objects.copy()
    origin[3] = stream(b"", b"q 1 0 0 1 10 20 cm BT /F1 12 Tf [<41> 100 <42>] TJ ET Q")
    (fixtures / "text-default-origin.pdf").write_bytes(pdf_bytes(origin))
