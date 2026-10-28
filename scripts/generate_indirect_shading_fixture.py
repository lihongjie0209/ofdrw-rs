"""Store every function parameter and array member behind indirect references.

The generated PDFs must render exactly like the direct nested stitching fixtures.
The serializer and object graph come from that fixture generator.
"""
from pathlib import Path
import re

from generate_nested_stitching_fixture import nested_stitching_objects, pdf_bytes


PARAMETER = re.compile(
    rb"/(FunctionType|Domain|Range|Functions|Bounds|Encode|Decode|Size|Order|BitsPerSample|N|C0|C1)"
    rb"\s+(\[[^\]]*\]|[-+\d.]+)"
)
VALUE = re.compile(rb"\d+\s+\d+\s+R|[-+]?(?:\d*\.)?\d+")


def indirect_function_objects(kind):
    objects = nested_stitching_objects(kind)
    objects[8] = objects[8].replace(
        b"/Size [5]", b"/Decode [0 1 0 1 0 1] /Encode [0 4] /Size [5]")

    def reference(value):
        if re.fullmatch(rb"\d+\s+\d+\s+R", value):
            return value
        objects.append(value)
        return f"{len(objects)} 0 R".encode()

    def replace(match):
        key, value = match.groups()
        if value.startswith(b"["):
            value = b"[" + b" ".join(reference(item) for item in VALUE.findall(value[1:-1])) + b"]"
        return b"/" + key + b" " + reference(value)

    for index in range(5, 10):
        header, separator, data = objects[index].partition(b"\nstream\n")
        objects[index] = PARAMETER.sub(replace, header) + separator + data
    return objects


if __name__ == "__main__":
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    for kind in ["axial", "radial"]:
        (fixtures / f"shading-indirect-functions-{kind}.pdf").write_bytes(
            pdf_bytes(indirect_function_objects(kind)))
