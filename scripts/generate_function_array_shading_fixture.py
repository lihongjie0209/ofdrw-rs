"""Generate RGB function arrays mixing sampled, exponential, stitching and calculator functions.

Reference (Ghostscript 10.02.1):
gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m -r72 -dGraphicsAlphaBits=1
   -sOutputFile=shading-function-array-reference.png shading-function-array-axial.pdf
"""
from pathlib import Path
import zlib

from generate_nested_stitching_fixture import pdf_bytes, stream


def function_array_objects(kind):
    shading = {
        "axial": b"/ShadingType 2 /Coords [0 16 384 16]",
        "radial": b"/ShadingType 3 /Coords [192 16 0 192 16 192]",
    }[kind]
    red = b"".join(value.to_bytes(2, "big") for value in [65535, 65535, 0, 0, 0])
    blue = b"{dup .5 lt {pop 0} {.5 sub 2 mul} ifelse}"
    return [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 384 32] "
        b"/Resources << /Shading << /Sh1 5 0 R >> >> /Contents 4 0 R >>",
        stream(b"", b"/Sh1 sh"),
        b"<< " + shading + b" /ColorSpace /DeviceRGB /Domain [0 1] "
        b"/Function 6 0 R /Extend [true true] >>",
        b"[7 0 R 8 0 R 9 0 R]",
        stream(b"/FunctionType 0 /Domain [0 1] /Range [0 1] /Size [5] "
               b"/Order 3 /BitsPerSample 16 /Filter /FlateDecode", zlib.compress(red)),
        b"<< /FunctionType 3 /Domain [0 1] /Range [0 1] /Functions [10 0 R 10 0 R] "
        b"/Bounds [.5] /Encode [0 1 1 0] >>",
        stream(b"/FunctionType 4 /Domain [0 1] /Range [0 1] /Filter /FlateDecode", zlib.compress(blue)),
        b"<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 2 >>",
    ]


if __name__ == "__main__":
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    for kind in ["axial", "radial"]:
        (fixtures / f"shading-function-array-{kind}.pdf").write_bytes(
            pdf_bytes(function_array_objects(kind)))
