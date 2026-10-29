"""Generate CJK named vertical PDFs, with and without ToUnicode maps."""
from pathlib import Path
import re

from generate_nested_stitching_fixture import pdf_bytes, stream


if __name__ == "__main__":
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    for family in ["gbk", "cns", "jis", "ks"]:
        source = (fixtures / f"type0-{family}-cmap.pdf").read_bytes()
        objects = re.findall(rb"\d+ 0 obj\n(.*?)\nendobj", source, re.S)
        assert len(objects) == 7
        objects[4] = objects[4].replace(b"-H", b"-V")
        data = objects[3].split(b"stream\n", 1)[1].rsplit(b"endstream", 1)[0]
        objects[3] = stream(b"", data.replace(b"10 20 Td", b"40 60 Td"))
        # Vertical displacement deliberately differs from horizontal W widths.
        objects[6] = objects[6][:-2] + b" /DW2 [800 -900] >>"
        (fixtures / f"type0-{family}-vertical-cmap.pdf").write_bytes(pdf_bytes(objects))
        objects[4] = objects[4].replace(b" /ToUnicode 6 0 R", b"")
        (fixtures / f"type0-{family}-vertical-collection.pdf").write_bytes(pdf_bytes(objects))
