"""Generate registered-CID PDFs without explicit ToUnicode mappings."""
from pathlib import Path
import re

from generate_nested_stitching_fixture import pdf_bytes, stream
from generate_cid_cmap_fixture import cmap


if __name__ == "__main__":
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    for family in ["gbk", "cns", "jis", "ks"]:
        source = fixtures / f"type0-{family}-cmap.pdf"
        objects = re.findall(rb"\d+ 0 obj\n(.*?)\nendobj", source.read_bytes(), re.S)
        assert len(objects) == 7
        objects[4] = objects[4].replace(b" /ToUnicode 6 0 R", b"")
        (fixtures / f"type0-{family}-collection-unicode.pdf").write_bytes(pdf_bytes(objects))
        if family == "gbk":
            identity = objects.copy()
            identity[4] = identity[4].replace(b"/GBK-EUC-H", b"/Identity-H")
            identity[3] = stream(b"", b"BT /F1 12 Tf 10 20 Td [<11CF> 100 <0ED3>] TJ ET")
            (fixtures / "type0-identity-collection-unicode.pdf").write_bytes(pdf_bytes(identity))
            embedded = objects.copy()
            embedded[4] = embedded[4].replace(b"/Encoding /GBK-EUC-H", b"/Encoding 8 0 R")
            embedded[3] = stream(b"", b"BT /F1 12 Tf 10 20 Td [<41> 100 <8140>] TJ ET")
            data = cmap(1, b"2 begincodespacerange <00> <7f> <8140> <81ff> endcodespacerange\n"
                          b"2 begincidchar <41> 4559 <8140> 3795 endcidchar")
            data = data.replace(b"/Ordering (Identity)", b"/Ordering (GB1)")
            embedded.append(stream(b"", data))
            (fixtures / "type0-embedded-collection-unicode.pdf").write_bytes(pdf_bytes(embedded))
        if family == "jis":
            supplementary = objects.copy()
            supplementary[4] = supplementary[4].replace(b"/UniJIS-UTF16-H", b"/Adobe-Japan1-7")
            supplementary[3] = stream(b"", b"BT /F1 12 Tf 10 20 Td <4ABA> Tj ET")
            supplementary[6] = supplementary[6].replace(b"/W [3284 [600] 3722 [500]]", b"/W [19130 [600]]")
            (fixtures / "type0-supplementary-collection-unicode.pdf").write_bytes(pdf_bytes(supplementary))
