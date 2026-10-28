"""Generate nested stitching functions with reversed mappings and a sampled/exponential mix.

Reference (Ghostscript 10.02.1):
gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m -r72 -dGraphicsAlphaBits=1
   -sOutputFile=shading-nested-stitching-reference.png shading-nested-stitching-axial.pdf
"""
from pathlib import Path
import zlib


def stream(dictionary, data):
    return (b"<< " + dictionary + f" /Length {len(data)} >>\nstream\n".encode()
            + data + b"\nendstream")


def nested_stitching_objects(kind):
    # Keep branch boundaries on pixel edges and cubic intervals at least 24 pixels
    # wide at 72 dpi for comparison between independent rasterizers.
    shading = {
        "axial": b"/ShadingType 2 /Coords [0 16 384 16]",
        "radial": b"/ShadingType 3 /Coords [192 16 0 192 16 192]",
    }[kind]
    values = [4095, 0, 0, 0, 4095, 0, 0, 0, 4095, 4095, 4095, 4095, 4095, 0, 0]
    bits = "".join(f"{value:012b}" for value in values)
    bits += "0" * (-len(bits) % 8)
    samples = int(bits, 2).to_bytes(len(bits) // 8, "big")
    return [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 384 32] "
        b"/Resources << /Shading << /Sh1 5 0 R >> >> /Contents 4 0 R >>",
        stream(b"", b"/Sh1 sh"),
        b"<< " + shading + b" /ColorSpace /DeviceRGB /Domain [-1 1] "
        b"/Function 6 0 R /Extend [true true] >>",
        b"<< /FunctionType 3 /Domain [-1 1] /Range [0 1 0 1 0 1] "
        b"/Functions [7 0 R 8 0 R] /Bounds [0] /Encode [0 1 1 0] >>",
        b"<< /FunctionType 3 /Domain [0 1] /Range [0 1 0 1 0 1] "
        b"/Functions [9 0 R 10 0 R] /Bounds [.5] /Encode [0 1 1 0] >>",
        b"<< /FunctionType 3 /Domain [0 1] /Range [0 1 0 1 0 1] "
        b"/Functions [10 0 R 9 0 R] /Bounds [.5] /Encode [1 0 0 1] >>",
        stream(b"/FunctionType 0 /Domain [0 1] /Range [0 1 0 1 0 1] "
               b"/Size [5] /Order 3 /BitsPerSample 12 /Filter /FlateDecode", zlib.compress(samples)),
        b"<< /FunctionType 2 /Domain [0 1] /C0 [.2 .1 .9] /C1 [.9 .8 .2] /N 2 >>",
    ]


def pdf_bytes(objects):
    pdf = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for index, obj in enumerate(objects, 1):
        offsets.append(len(pdf))
        pdf.extend(f"{index} 0 obj\n".encode() + obj + b"\nendobj\n")
    xref = len(pdf)
    pdf.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        pdf.extend(f"{offset:010d} 00000 n \n".encode())
    pdf.extend((f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\n"
                f"startxref\n{xref}\n%%EOF\n").encode())
    return bytes(pdf)


if __name__ == "__main__":
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    for kind in ["axial", "radial"]:
        (fixtures / f"shading-nested-stitching-{kind}.pdf").write_bytes(
            pdf_bytes(nested_stitching_objects(kind)))
