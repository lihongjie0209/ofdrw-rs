"""Generate Type 4 shadings with arithmetic, trigonometry and a branch jump.

Reference (Ghostscript 10.02.1):
gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m -r72 -dGraphicsAlphaBits=1
   -sOutputFile=shading-calculator-reference.png shading-calculator-axial.pdf
"""
from pathlib import Path
import zlib

from generate_nested_stitching_fixture import pdf_bytes, stream


CODE = b"""{
  % red to green, followed by a jump to blue and a sine curve to white
  dup .5 lt
    { 2 mul dup mul dup 1 exch sub exch 0 }
    { .5 sub 2 mul 90 mul sin dup 1 }
  ifelse
}"""


def calculator_shading_objects(kind):
    shading = {
        "axial": b"/ShadingType 2 /Coords [0 16 384 16]",
        "radial": b"/ShadingType 3 /Coords [192 16 0 192 16 192]",
    }[kind]
    return [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 384 32] "
        b"/Resources << /Shading << /Sh1 5 0 R >> >> /Contents 4 0 R >>",
        stream(b"", b"/Sh1 sh"),
        b"<< " + shading + b" /ColorSpace /DeviceRGB /Domain [0 1] "
        b"/Function 6 0 R /Extend [true true] >>",
        stream(b"/FunctionType 4 /Domain [0 1] /Range [0 1 0 1 0 1] "
               b"/Filter /FlateDecode", zlib.compress(CODE)),
    ]


if __name__ == "__main__":
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    for kind in ["axial", "radial"]:
        (fixtures / f"shading-calculator-{kind}.pdf").write_bytes(
            pdf_bytes(calculator_shading_objects(kind)))
