"""Generate a PDF with associated files on five page resource categories."""

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


files = [
    (b"graphics.txt", b"graphics"),
    (b"profile.txt", b"profile"),
    (b"pattern.txt", b"pattern"),
    (b"shading.txt", b"shading"),
    (b"properties.txt", b"properties"),
    (b"resources.txt", b"resources"),
    (b"page-tree.txt", b"page tree"),
    (b"pattern-list.txt", b"pattern list"),
]
file_spec_start = 10
embedded_start = file_spec_start + len(files)
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 /AF [16 0 R] >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] "
    b"/Resources << /AF [15 0 R] /ExtGState << /GS1 5 0 R >> "
    b"/ColorSpace << /CS1 [/ICCBased 6 0 R] >> "
    b"/Pattern << /AF [17 0 R] /P1 7 0 R >> /Shading << /S1 8 0 R >> "
    b"/Properties << /MC1 9 0 R /MC2 5 0 R >> >> "
    b"/Contents 4 0 R >>",
    stream(b"", b"0 0 10 10 re f"),
]
for index in range(5):
    associated = b"/AF [" + str(file_spec_start + index).encode("ascii") + b" 0 R]"
    if index == 1:
        objects.append(stream(b"/N 3 " + associated, b""))
    else:
        objects.append(b"<< " + associated + b" >>")
for index, (name, _) in enumerate(files):
    embedded_id = embedded_start + index
    objects.append(
        b"<< /Type /Filespec /F ("
        + name
        + b") /EF << /F "
        + str(embedded_id).encode("ascii")
        + b" 0 R >> >>"
    )
for _, payload in files:
    objects.append(stream(b"/Type /EmbeddedFile", payload))

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
    "crates/ofdrw/tests/fixtures/page-resource-associated-files.pdf"
).write_bytes(pdf)
