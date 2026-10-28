"""Generate small PDFs with named and indirectly declared font styles."""
from pathlib import Path
import re

from generate_nested_stitching_fixture import pdf_bytes


if __name__ == "__main__":
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    objects = re.findall(rb"\d+ 0 obj\n(.*?)\nendobj", (fixtures / "type1-tounicode.pdf").read_bytes(), re.S)
    assert len(objects) == 6
    standard = objects.copy()
    standard[4] = standard[4].replace(b"/Helvetica", b"/Helvetica-BoldOblique")
    (fixtures / "font-style-standard.pdf").write_bytes(pdf_bytes(standard))
    descriptor = objects.copy()
    descriptor[4] = descriptor[4].replace(b"/Helvetica", b"/DemoText").replace(b">>", b"/FontDescriptor 7 0 R >>")
    descriptor.extend([
        b"<< /Type /FontDescriptor /FontWeight 8 0 R /ItalicAngle 9 0 R /Flags 10 0 R >>",
        b"600", b"-13", b"64",
    ])
    (fixtures / "font-style-descriptor.pdf").write_bytes(pdf_bytes(descriptor))
