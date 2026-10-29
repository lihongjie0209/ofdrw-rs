"""Generate vertical CIDFont fixtures and optional independent Ghostscript rasters."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from generate_pdf_glyph_fixtures import objects
from generate_nested_stitching_fixture import pdf_bytes, stream


def vertical_objects(font, explicit=False, transformed=False):
    result = objects(font)
    result[2] = result[2].replace(b"144 96", b"144 144")
    result[4] = result[4].replace(b"Identity-H", b"Identity-V")
    content = b"BT /F1 24 Tf 70 Tz 1 Tc 2 Ts 60 130 Td [<0001> 100 <0002>] TJ <0001> Tj ET"
    if explicit:
        result[6] = result[6][:-2] + b" /DW2 [800 -1000] /W2 [1 [-900 250 700] 2 2 -1100 500 900] >>"
    if transformed:
        content = content.replace(b"60 130 Td", b"1 0.2 0.3 1 60 130 Tm").replace(b"70 Tz", b"-70 Tz")
    result[3] = stream(b"", content)
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--references", action="store_true")
    args = parser.parse_args()
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    font = (fixtures / "dejavu-serif.ttf").read_bytes()
    assert hashlib.sha256(font).hexdigest() == "8f2c103bfa3fd5de71f1b92b18f21906b5a26871fb7e19a9a4c9af539c3cc7ab"
    for name, explicit, transformed in [
        ("type0-vertical-default", False, False),
        ("type0-vertical-metrics", True, False),
        ("type0-vertical-transformed", True, True),
    ]:
        source = pdf_bytes(vertical_objects(font, explicit, transformed))
        (fixtures / f"{name}.pdf").write_bytes(source)
        if args.references:
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "source.pdf"
                path.write_bytes(source)
                subprocess.run(["gs", "-q", "-dSAFER", "-dBATCH", "-dNOPAUSE",
                                "-sDEVICE=png16m", "-r72", "-dTextAlphaBits=4",
                                f"-sOutputFile={fixtures / name}.png", str(path)], check=True)
