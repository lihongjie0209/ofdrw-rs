"""Generate small PDF link fixtures used by binding integration tests."""

from pathlib import Path


def write_pdf(name: str, objects: list[bytes]) -> None:
    pdf = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for number, body in enumerate(objects, 1):
        offsets.append(len(pdf))
        pdf.extend(f"{number} 0 obj\n".encode())
        pdf.extend(body)
        pdf.extend(b"\nendobj\n")
    xref = len(pdf)
    pdf.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        pdf.extend(f"{offset:010} 00000 n \n".encode())
    pdf.extend(f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    destination = Path(__file__).resolve().parents[1] / f"crates/ofdrw/tests/fixtures/{name}"
    destination.write_bytes(pdf)


def main() -> None:
    named = [
        b"<< /Type /Catalog /Pages 2 0 R /Dests << /legacy [4 0 R /Fit] >> /Names << /Dests 9 0 R >> >>",
        b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [10 20 82 92] /Contents 7 0 R /Annots [5 0 R 6 0 R] >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] /Contents 8 0 R >>",
        b"<< /Type /Annot /Subtype /Link /Rect [20 30 40 50] /F 4 /Border [0 0 0] /A << /S /GoTo /D (section) >> >>",
        b"<< /Type /Annot /Subtype /Link /Rect [45 30 65 50] /F 4 /Border [0 0 0] /Dest /legacy >>",
        b"<< /Length 0 >>\nstream\n\nendstream",
        b"<< /Length 0 >>\nstream\n\nendstream",
        b"<< /Kids [10 0 R] >>",
        b"<< /Limits [(section) (section)] /Names [(section) 11 0 R] >>",
        b"<< /D [4 0 R /XYZ 10 50 1.25] >>",
    ]
    write_pdf("named-links.pdf", named)
    quads = named[:8]
    quads[0] = b"<< /Type /Catalog /Pages 2 0 R >>"
    quads[4] = b"<< /Type /Annot /Subtype /Link /Rect [20 30 40 50] /F 4 /Border [0 0 0] /QuadPoints [22 32 32 32 30 40 20 40 33 42 38 42 38 48 33 48] /A << /S /URI /URI (https://example.org/quads) >> >>"
    quads[5] = b"<< /Type /Annot /Subtype /Link /Rect [45 30 65 50] /F 4 /Border [0 0 0] /Dest [4 0 R /Fit] >>"
    write_pdf("quad-links.pdf", quads)
    page_content = b"/Fm1 Do 0 0 1 rg 0 0 10 10 re f"
    outer_content = b"q /GS1 gs /Fm2 Do Q BT /F1 8 Tf 0 25 Td (Hi) Tj ET q 10 0 0 10 20 0 cm /Im1 Do Q"
    inner_content = b"1 0 0 rg 0 0 30 30 re f"
    forms = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] /Resources << /XObject << /Fm1 5 0 R >> >> /Contents 4 0 R >>",
        f"<< /Length {len(page_content)} >>\nstream\n".encode() + page_content + b"\nendstream",
        f"<< /Type /XObject /Subtype /Form /BBox [0 0 40 40] /Matrix [1 0 0 1 10 10] /Resources << /XObject << /Fm2 6 0 R /Im1 8 0 R >> /Font << /F1 7 0 R >> /ExtGState << /GS1 9 0 R >> >> /Length {len(outer_content)} >>\nstream\n".encode() + outer_content + b"\nendstream",
        f"<< /Type /XObject /Subtype /Form /BBox [0 0 12 12] /Length {len(inner_content)} >>\nstream\n".encode() + inner_content + b"\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        b"<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length 3 >>\nstream\n\x00\xff\x00\nendstream",
        b"<< /Type /ExtGState /ca 1 /CA 1 >>",
    ]
    write_pdf("forms.pdf", forms)
    cmyk_content = b"0 1 1 0 k 0 0 18 18 re f 1 0 0 0 K 2 w 42 42 18 18 re S q 20 0 0 20 20 20 cm /Im1 Do Q"
    cmyk_pixels = bytes([0, 255, 255, 0, 0, 0, 0, 255])
    cmyk = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] /Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>",
        f"<< /Length {len(cmyk_content)} >>\nstream\n".encode() + cmyk_content + b"\nendstream",
        b"<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /ColorSpace /DeviceCMYK /BitsPerComponent 8 /Length 8 >>\nstream\n" + cmyk_pixels + b"\nendstream",
    ]
    write_pdf("cmyk.pdf", cmyk)
    fixture_dir = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    cmyk_jpeg = (fixture_dir / "cmyk.jpg").read_bytes()
    cmyk_profile = (fixture_dir / "snap-newsprint-cmyk.icc").read_bytes()
    icc_content = b"q 24 0 0 12 20 20 cm /Im1 Do Q"
    cmyk_icc = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] /Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>",
        f"<< /Length {len(icc_content)} >>\nstream\n".encode() + icc_content + b"\nendstream",
        f"<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /ColorSpace [/ICCBased 6 0 R] /BitsPerComponent 8 /Filter /DCTDecode /Decode [1 0 1 0 1 0 1 0] /Length {len(cmyk_jpeg)} >>\nstream\n".encode() + cmyk_jpeg + b"\nendstream",
        f"<< /N 4 /Length {len(cmyk_profile)} >>\nstream\n".encode() + cmyk_profile + b"\nendstream",
    ]
    write_pdf("cmyk-icc-jpeg.pdf", cmyk_icc)
    ycck_jpeg = (fixture_dir / "ycck.jpg").read_bytes()
    ycck_icc = cmyk_icc.copy()
    ycck_icc[4] = (
        f"<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /ColorSpace [/ICCBased 6 0 R] /BitsPerComponent 8 /Filter /DCTDecode /Decode [1 0 1 0 1 0 1 0] /Length {len(ycck_jpeg)} >>\nstream\n".encode()
        + ycck_jpeg + b"\nendstream"
    )
    write_pdf("ycck-icc-jpeg.pdf", ycck_icc)
    page_color_content = b"/Spot cs .5 sc 0 0 10 10 re f /Fm1 Do /Fm2 Do /DeviceCMYK CS 1 0 0 0 SC 50 50 10 10 re S"
    cmyk_form_content = b"/Spot cs 0 1 1 0 sc 0 0 20 20 re f"
    rgb_form_content = b"/Spot cs 0 0 1 scn 0 0 20 20 re f"
    color_spaces = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] /Resources << /ColorSpace << /Spot /DeviceGray >> /XObject << /Fm1 5 0 R /Fm2 6 0 R >> >> /Contents 4 0 R >>",
        f"<< /Length {len(page_color_content)} >>\nstream\n".encode() + page_color_content + b"\nendstream",
        f"<< /Type /XObject /Subtype /Form /BBox [0 0 20 20] /Matrix [1 0 0 1 10 10] /Resources << /ColorSpace << /Spot /DeviceCMYK >> >> /Length {len(cmyk_form_content)} >>\nstream\n".encode() + cmyk_form_content + b"\nendstream",
        f"<< /Type /XObject /Subtype /Form /BBox [0 0 20 20] /Matrix [1 0 0 1 35 10] /Resources << /ColorSpace << /Spot /DeviceRGB >> >> /Length {len(rgb_form_content)} >>\nstream\n".encode() + rgb_form_content + b"\nendstream",
    ]
    write_pdf("color-spaces.pdf", color_spaces)


if __name__ == "__main__":
    main()
