"""Generate embedded-font PDFs with Unicode sequences and exact source glyphs.

DejaVu Serif glyph IDs are fixed by the checked source-font digest below.
Reference raster: gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m -r72
  -dTextAlphaBits=4 -sOutputFile=pdf-ligature-reference.png type0-ligature-glyph.pdf
"""
import argparse
import hashlib
from pathlib import Path
import struct
import subprocess
import tempfile
import zlib

from generate_cid_cmap_fixture import cmap
from generate_nested_stitching_fixture import pdf_bytes, stream


def without_unicode_cmap(font):
    """Remove the sfnt cmap directory entry without moving any glyph tables."""
    font = bytearray(font)
    count = int.from_bytes(font[4:6], "big")
    records = [font[12 + index * 16:28 + index * 16] for index in range(count)]
    head = next(record for record in records if record[:4] == b"head")
    head_offset = int.from_bytes(head[8:12], "big")
    records = [record for record in records if record[:4] != b"cmap"]
    assert len(records) == count - 1
    count = len(records)
    power = count.bit_length() - 1
    font[4:12] = struct.pack(">HHHH", count, 16 * (1 << power), power, 16 * count - 16 * (1 << power))
    font[12:12 + (count + 1) * 16] = b"".join(records) + bytes(16)
    font[head_offset + 8:head_offset + 12] = bytes(4)
    padded = bytes(font) + bytes((-len(font)) % 4)
    checksum = sum(struct.unpack(f">{len(padded) // 4}I", padded)) & 0xffffffff
    font[head_offset + 8:head_offset + 12] = ((0xb1b0afba - checksum) & 0xffffffff).to_bytes(4, "big")
    return bytes(font)


def with_sequence_glyph_name(font):
    """Rename fi in post format 2 and rebuild sfnt offsets and checksums."""
    count = int.from_bytes(font[4:6], "big")
    tables = []
    for index in range(count):
        tag, _, offset, length = struct.unpack(">4sIII", font[12 + index * 16:28 + index * 16])
        data = bytearray(font[offset:offset + length])
        if tag == b"post":
            assert data[:4] == bytes([0, 2, 0, 0])
            start = 34 + 2 * int.from_bytes(data[32:34], "big")
            names = []
            while start < len(data):
                size = data[start]
                name = bytes(data[start + 1:start + 1 + size])
                names.append(name)
                start += size + 1
            data[34 + 2 * 3315:36 + 2 * 3315] = (258 + len(names)).to_bytes(2, "big")
            names.append(b"uni00660069")
            start = 34 + 2 * int.from_bytes(data[32:34], "big")
            data = data[:start] + b"".join(bytes([len(name)]) + name for name in names)
        if tag == b"head":
            data[8:12] = bytes(4)
        tables.append((tag, data))
    result = bytearray(font[:12] + bytes(16 * count))
    head_offset = None
    for index, (tag, data) in enumerate(tables):
        offset = len(result)
        padded = data + bytes((-len(data)) % 4)
        checksum = sum(struct.unpack(f">{len(padded) // 4}I", padded)) & 0xffffffff
        result[12 + index * 16:28 + index * 16] = struct.pack(">4sIII", tag, checksum, offset, len(data))
        result.extend(padded)
        if tag == b"head":
            head_offset = offset
    checksum = sum(struct.unpack(f">{len(result) // 4}I", result)) & 0xffffffff
    result[head_offset + 8:head_offset + 12] = ((0xb1b0afba - checksum) & 0xffffffff).to_bytes(4, "big")
    return bytes(result)


