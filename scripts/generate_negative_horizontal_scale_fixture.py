"""Generate a PDF with mirrored Type1 text for binding integration tests."""

from pathlib import Path


CONTENT = b"BT /F1 10 Tf 20 20 Td -50 Tz (AB) Tj (C) Tj 100 Tz (D) Tj ET"
OBJECTS = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] /Contents 4 0 R "
    b"/Resources << /Font << /F1 5 0 R >> >> >>",
    b"<< /Length %d >>\nstream\n" % len(CONTENT) + CONTENT + b"\nendstream",
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica "
    b"/Encoding /WinAnsiEncoding /FirstChar 65 /LastChar 68 "
    b"/Widths [600 600 600 600] >>",
]


def main() -> None:
    pdf = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for number, body in enumerate(OBJECTS, 1):
        offsets.append(len(pdf))
        pdf.extend(f"{number} 0 obj\n".encode())
        pdf.extend(body)
        pdf.extend(b"\nendobj\n")
    xref = len(pdf)
    pdf.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        pdf.extend(f"{offset:010} 00000 n \n".encode())
    pdf.extend(
        f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    )
    path = (
        Path(__file__).resolve().parents[1]
        / "crates/ofdrw/tests/fixtures/negative-horizontal-scale.pdf"
    )
    path.write_bytes(pdf)


if __name__ == "__main__":
    main()
