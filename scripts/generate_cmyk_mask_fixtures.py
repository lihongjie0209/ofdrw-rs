"""Embed the two existing four-channel JPEGs in masked PDF image XObjects."""

from pathlib import Path


FIXTURES = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
CONTENT = b"q 72 0 0 36 0 0 cm /Im1 Do Q\n"


def write_fixture(source: str, target: str, invert_decode: bool = False) -> None:
    jpeg = (FIXTURES / source).read_bytes()
    decode = b"/Decode [1 0 1 0 1 0 1 0] " if invert_decode else b""
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 36] "
        b"/Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>",
        b"<< /Length %d >>\nstream\n" % len(CONTENT) + CONTENT + b"endstream",
        b"<< /Type /XObject /Subtype /Image /Width 2 /Height 1 "
        b"/ColorSpace /DeviceCMYK /BitsPerComponent 8 /Filter /DCTDecode "
        + decode
        + b"/Mask [220 255 0 35 0 35 220 255] /Length %d >>\nstream\n" % len(jpeg)
        + jpeg
        + b"\nendstream",
    ]
    pdf = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for number, body in enumerate(objects, 1):
        offsets.append(len(pdf))
        pdf += f"{number} 0 obj\n".encode() + body + b"\nendobj\n"
    xref = len(pdf)
    pdf += b"xref\n0 6\n0000000000 65535 f \n"
    for offset in offsets[1:]:
        pdf += f"{offset:010d} 00000 n \n".encode()
    pdf += b"trailer\n<< /Size 6 /Root 1 0 R >>\n"
    pdf += f"startxref\n{xref}\n%%EOF\n".encode()
    (FIXTURES / target).write_bytes(pdf)


if __name__ == "__main__":
    write_fixture("cmyk.jpg", "cmyk-mask.pdf")
    write_fixture("ycck.jpg", "ycck-mask.pdf")
    write_fixture("cmyk.jpg", "cmyk-decode-mask.pdf", True)
    write_fixture("ycck.jpg", "ycck-decode-mask.pdf", True)
