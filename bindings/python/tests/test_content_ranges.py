import copy
import json
from pathlib import Path

import pytest
from ofdrw_rs import Document, OfdError

FIXTURES = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"


@pytest.mark.parametrize("case", json.loads((FIXTURES / "box-content-ranges.json").read_text())["cases"], ids=lambda c: c["name"])
def test_protected_content_ranges_preserve_fragments_source_offsets_and_inputs(case):
    request = copy.deepcopy(case["request"])
    items, policies = request.pop("items"), request.pop("content_ranges")
    original = copy.deepcopy((items, policies))
    with Document.new() as doc:
        result = doc.add_box_layout_with_content_ranges(0, items, policies, **request)
        assert result == case["expected"]
        assert (items, policies) == original
        with Document.from_bytes(doc.to_bytes()) as reopened:
            assert reopened.page_count == 2
            assert reopened.page_png(1, 30)
        before = doc.to_bytes()
        for bad in [None, {}, [{"source_index": -1, "ranges": []}],
                    [{"source_index": 0, "ranges": [[0, 11]]}],
                    [{"source_index": 0, "ranges": [[3, 2]]}],
                    [{"source_index": 0, "ranges": [[0, 4], [3, 5]]}],
                    [{"source_index": 0, "ranges": [[0, float("nan")]]}],
                    [{"source_index": 0, "ranges": [], "extra": True}],
                    [{"source_index": 0, "ranges": []}] * 2]:
            with pytest.raises((OfdError, TypeError, ValueError)):
                doc.add_box_layout_with_content_ranges(0, items, bad, **request)
            assert doc.to_bytes() == before
        with pytest.raises(TypeError):
            doc.add_box_layout_with_content_ranges(0, {}, policies)
        with pytest.raises(IndexError):
            doc.add_box_layout_with_content_ranges(-1, items, policies)
        with pytest.raises(OfdError):
            doc.add_box_layout_with_content_ranges(99, items, policies)
    with pytest.raises(OfdError):
        doc.add_box_layout_with_content_ranges(0, items, policies)


def test_text_measurement_builds_protected_intervals_and_empty_policy_retains_geometric_cuts():
    with Document.new() as doc:
        metrics = doc.measure_flowed_spans(0, [{"text": "A\nB\nC\nD"}], 8, font_size=2, line_height=3)
        policies = [{"source_index": 0, "ranges": [[line["y"], line["y"] + line["height"]]
                     for line in metrics["lines"]]}]
        items = [{"type": "box", "size": [8, metrics["height"]]}]
        result = doc.add_box_layout_with_content_ranges(0, items, policies,
            page_size=[20, 14], margins=[2] * 4)
        assert [p["content_box"][3] for p in result["placements"]] == [9, 3]
    with Document.new() as protected, Document.new() as ordinary:
        options = {"page_size": [20, 14], "margins": [2] * 4}
        assert protected.add_box_layout_with_content_ranges(0, items, [], **options) == ordinary.add_box_layout(0, items, **options)
