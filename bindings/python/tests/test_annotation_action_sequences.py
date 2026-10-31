import pytest

from ofdrw_rs import Document


def region(x0, x1):
    return (f'<ofd:Region><ofd:Area Start="{x0} 3"><ofd:Line Point1="{x1} 3"/>'
            f'<ofd:Line Point1="{x1} 7"/><ofd:Line Point1="{x0} 7"/>'
            '<ofd:Close/></ofd:Area></ofd:Region>')


@pytest.mark.parametrize("overlap", [False, True])
def test_native_link_annotation_exports_all_click_actions(overlap):
    expected = ["https://example.org/first", "https://example.org/second"]
    with Document.new() as document:
        document.add_blank_page(30, 20)
        document.add_uri_link_annotation(0, expected[0], [2, 3, 10, 5])
        path = document.annotations()[0]["source_path"]
        xml = document.entry(path).decode("utf-8")
        if overlap:
            xml = xml.replace('<ofd:Action Event="CLICK">', '<ofd:Action Event="CLICK">' + region(2, 8), 1)
        second = (f'<ofd:Action Event="CLICK">{region(5, 11) if overlap else ""}'
                  f'<ofd:URI URI="{expected[1]}"/></ofd:Action>')
        assert "</ofd:Actions>" in xml
        document.set_entry(path, xml.replace("</ofd:Actions>", second + "</ofd:Actions>", 1).encode("utf-8"))
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
