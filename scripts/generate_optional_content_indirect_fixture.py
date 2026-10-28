"""Generate a small PDF with indirect optional-content configuration values."""

from pathlib import Path


TARGET = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures/optional-content-indirect.pdf"
content = (
    b"BT /F1 12 Tf 10 20 Td /OC /Hidden BDC (A) Tj EMC ET "
    b"BT /F1 12 Tf 10 40 Td /OC /Shown BDC (B) Tj EMC ET"
)
objects = [
    b"<< /Type /Catalog /Pages 2 0 R /OCProperties 7 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] "
    b"/Resources << /Font << /F1 4 0 R >> "
    b"/Properties << /Hidden 5 0 R /Shown 6 0 R >> >> /Contents 10 0 R >>",
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    b"<< /Type /OCG /Name (Hidden) /Intent 11 0 R >>",
    b"<< /Type /OCG /Name (Shown) /Intent 11 0 R >>",
    b"<< /OCGs 8 0 R /D 9 0 R >>",
    b"[5 0 R 6 0 R]",
    b"<< /BaseState 12 0 R /ON [6 0 R] /Intent 11 0 R >>",
    b"<< /Length " + str(len(content)).encode() + b" >>\nstream\n" + content + b"\nendstream",
    b"/View",
    b"/OFF",
]
pdf = bytearray(b"%PDF-1.5\n%\xe2\xe3\xcf\xd3\n")
offsets = [0]
for number, obj in enumerate(objects, 1):
    offsets.append(len(pdf))
    pdf.extend(f"{number} 0 obj\n".encode() + obj + b"\nendobj\n")
xref = len(pdf)
pdf.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
for offset in offsets[1:]:
    pdf.extend(f"{offset:010d} 00000 n \n".encode())
pdf.extend(f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
TARGET.write_bytes(pdf)
