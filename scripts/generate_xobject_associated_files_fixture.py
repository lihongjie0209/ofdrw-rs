"""Generate the nested XObject associated-file import fixture."""

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


objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] "
    b"/Resources << /XObject << /Fm1 5 0 R >> >> /Contents 4 0 R >>",
    stream(b"", b"q /Fm1 Do Q"),
    stream(
        b"/Type /XObject /Subtype /Form /BBox [0 0 20 20] "
        b"/Resources << /XObject << /Im1 6 0 R >> >> /AF [7 0 R]",
        b"q 20 0 0 20 0 0 cm /Im1 Do Q",
    ),
    stream(
        b"/Type /XObject /Subtype /Image /Width 1 /Height 1 "
        b"/ColorSpace /DeviceRGB /BitsPerComponent 8 /AF [8 0 R]",
        bytes([255, 0, 0]),
    ),
    b"<< /Type /Filespec /F (form-source.csv) /EF << /F 9 0 R >> >>",
    b"<< /Type /Filespec /F (image-source.rgb) /EF << /F 10 0 R >> >>",
    stream(b"/Type /EmbeddedFile", b"a,b\n"),
    stream(b"/Type /EmbeddedFile", bytes([255, 0, 0])),
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
Path(__file__).resolve().parents[1].joinpath(
    "crates/ofdrw/tests/fixtures/xobject-associated-files.pdf"
).write_bytes(pdf)
