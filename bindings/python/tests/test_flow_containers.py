import base64
import copy
import json
from pathlib import Path

import pytest
from ofdrw_rs import Document, OfdError

FIXTURES = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures"


def typography(children, font, shape):
    for child in children:
        if child["type"] == "paragraph":
            child["style"].update(font_id=font, shape=shape)
        elif child["type"] == "table":
            child["table_style"]["font_id"] = font
        elif child["type"] == "container":
            typography(child["container"]["children"], font, shape)
        elif child["type"] == "image":
            child["data"] = base64.b64decode(child.pop("data_base64"))


@pytest.mark.parametrize("shape", [False, True])
def test_nested_mixed_containers_match_reference_and_continue(shape):
    fixture = json.loads((FIXTURES / "flow-containers.json").read_text())
    request = fixture["request"]
    containers = request.pop("containers")
    with Document.new() as doc:
        font = doc.add_embedded_font("DejaVu Sans", (FIXTURES / "dejavu-sans.ttf").read_bytes())
        typography(containers[0]["children"], font, shape)
        before = copy.deepcopy(containers)
        result = doc.add_flow_containers(0, containers, **request)
        assert result == fixture["expected"]
        assert containers == before
        for page, expected in enumerate(fixture["text"]):
            assert doc.page_text(page) == expected
        assert doc.page_images(1)[0]["boundary"] == fixture["image_boundary"]
        with Document.from_bytes(doc.to_bytes()) as reopened:
            assert reopened.page_text(2) == ["R3", "Z"]
            assert reopened.page_png(1, 30)
        extra = {"layout": {"size": [4, 0]}, "children": [{"type": "paragraph",
                 "spans": [{"text": "Q"}], "style": containers[0]["children"][0]["style"]}]}
        assert doc.add_flow_containers(0, [extra], **request, start=result["next"])["pages"] == [2]
        assert doc.page_text(2) == ["R3", "Z", "Q"]


def test_container_errors_are_atomic_and_wrapper_guards_apply():
    root = {"layout": {"size": [12, 0]}, "children": []}
    with Document.new() as doc:
        before = doc.to_bytes()
        for child in [{}, {"type": "unknown"}, {"type": "paragraph", "spans": []},
                      {"type": "spacer", "height": -1}, {"type": "spacer", "extra": True},
                      {"type": "container", "container": {"layout": {"size": [4, 0]}, "children": False}}]:
            with pytest.raises(OfdError):
                doc.add_flow_containers(0, [{**root, "children": [child]}])
            assert doc.to_bytes() == before
        with pytest.raises(ValueError):
            doc.add_flow_containers(0, [{**root, "gap": float("nan")}])
        with pytest.raises(TypeError):
            doc.add_flow_containers(0, {})
        with pytest.raises(IndexError):
            doc.add_flow_containers(-1, [root])
    with pytest.raises(OfdError):
        doc.add_flow_containers(0, [root])
