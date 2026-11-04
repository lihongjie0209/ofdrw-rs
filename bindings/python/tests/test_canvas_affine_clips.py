"""Compare binding output with independent Java2D geometry, using PNG's five filters."""
import copy
import json
from pathlib import Path
import struct
import zlib

import pytest
from ofdrw_rs import Document

ROOT = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/canvas-affine-clips-java"
CASES = json.loads((ROOT / "reference.json").read_text(encoding="utf-8"))["cases"]


def rgb_png(data):
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    width, height = struct.unpack_from(">II", data, 16)
    assert data[24] == 8 and data[25] in (2, 6) and data[28] == 0
    channels = 3 if data[25] == 2 else 4
    compressed = []
    offset = 8
    while offset < len(data):
        length = struct.unpack_from(">I", data, offset)[0]
        if data[offset + 4:offset + 8] == b"IDAT":
            compressed.append(data[offset + 8:offset + 8 + length])
        offset += length + 12
    raw = zlib.decompress(b"".join(compressed))
    stride = width * channels
    assert len(raw) == height * (stride + 1)
    previous = bytearray(stride)
    pixels = []
    for y in range(height):
        start = y * (stride + 1)
        kind = raw[start]
        assert kind in range(5)
        row = bytearray(raw[start + 1:start + 1 + stride])
        for i in range(stride):
            left = row[i - channels] if i >= channels else 0
            above = previous[i]
            diagonal = previous[i - channels] if i >= channels else 0
            prediction = 0
            if kind == 1:
                prediction = left
            elif kind == 2:
                prediction = above
            elif kind == 3:
                prediction = (left + above) // 2
            elif kind == 4:
                p = left + above - diagonal
                distances = [abs(p - left), abs(p - above), abs(p - diagonal)]
                prediction = [left, above, diagonal][distances.index(min(distances))]
            row[i] = (row[i] + prediction) & 255
        for x in range(width):
            i = x * channels
            if channels == 4:
                assert row[i + 3] == 255
            pixels.append(tuple(row[i:i + 3]))
        previous = row
    return width, height, pixels


def assert_raster_matches(actual_png, expected_png):
    width, height, actual = rgb_png(actual_png)
    expected_width, expected_height, expected = rgb_png(expected_png)
    assert (width, height) == (expected_width, expected_height)
    offsets = [-width - 1, -width, -width + 1, -1, 0, 1, width - 1, width, width + 1]
    interiors = [y * width + x for y in range(1, height - 1) for x in range(1, width - 1)]
    error = samples = 0
    for first, second in [(actual, expected), (expected, actual)]:
        white = [pixel == (255, 255, 255) for pixel in first]
        for i in interiors:
            if any(white[i + offset] for offset in offsets):
                continue
            color = first[i]
            if color != second[i]:
                error += min(sum(abs(a - b) for a, b in zip(color, second[i + offset])) for offset in offsets)
            samples += 3
    assert samples > 100 and error / samples < 4
    white = [pixel == (255, 255, 255) for pixel in expected]
    extra = sum(all(white[i + offset] for offset in offsets) and min(actual[i]) < 248 for i in interiors)
    assert extra / (width * height) < 0.005


def commands_for(case):
    commands = []
    for entry in copy.deepcopy(case["commands"]):
        if entry["type"] == "fill_rect":
            color = entry.pop("color")
            commands += [{"type": "set_line_width", "width": 0.353},
                         {"type": "set_fill_color", "color": color},
                         {"type": "set_stroke_color", "color": color}]
        if entry["type"] == "circle_arc":
            entry.update(start_angle=0, end_angle=360, sweep=True)
        commands.append(entry)
    return commands


@pytest.mark.parametrize("case", CASES, ids=lambda case: case["name"])
def test_affine_clips_match_java2d_and_survive_ofd_round_trip(case):
    commands = commands_for(case)
    saved = copy.deepcopy(commands)
    with Document.new() as doc:
        doc.add_blank_page(40, 30)
        assert doc.draw_canvas(0, (4, 3, 28, 22), commands)
        assert commands == saved
        assert doc.validate_references() == []
        with Document.from_bytes(doc.to_bytes()) as reopened:
            for ppm in [4, 12]:
                png = doc.page_png(0, ppm * 25.4)
                assert_raster_matches(png, (ROOT / f"{case['name']}-geometry-{ppm}.png").read_bytes())
                assert reopened.page_png(0, ppm * 25.4) == png
            assert reopened.validate_references() == []
