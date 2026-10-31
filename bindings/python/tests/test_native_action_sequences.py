import pytest

from ofdrw_rs import Document


@pytest.mark.parametrize("overlap", [False, True])
def test_native_click_actions_survive_pdf_export(overlap):
    expected = ["https://example.org/first", "https://example.org/second"]
    with Document.new() as document:
        document.add_blank_page(30, 20)
        object_id = document.add_path_object(0, "M 0 0 L 10 0 L 10 5 C", [2, 3, 10, 5])
        for uri in expected:
            document.add_page_object_uri_action(0, object_id, uri)
        if overlap:
            document.set_page_object_action_regions(0, object_id, 0, [[2, 3, 6, 4]])
            document.set_page_object_action_regions(0, object_id, 1, [[5, 3, 6, 4]])
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
