from pathlib import Path

from ofdrw_rs import Document


def test_pdf_catalog_uri_base_survives_round_trips():
    source = Path(__file__).resolve().parents[3] / "crates/ofdrw/tests/fixtures/uri-base-links.pdf"
    expected = [
        "https://example.org/资料/页.html?字=值#段",
        "https://example.org/docs/manual.pdf?view=full",
        "https://example.org/docs/manual.pdf?lang=en#section",
        "https://other.example/a",
        "mailto:user@example.org",
        "https://example.org/docs/manual.pdf?lang=en",
    ]

    def check(document):
        assert [item["actions"][0]["attributes"]["URI"] for item in document.annotations()] == expected
        assert document.validate_references() == []

    with Document.from_pdf_bytes(source.read_bytes()) as document:
        check(document)
        data = document.to_bytes()
    with Document.from_bytes(data) as reopened:
        check(reopened)
        with Document.from_pdf_bytes(reopened.to_pdf()) as exported:
            check(exported)
