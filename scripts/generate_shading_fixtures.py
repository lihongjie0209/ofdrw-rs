"""Generate small deterministic PDF shading fixtures without third-party packages."""

from pathlib import Path


def write_pdf(path: Path, stream: bytes, shading: bytes, function: bytes) -> None:
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
        (b"<< /Type /Page /Parent 2 0 R /MediaBox [10 20 82 92] "
         b"/Contents 4 0 R /Resources << /Shading << /Sh1 5 0 R >> >> >>"),
        b"<< /Length " + str(len(stream)).encode() + b" >>\nstream\n" + stream + b"endstream",
        shading,
        function,
    ]
    output = bytearray(b"%PDF-1.4\n%" + bytes([226, 227, 207, 211]) + b"\n")
    offsets = [0]
    for index, obj in enumerate(objects, 1):
        offsets.append(len(output))
        output.extend(f"{index} 0 obj\n".encode() + obj + b"\nendobj\n")
    xref = len(output)
    output.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        output.extend(f"{offset:010d} 00000 n \n".encode())
    output.extend(
        f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\n"
        f"startxref\n{xref}\n%%EOF\n".encode()
    )
    path.write_bytes(output)


def main() -> None:
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    function = b"<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>"
    common = b"/ColorSpace /DeviceRGB /Extend [true true] /Function 6 0 R >>"
    clip = b"q 10 20 72 36 re W n /Sh1 sh Q\n"
    write_pdf(
        fixtures / "shading-axial.pdf",
        clip,
        b"<< /ShadingType 2 /Coords [10 20 82 20] " + common,
        function,
    )
    write_pdf(
        fixtures / "shading-bbox.pdf",
        b"q 38 28 36 54 re W n /Sh1 sh Q\n",
        (b"<< /ShadingType 2 /Coords [10 20 82 20] /BBox [28 38 64 74] "
         b"/Background [0 1 0] " + common),
        function,
    )
    stitching = (
        b"<< /FunctionType 3 /Domain [0 1] /Bounds [0.5] /Encode [0 1 1 0] "
        b"/Functions ["
        b"<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 1 0] /N 1 >> "
        b"<< /FunctionType 2 /Domain [0 1] /C0 [1 1 1] /C1 [0 0 1] /N 1 >>"
        b"] >>"
    )
    write_pdf(
        fixtures / "shading-stitching.pdf",
        b"/Sh1 sh\n",
        b"<< /ShadingType 2 /Coords [10 20 82 20] " + common,
        stitching,
    )
    ranged = (
        b"<< /FunctionType 2 /Domain [0 1] /Range [0.2 0.8 0 1 0 1] "
        b"/C0 [-0.5 0 0] /C1 [1.5 0 0] /N 1 >>"
    )
    write_pdf(
        fixtures / "shading-range.pdf",
        b"/Sh1 sh\n",
        b"<< /ShadingType 2 /Coords [10 20 82 20] " + common,
        ranged,
    )
    write_pdf(
        fixtures / "shading-cmyk-range.pdf",
        b"/Sh1 sh\n",
        (b"<< /ShadingType 2 /Coords [10 20 82 20] /ColorSpace /DeviceCMYK "
         b"/Extend [true true] /Function 6 0 R >>"),
        (b"<< /FunctionType 2 /Domain [0 1] /Range [0 1 0 1 0 1 0 1] "
         b"/C0 [0 0 0 0] /C1 [1 0 0 1] /N 1 >>"),
    )
    stitching_range = stitching.replace(
        b"/C0 [1 0 0] /C1 [0 1 0]",
        b"/Range [0.2 0.8 0 1 0 1] /C0 [1 0 0] /C1 [0 1 0]",
    )
    write_pdf(
        fixtures / "shading-stitching-range.pdf",
        b"/Sh1 sh\n",
        b"<< /ShadingType 2 /Coords [10 20 82 20] " + common,
        stitching_range,
    )
    for name, interpolation in [
        ("shading-domain.pdf", function),
        ("shading-stitching-domain.pdf", stitching),
    ]:
        write_pdf(
            fixtures / name,
            b"/Sh1 sh\n",
            (b"<< /ShadingType 2 /Coords [10 20 82 20] /Domain [0.25 0.75] "
             + common),
            interpolation,
        )
    for name, flags in [
        ("none", b"[false false]"),
        ("start", b"[true false]"),
        ("end", b"[false true]"),
        ("both", b"[true true]"),
    ]:
        write_pdf(
            fixtures / f"shading-extend-{name}.pdf",
            b"/Sh1 sh\n",
            (b"<< /ShadingType 2 /Coords [22 20 70 20] /ColorSpace /DeviceRGB "
             b"/Extend " + flags + b" /Function 6 0 R >>"),
            function,
        )
    for name, coords in [
        ("shading-radial.pdf", b"[46 56 0 46 56 36]"),
        ("shading-radial-reverse.pdf", b"[46 56 36 46 56 0]"),
    ]:
        write_pdf(
            fixtures / name,
            b"/Sh1 sh\n",
            b"<< /ShadingType 3 /Coords " + coords + b" " + common,
            function,
        )


if __name__ == "__main__":
    main()
