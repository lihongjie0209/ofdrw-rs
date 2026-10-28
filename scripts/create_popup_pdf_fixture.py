"""Regenerate small PDF annotation fixtures used by binding integration tests."""

from pathlib import Path


FIXTURES = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
COMMON = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
]
LINKED = COMMON + [
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] "
    b"/Contents 4 0 R /Annots [5 0 R 6 0 R] >>",
    b"<< /Length 0 >>\nstream\n\nendstream",
    b"<< /Type /Annot /Subtype /Text /Rect [10 20 30 40] "
    b"/Popup 6 0 R /Name /Comment /Open false /Contents (review) >>",
    b"<< /Type /Annot /Subtype /Popup /Parent 5 0 R "
    b"/Rect [35 15 75 55] /Open true >>",
]
STANDALONE = COMMON + [
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] "
    b"/Contents 4 0 R /Annots [5 0 R] >>",
    b"<< /Length 0 >>\nstream\n\nendstream",
    b"<< /Type /Annot /Subtype /Popup /Rect [35 15 75 55] "
    b"/Open true /Contents (standalone review) /T (Reviewer) /NM (popup-5) >>",
]
INK = COMMON + [
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] "
    b"/Contents 4 0 R /Annots [5 0 R] >>",
    b"<< /Length 0 >>\nstream\n\nendstream",
    b"<< /Type /Annot /Subtype /Ink /Rect [10 10 60 60] "
    b"/InkList [[20 20 30 40 45 30] [25 50 40 50]] "
    b"/BS << /W 2 /S /D /D [3 1] >> /C [1 0 0] /CA 0.5 "
    b"/Contents (two pen strokes) /T (Reviewer) /NM (ink-5) >>",
]
POLYGON = COMMON + [
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] "
    b"/Contents 4 0 R /Annots [5 0 R] >>",
    b"<< /Length 0 >>\nstream\n\nendstream",
    b"<< /Type /Annot /Subtype /Polygon /Rect [10 10 60 60] "
    b"/Vertices [20 20 40 20 45 40 20 45] "
    b"/BS << /W 2 /S /D /D [3 1] >> /C [0 0 1] /IC [1 0 0] /CA 0.5 "
    b"/IT /PolygonDimension /Contents (red polygon) /T (Reviewer) /NM (polygon-5) >>",
]
POLYLINE = COMMON + [
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] "
    b"/Contents 4 0 R /Annots [5 0 R] >>",
    b"<< /Length 0 >>\nstream\n\nendstream",
    b"<< /Type /Annot /Subtype /PolyLine /Rect [10 10 60 60] "
    b"/Vertices [20 20 30 40 45 30] /LE [/ClosedArrow /Square] "
    b"/BS << /W 2 /S /S >> /C [0 0 1] /IC [0 1 0] /CA 0.75 "
    b"/IT /PolyLineDimension /Contents (blue polyline) /T (Reviewer) /NM (polyline-5) >>",
]


def write_pdf(target: Path, objects: list[bytes]) -> None:
    pdf = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for number, body in enumerate(objects, 1):
        offsets.append(len(pdf))
        pdf.extend(f"{number} 0 obj\n".encode("ascii"))
        pdf.extend(body)
        pdf.extend(b"\nendobj\n")
    xref = len(pdf)
    pdf.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode("ascii"))
    for offset in offsets[1:]:
        pdf.extend(f"{offset:010d} 00000 n \n".encode("ascii"))
    pdf.extend(
        f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\n"
        f"startxref\n{xref}\n%%EOF\n".encode("ascii")
    )
    target.write_bytes(pdf)


def main() -> None:
    write_pdf(FIXTURES / "popup.pdf", LINKED)
    write_pdf(FIXTURES / "standalone-popup.pdf", STANDALONE)
    write_pdf(FIXTURES / "ink.pdf", INK)
    write_pdf(FIXTURES / "polygon.pdf", POLYGON)
    write_pdf(FIXTURES / "polyline.pdf", POLYLINE)


if __name__ == "__main__":
    main()
