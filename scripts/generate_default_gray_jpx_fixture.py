"""Generate a PDF exercising DefaultGray on vector paint and JPEG2000."""

from pathlib import Path


def stream(dictionary: bytes, data: bytes) -> bytes:
    return (
        b"<< "
        + dictionary
        + b" /Length "
        + str(len(data)).encode("ascii")
        + b" >>\nstream\n"
        + data
        + b"\nendstream"
    )


fixtures = Path(__file__).resolve().parents[1] / "crates" / "ofdrw" / "tests" / "fixtures"
jp2 = (fixtures / "gray16-pair-32x32.jp2").read_bytes()
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] "
    b"/Resources << /ColorSpace << /DefaultGray [/CalGray "
    b"<< /WhitePoint [0.9505 1 1.089] /Gamma 1 >>] >> "
    b"/XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>",
    stream(b"", b"0.5 g 5 5 10 10 re f q 32 0 0 32 20 20 cm /Im1 Do Q"),
    stream(
        b"/Type /XObject /Subtype /Image /Width 32 /Height 32 "
        b"/ColorSpace /DeviceGray /BitsPerComponent 16 /Filter /JPXDecode",
        jp2,
    ),
]

pdf = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
offsets = [0]
for index, obj in enumerate(objects, 1):
    offsets.append(len(pdf))
    pdf.extend(f"{index} 0 obj\n".encode("ascii") + obj + b"\nendobj\n")
xref = len(pdf)
pdf.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode("ascii"))
for offset in offsets[1:]:
    pdf.extend(f"{offset:010} 00000 n \n".encode("ascii"))
pdf.extend(
    f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\n"
    f"startxref\n{xref}\n%%EOF\n".encode("ascii")
)
(fixtures / "default-gray-jpx.pdf").write_bytes(pdf)
