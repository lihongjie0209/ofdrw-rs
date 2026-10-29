"""Generate Type0 text with independent mixed-length source codes and CIDs."""
from pathlib import Path
import re
import zlib

from generate_nested_stitching_fixture import pdf_bytes, stream


def cmap(kind, sections):
    return (b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n"
            b"/CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> def\n"
            b"/CMapName /MixedCodes def\n/CMapType " + str(kind).encode() + b" def\n/WMode 0 def\n"
            + sections + b"\nendcmap\nCMapName currentdict /CMap defineresource pop\nend\nend")


if __name__ == "__main__":
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    original = re.findall(rb"\d+ 0 obj\n(.*?)\nendobj", (fixtures / "type0-identity-tounicode.pdf").read_bytes(), re.S)
    assert len(original) == 7
    for all_lengths in [False, True]:
        objects = original.copy()
        spaces = (b"4 begincodespacerange <00> <7f> <8140> <81ff> <900000> <9000ff> <a0000000> <a00000ff> endcodespacerange"
                  if all_lengths else b"2 begincodespacerange <00> <7f> <8140> <81ff> endcodespacerange")
        codes = (b"4 begincidchar <41> 1 <8140> 2 <900001> 3 <a0000002> 4 endcidchar"
                 if all_lengths else b"1 begincidchar <41> 1 endcidchar\n1 begincidrange <8140> <8140> 2 endcidrange")
        unicode = (b"4 beginbfchar <41> <4e2d> <8140> <6587> <900001> <0041> <a0000002> <0042> endbfchar"
                   if all_lengths else b"2 beginbfchar <41> <4e2d> <8140> <6587> endbfchar")
        content = (b"BT /F1 12 Tf 10 20 Td <418140900001a0000002> Tj <41> Tj ET"
                   if all_lengths else b"BT /F1 12 Tf 10 20 Td [<41> 100 <8140>] TJ ET")
        objects[3] = stream(b"", content)
        objects[4] = objects[4].replace(b"/Encoding /Identity-H", b"/Encoding 8 0 R")
        objects[5] = stream(b"", cmap(2, spaces + b"\n" + unicode))
        objects[6] = objects[6].replace(b"[600 500]", b"[600 500 700 800]")
        objects.append(stream(b"/Type /CMap /CMapName /MixedCodes /WMode 0 /Filter /FlateDecode",
                              zlib.compress(cmap(1, spaces + b"\n" + codes))))
        name = "type0-four-byte-cmap.pdf" if all_lengths else "type0-mixed-cmap.pdf"
        (fixtures / name).write_bytes(pdf_bytes(objects))
