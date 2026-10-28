"""Generate shared Standard 14 font cases with omitted Widths."""
from pathlib import Path
import re

from generate_nested_stitching_fixture import pdf_bytes, stream


def symbolic_pdf(font, codes):
    return pdf_bytes([
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] "
        b"/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        stream(b"", b"BT /F1 12 Tf 10 20 Td <" + codes.hex().encode() + b"> Tj ET"),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /" + font.encode() + b" >>",
    ])


if __name__ == "__main__":
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    objects = re.findall(rb"\d+ 0 obj\n(.*?)\nendobj", (fixtures / "type1-tounicode.pdf").read_bytes(), re.S)
    assert len(objects) == 6
    objects[4] = b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding /ToUnicode 6 0 R >>"
    (fixtures / "standard14-tounicode.pdf").write_bytes(pdf_bytes(objects))
    (fixtures / "standard14-symbol.pdf").write_bytes(symbolic_pdf("Symbol", b"AB"))
    (fixtures / "standard14-dingbats.pdf").write_bytes(symbolic_pdf("ZapfDingbats", b'!"'))
