"""Build the PDF fixture that applies a color-key mask to a JP2 image."""

from pathlib import Path


FIXTURES = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
image = (FIXTURES / "red-blue-32x32.jp2").read_bytes()
content = b"q 32 0 0 32 0 0 cm /Im1 Do Q\n"

objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] "
    b"/Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>",
    b"<< /Length %d >>\nstream\n" % len(content) + content + b"endstream",
    b"<< /Type /XObject /Subtype /Image /Width 32 /Height 32 "
    b"/ColorSpace /DeviceRGB /Filter /JPXDecode "
    b"/Mask [250 255 0 5 0 5] /Length %d >>\nstream\n" % len(image)
    + image
    + b"\nendstream",
]

pdf = bytearray(b"%PDF-1.5\n%\xe2\xe3\xcf\xd3\n")
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
(FIXTURES / "red-blue-jpx-mask.pdf").write_bytes(pdf)
