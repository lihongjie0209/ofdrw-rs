"""Generate a Type0 font with indirect encoding and both CID width-table forms."""
from pathlib import Path
import re

from generate_nested_stitching_fixture import pdf_bytes


def indirect_cid_pdf():
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    # This fixed source fixture has seven objects, with no object delimiters in streams.
    objects = re.findall(rb"\d+ 0 obj\n(.*?)\nendobj", (fixtures / "type0-identity-tounicode.pdf").read_bytes(), re.S)
    assert len(objects) == 7
    objects[4] = objects[4].replace(b"/Encoding /Identity-H", b"/Encoding 8 0 R")
    objects[6] = objects[6].replace(b"/DW 1000 /W [1 [600 500]]", b"/DW 9 0 R /W 10 0 R")
    objects.extend([
        b"/Identity-H", b"1000", b"[11 0 R 12 0 R 15 0 R 16 0 R 17 0 R]",
        b"1", b"[13 0 R 14 0 R]", b"600", b"500", b"3", b"4", b"700",
    ])
    return pdf_bytes(objects)


if __name__ == "__main__":
    target = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures/type0-indirect-widths.pdf"
    target.write_bytes(indirect_cid_pdf())
