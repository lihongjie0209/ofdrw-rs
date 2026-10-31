import pytest

from ofdrw_rs import Document


@pytest.mark.parametrize("overlap", [False, True])
def test_native_link_annotation_exports_all_click_actions(overlap):
    expected = ["https://example.org/first", "https://example.org/second"]
    with Document.new() as document:
        document.add_blank_page(30, 20)
        document.add_uri_link_annotation(0, expected[0], [2, 3, 10, 5])
        annotation_id = document.annotations()[0]["id"]
        object_id = document.annotation_objects(0, annotation_id)[0]["id"]
        document.add_annotation_object_action(0, annotation_id, object_id,
                                              {"kind": "uri", "uri": "https://example.org/discard"})
        document.remove_annotation_object_action(0, annotation_id, object_id, 1)
        document.add_annotation_object_action(0, annotation_id, object_id,
                                              {"kind": "uri", "uri": expected[1]})
        document.set_annotation_object_action_regions(0, annotation_id, object_id, 0, [[1, 1, 2, 2]])
        document.set_annotation_object_action_regions(0, annotation_id, object_id, 0, [])
        assert document.annotation_object_actions(0, annotation_id, object_id)[0]["region"] is None
        if overlap:
            document.set_annotation_object_action_regions(0, annotation_id, object_id, 0, [[2, 3, 6, 4]])
            document.set_annotation_object_action_regions(0, annotation_id, object_id, 1, [[5, 3, 6, 4]])
        actions = document.annotation_object_actions(0, annotation_id, object_id)
        assert [action["attributes"]["URI"] for action in actions] == expected
        before = document.to_bytes()
        for operation in [
            lambda: document.add_annotation_object_action(0, annotation_id, object_id, {"kind": "uri", "uri": "x", "event": "PO"}),
            lambda: document.remove_annotation_object_action(0, annotation_id, object_id, 99),
            lambda: document.set_annotation_object_action_regions(0, annotation_id, object_id, 0, [[0, 0, -1, 2]]),
        ]:
            with pytest.raises(Exception):
                operation()
            assert document.to_bytes() == before
        data = document.to_bytes()
    with Document.from_bytes(data) as reopened:
        with Document.from_pdf_bytes(reopened.to_pdf()) as imported:
            annotations = imported.annotations()
            assert sorted(len(item["actions"]) for item in annotations) == ([1, 1, 2] if overlap else [2])
            for annotation in annotations:
                targets = [action["attributes"]["URI"] for action in annotation["actions"]]
                if len(targets) == 2:
                    assert targets == expected
                else:
                    assert targets[0] in expected
            assert imported.validate_references() == []
