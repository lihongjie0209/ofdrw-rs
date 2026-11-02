import json
import math
from pathlib import Path
import struct

import pytest
from ofdrw_rs import canvas_mm_to_pixels, canvas_pixels_to_mm, OfdError

ROOT = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"
CASES = json.loads((ROOT / "canvas-units-java.json").read_text(encoding="utf-8"))["cases"]


def from_bits(value):
    return struct.unpack(">d", bytes.fromhex(value))[0]


def test_canvas_unit_conversions_match_all_java_reference_bits():
    assert len(CASES) == 329
    for case in CASES:
        ppm = from_bits(case["ppm_bits"])
        if case["op"] == "mm":
            actual = canvas_pixels_to_mm(case["input"], ppm)
            assert struct.pack(">d", actual).hex() == case["expected_bits"]
        else:
            assert canvas_mm_to_pixels(from_bits(case["input_bits"]), ppm) == case["expected"]
    assert canvas_pixels_to_mm(378) == 100
    assert canvas_mm_to_pixels(100) == 378


def test_canvas_unit_conversions_reject_invalid_density_and_pixel_range():
    for ppm in [0, -0.0, -1, math.nan, math.inf, -math.inf]:
        with pytest.raises(OfdError):
            canvas_pixels_to_mm(4, ppm)
        with pytest.raises(OfdError):
            canvas_mm_to_pixels(4, ppm)
    for pixels in [-(2**31)-1, 2**31]:
        with pytest.raises(ValueError):
            canvas_pixels_to_mm(pixels)
    for pixels in [1.5, "1", True, None]:
        with pytest.raises(TypeError):
            canvas_pixels_to_mm(pixels)
