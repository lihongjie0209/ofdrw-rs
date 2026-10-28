"""Generate cubic PDF gradients using a Flate-compressed 12-bit sampled function.

The independent pixel reference was rendered with Ghostscript 10.02.1:
gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m -r72 -dGraphicsAlphaBits=1
   -sOutputFile=shading-sampled-cubic-reference.png shading-sampled-cubic-axial.pdf
"""

from pathlib import Path
import zlib


def stream(dictionary: bytes, data: bytes) -> bytes:
    return (b"<< " + dictionary + f" /Length {len(data)} >>\nstream\n".encode()
            + data + b"\nendstream")


values = [4095, 0, 0, 0, 4095, 0, 0, 0, 4095, 4095, 4095, 4095, 4095, 0, 0]
sample_bits = "".join(f"{value:012b}" for value in values)
sample_bits += "0" * (-len(sample_bits) % 8)
samples = int(sample_bits, 2).to_bytes(len(sample_bits) // 8, "big")
fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"

for kind, shading in [
    ("axial", b"/ShadingType 2 /Coords [0 16 96 16]"),
    ("radial", b"/ShadingType 3 /Coords [48 16 0 48 16 48]"),
]:
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 32] "
        b"/Resources << /Shading << /Sh1 5 0 R >> >> /Contents 4 0 R >>",
        stream(b"", b"/Sh1 sh"),
        b"<< " + shading + b" /ColorSpace /DeviceRGB /Domain [-1 1] "
        b"/Function 6 0 R /Extend [true true] >>",
        stream(b"/FunctionType 0 /Domain [-1 1] /Range [0 1 0 1 0 1] "
               b"/Size [5] /Order 3 /BitsPerSample 12 /Filter /FlateDecode", zlib.compress(samples)),
    ]
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
    (fixtures / f"shading-sampled-cubic-{kind}.pdf").write_bytes(pdf)
