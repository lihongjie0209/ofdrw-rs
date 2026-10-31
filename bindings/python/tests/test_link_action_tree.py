from pathlib import Path

from ofdrw_rs import Document


def test_pdf_action_tree_keeps_execution_order_and_regions():
    source = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/link-action-tree.pdf"

    def check(document):
        annotations = document.annotations()
        assert len(annotations) == 1
        actions = annotations[0]["actions"]
        assert [action["kind"] for action in actions] == ["URI", "Goto", "URI", "URI", "URI", "URI"]
        assert [action["attributes"]["URI"] for action in actions if action["kind"] == "URI"] == [
            "https://example.org/docs/start", "https://example.org/docs/shared",
            "https://example.org/last", "https://example.org/docs/tail", "https://example.org/docs/shared",
        ]
        assert actions[1]["destination"]["page_id"] == document.pages()[1]["id"]
        assert actions[0]["region"] is not None
        assert all(action["region"] == actions[0]["region"] for action in actions)
        assert document.validate_references() == []

    with Document.from_pdf_bytes(source.read_bytes()) as document:
        check(document)
        data = document.to_bytes()
    with Document.from_bytes(data) as reopened:
        check(reopened)
        with Document.from_pdf_bytes(reopened.to_pdf()) as exported:
            check(exported)
