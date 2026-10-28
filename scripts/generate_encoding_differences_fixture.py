"""Generate glyph-name text and an embedded TrueType font without ToUnicode.

Reference (Ghostscript 10.02.1):
gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m -r72 -dTextAlphaBits=4
   -sOutputFile=encoding-differences-reference.png encoding-differences-truetype.pdf
"""
from pathlib import Path
import zlib

from generate_nested_stitching_fixture import pdf_bytes, stream


def differences_pdf(base_font=b"Helvetica"):
    return pdf_bytes([
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] "
        b"/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        stream(b"", b"BT /F1 12 Tf 10 20 Td [<41> 100 <42>] TJ ET"),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /" + base_font + b" /Encoding 6 0 R "
        b"/FirstChar 65 /LastChar 66 /Widths [600 500] >>",
        b"<< /Type /Encoding /BaseEncoding /WinAnsiEncoding "
        b"/Differences [65 /uni4E2D /uni6587] >>",
    ])


def embedded_differences_pdf():
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    font = (fixtures / "dejavu-sans.ttf").read_bytes()
    return pdf_bytes([
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] "
        b"/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        stream(b"", b"BT /F1 24 Tf 10 36 Td (AB) Tj ET"),
        b"<< /Type /Font /Subtype /TrueType /BaseFont /DejaVuSans /Encoding 7 0 R "
        b"/FontDescriptor 6 0 R /FirstChar 65 /LastChar 66 /Widths [600 500] >>",
        b"<< /Type /FontDescriptor /FontName /DejaVuSans /Flags 32 "
        b"/FontBBox [-1021 -463 1794 1232] /ItalicAngle 0 /Ascent 928 /Descent -236 "
        b"/CapHeight 928 /StemV 80 /FontFile2 8 0 R >>",
        b"<< /Type /Encoding /BaseEncoding /WinAnsiEncoding /Differences [65 /B /A] >>",
        stream(f"/Length1 {len(font)} /Filter /FlateDecode".encode(), zlib.compress(font)),
    ])


if __name__ == "__main__":
    target = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures/encoding-differences.pdf"
    target.write_bytes(differences_pdf())
    target.with_name("custom-type1-differences.pdf").write_bytes(differences_pdf(b"ABCDEF+CustomFont"))
    target.with_name("encoding-differences-truetype.pdf").write_bytes(embedded_differences_pdf())