def objects(font, simple=False, variation=False):
    first = b"41" if simple else (b"00e6" if variation else b"0001")
    second = b"42" if simple else b"0002"
    content = (b"BT /F1 24 Tf 10 60 Td <00e6> Tj <00e6> Tj ET" if variation else
               b"BT /F1 24 Tf 10 60 Td [<" + first + b"> 100 <" + second + b">] TJ <" + first + b"> Tj ET")
    spaces = b"1 begincodespacerange <00> <ff> endcodespacerange" if simple else b"1 begincodespacerange <0000> <ffff> endcodespacerange"
    unicode = cmap(2, spaces + b"\n2 beginbfchar <" + first + b"> <00660069> <" + second + b"> <0041> endbfchar")
    dictionary = (b"<< /Type /Font /Subtype /TrueType /BaseFont /DejaVuSerif /ToUnicode 6 0 R /FontDescriptor 8 0 R "
                  b"/Encoding << /BaseEncoding /WinAnsiEncoding /Differences [65 /fi /A] >> /FirstChar 65 /LastChar 66 /Widths [750 722] >>"
                  if simple else b"<< /Type /Font /Subtype /Type0 /BaseFont /DejaVuSerif /Encoding /Identity-H /DescendantFonts [7 0 R] "
                  + (b"" if variation else b"/ToUnicode 6 0 R ") + b">>")
    descendant = (b"<< /Type /Font /Subtype /CIDFontType2 /BaseFont /DejaVuSerif /CIDSystemInfo << /Registry (Adobe) "
                  + (b"/Ordering (Japan1) /Supplement 7" if variation else b"/Ordering (Identity) /Supplement 0")
                  + b" >> /FontDescriptor 8 0 R /CIDToGIDMap 10 0 R /DW 1000 /W "
                  + (b"[230 [750]]" if variation else b"[1 [750 722]]") + b" >>")
    gid_map = bytearray(462 if variation else 6)
    if variation:
        gid_map[460:462] = (19).to_bytes(2, "big")  # zero
    else:
        gid_map[2:4] = (3315).to_bytes(2, "big")  # fi ligature
        gid_map[4:6] = (36).to_bytes(2, "big")  # A
    return [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 144 96] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        stream(b"", content), dictionary, stream(b"", unicode), descendant,
        b"<< /Type /FontDescriptor /FontName /DejaVuSerif /Flags 32 /FontBBox [-558 -347 1600 1100] "
        b"/ItalicAngle 0 /Ascent 928 /Descent -236 /CapHeight 928 /StemV 80 /FontFile2 9 0 R >>",
        stream(f"/Length1 {len(font)} /Filter /FlateDecode".encode(), zlib.compress(font, 9)),
        stream(b"", bytes(gid_map)),
    ]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--references", action="store_true", help="render independent Ghostscript PNG references")
    args = parser.parse_args()
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    font = (fixtures / "dejavu-serif.ttf").read_bytes()
    assert hashlib.sha256(font).hexdigest() == "8f2c103bfa3fd5de71f1b92b18f21906b5a26871fb7e19a9a4c9af539c3cc7ab"
    for name, simple, variation in [("type0-ligature-glyph.pdf", False, False),
                                    ("truetype-ligature-glyph.pdf", True, False),
                                    ("type0-variation-glyph.pdf", False, True)]:
        (fixtures / name).write_bytes(pdf_bytes(objects(font, simple, variation)))
    (fixtures / "type0-no-unicode-cmap-glyph.pdf").write_bytes(pdf_bytes(objects(without_unicode_cmap(font))))
    sequence = objects(with_sequence_glyph_name(font), simple=True)
    sequence[4] = sequence[4].replace(b"/ToUnicode 6 0 R ", b"").replace(b"/fi /A", b"/uni00660069 /A")
    (fixtures / "truetype-glyph-name-sequence.pdf").write_bytes(pdf_bytes(sequence))
    if args.references:
        overhang = objects(font)
        overhang[9] = stream(b"", bytes([0, 0, 0, 77, 0, 36]))
        for source, name in [(pdf_bytes(objects(font)), "pdf-ligature-reference.png"),
                             (pdf_bytes(overhang), "pdf-overhang-reference.png")]:
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "source.pdf"
                path.write_bytes(source)
                subprocess.run(["gs", "-q", "-dSAFER", "-dBATCH", "-dNOPAUSE", "-sDEVICE=png16m", "-r72",
                                "-dTextAlphaBits=4", f"-sOutputFile={fixtures / name}", str(path)], check=True)
