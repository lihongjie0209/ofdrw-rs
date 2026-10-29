"""Generate named-CMap PDFs using known CIDs from pinned Adobe resources."""
from pathlib import Path
import re

from generate_nested_stitching_fixture import pdf_bytes, stream


if __name__ == "__main__":
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    original = re.findall(rb"\d+ 0 obj\n(.*?)\nendobj", (fixtures / "type0-identity-tounicode.pdf").read_bytes(), re.S)
    assert len(original) == 7
    for suffix, name, ordering, supplement, first, second, cid_first, cid_second, text in [
        ("gbk", "GBK-EUC-H", "GB1", 6, "D6D0", "CEC4", 4559, 3795, "中文"),
        ("cns", "UniCNS-UTF16-H", "CNS1", 7, "4E2D", "6587", 661, 726, "中文"),
        ("jis", "UniJIS-UTF16-H", "Japan1", 7, "65E5", "672C", 3284, 3722, "日本"),
        ("ks", "UniKS-UTF16-H", "Korea1", 2, "D55C", "AD6D", 3296, 1204, "한국"),
    ]:
        objects = original.copy()
        objects[3] = stream(b"", f"BT /F1 12 Tf 10 20 Td [<{first}> 100 <{second}>] TJ ET".encode())
        objects[4] = objects[4].replace(b"/Identity-H", b"/" + name.encode())
        source = objects[5]
        data = source.split(b"stream\n", 1)[1].rsplit(b"endstream", 1)[0]
        data = data.replace(b"<0001>", f"<{first}>".encode()).replace(b"<0002>", f"<{second}>".encode())
        for old, character in [(b"<4E2D>", text[0]), (b"<6587>", text[1])]:
            data = data.replace(old, b"<" + character.encode("utf-16-be").hex().upper().encode() + b">")
        objects[5] = stream(b"", data)
        objects[6] = objects[6].replace(b"/Ordering (Identity)", b"/Ordering (" + ordering.encode() + b")")
        objects[6] = objects[6].replace(b"/Supplement 0", f"/Supplement {supplement}".encode())
        objects[6] = objects[6].replace(b"/W [1 [600 500]]", f"/W [{cid_first} [600] {cid_second} [500]]".encode())
        (fixtures / f"type0-{suffix}-cmap.pdf").write_bytes(pdf_bytes(objects))
