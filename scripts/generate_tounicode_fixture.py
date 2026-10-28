"""Generate small Type1 and Type0 PDFs with one-byte and CID ToUnicode maps."""

from pathlib import Path
import zlib


def write_pdf(objects: list[bytes], name: str, info_ref: int | None = None) -> None:
    data = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for index, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data.extend(f"{index} 0 obj\n".encode())
        data.extend(obj)
        data.extend(b"\nendobj\n")
    xref = len(data)
    data.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        data.extend(f"{offset:010d} 00000 n \n".encode())
    info = f" /Info {info_ref} 0 R" if info_ref is not None else ""
    data.extend(
        f"trailer\n<< /Size {len(offsets)} /Root 1 0 R{info} >>\n"
        f"startxref\n{xref}\n%%EOF\n".encode()
    )
    fixture = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures" / name
    fixture.write_bytes(data)


def main() -> None:
    cmap = b"""/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
/CMapName /Adobe-Identity-UCS def
/CMapType 2 def
1 begincodespacerange
<00> <FF>
endcodespacerange
2 beginbfchar
<41> <4E2D>
<42> <6587>
endbfchar
endcmap
CMapName currentdict /CMap defineresource pop
end
end
"""
    content = b"BT /F1 12 Tf 10 20 Td [(A) 100 (B)] TJ ET"
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] "
        b"/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        b"<< /Length %d >>\nstream\n" % len(content) + content + b"\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica "
        b"/Encoding /WinAnsiEncoding /FirstChar 65 /LastChar 66 "
        b"/Widths [600 500] /ToUnicode 6 0 R >>",
        b"<< /Length %d >>\nstream\n" % len(cmap) + cmap + b"endstream",
    ]
    write_pdf(objects, "type1-tounicode.pdf")

    subset_objects = objects.copy()
    subset_objects[4] = (
        b"<< /Type /Font /Subtype /Type1 /BaseFont /ABCDEF+ExampleSubset "
        b"/Encoding << /Type /Encoding /BaseEncoding /WinAnsiEncoding "
        b"/Differences [65 /uni4E2D /uni6587] >> "
        b"/FirstChar 65 /LastChar 66 /Widths [600 500] /ToUnicode 6 0 R >>"
    )
    write_pdf(subset_objects, "type1-subset-tounicode.pdf")

    rotated_content = b"BT /F1 12 Tf 0 1 -1 0 30 20 Tm [(A) 100 (B)] TJ ET"
    rotated_objects = objects.copy()
    rotated_objects[3] = (
        b"<< /Length %d >>\nstream\n" % len(rotated_content)
        + rotated_content
        + b"\nendstream"
    )
    write_pdf(rotated_objects, "rotated-tounicode.pdf")

    def pdf_text(value: str) -> bytes:
        return b"<" + (b"\xfe\xff" + value.encode("utf-16-be")).hex().encode() + b">"

    metadata_objects = objects.copy()
    metadata_objects.append(
        b"<< /Title " + pdf_text("示例文档")
        + b" /Author (Alice) /Subject " + pdf_text("主题")
        + b" /Creator (ofdrw test) /Keywords " + pdf_text("测试, Rust")
        + b" /CreationDate (D:20240229153000+08'00')"
        + b" /ModDate (D:20240301100000Z) >>"
    )
    write_pdf(metadata_objects, "metadata-tounicode.pdf", info_ref=7)

    outline_objects = objects.copy()
    outline_objects[0] = b"<< /Type /Catalog /Pages 2 0 R /Outlines 7 0 R >>"
    outline_objects.extend([
        b"<< /Type /Outlines /First 8 0 R /Last 10 0 R /Count 2 >>",
        b"<< /Title " + pdf_text("章节一")
        + b" /Parent 7 0 R /Next 10 0 R /First 9 0 R /Last 9 0 R "
        b"/Count -1 /Dest [3 0 R /Fit] >>",
        b"<< /Title (Section 1.1) /Parent 8 0 R "
        b"/Dest [3 0 R /XYZ 10 20 null] >>",
        b"<< /Title (Chapter 2) /Parent 7 0 R /Prev 8 0 R "
        b"/A << /S /GoTo /D [3 0 R /Fit] >> >>",
    ])
    write_pdf(outline_objects, "outline-tounicode.pdf")

    cid_cmap = cmap.replace(b"<00> <FF>", b"<0000> <FFFF>")
    cid_cmap = cid_cmap.replace(b"<41>", b"<0001>").replace(b"<42>", b"<0002>")
    cid_content = b"BT /F1 12 Tf 10 20 Td [<0001> 100 <0002>] TJ ET"
    cid_objects = objects[:3] + [
        b"<< /Length %d >>\nstream\n" % len(cid_content) + cid_content + b"\nendstream",
        b"<< /Type /Font /Subtype /Type0 /BaseFont /TestCID "
        b"/Encoding /Identity-H /DescendantFonts [7 0 R] /ToUnicode 6 0 R >>",
        b"<< /Length %d >>\nstream\n" % len(cid_cmap) + cid_cmap + b"endstream",
        b"<< /Type /Font /Subtype /CIDFontType2 /BaseFont /TestCID "
        b"/CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> "
        b"/DW 1000 /W [1 [600 500]] >>",
    ]
    write_pdf(cid_objects, "type0-identity-tounicode.pdf")

    font_bytes = (Path(__file__).resolve().parents[1] /
                  "crates/ofdrw/tests/fixtures/dejavu-sans.ttf").read_bytes()
    compressed_font = zlib.compress(font_bytes, level=9)
    mac_content = b"BT /F1 12 Tf 10 20 Td (\x8e) Tj ET"
    mac_objects = objects[:3] + [
        b"<< /Length %d >>\nstream\n" % len(mac_content)
        + mac_content + b"\nendstream",
        b"<< /Type /Font /Subtype /TrueType /BaseFont /DejaVuSans "
        b"/Encoding /MacRomanEncoding /FirstChar 142 /LastChar 142 "
        b"/Widths [600] /FontDescriptor 6 0 R >>",
        b"<< /Type /FontDescriptor /FontName /DejaVuSans "
        b"/FontFile2 7 0 R >>",
        b"<< /Length %d /Length1 %d /Filter /FlateDecode >>\nstream\n"
        % (len(compressed_font), len(font_bytes))
        + compressed_font + b"\nendstream",
    ]
    write_pdf(mac_objects, "truetype-macroman.pdf")

    optional_content = (
        b"BT /F1 12 Tf 10 20 Td /OC /Hidden BDC (A) Tj EMC "
        b"/OC /Membership BDC (B) Tj EMC ET"
    )
    optional_objects = [
        b"<< /Type /Catalog /Pages 2 0 R /OCProperties << "
        b"/OCGs [6 0 R 7 0 R] /D << /BaseState /OFF /ON [6 0 R] >> >> >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] "
        b"/Resources << /Font << /F1 5 0 R >> /Properties << "
        b"/Hidden 7 0 R /Membership 8 0 R >> >> /Contents 4 0 R >>",
        b"<< /Length %d >>\nstream\n" % len(optional_content)
        + optional_content + b"\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica "
        b"/Encoding /WinAnsiEncoding /FirstChar 65 /LastChar 66 /Widths [600 600] >>",
        b"<< /Type /OCG /Name (Visible) >>",
        b"<< /Type /OCG /Name (Hidden) >>",
        b"<< /Type /OCMD /OCGs [6 0 R 7 0 R] /P /AnyOn >>",
    ]
    write_pdf(optional_objects, "optional-content.pdf")


if __name__ == "__main__":
    main()
