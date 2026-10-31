use ofdrw::OfdDocument;

const EXPECTED: [&str; 6] = [
    "https://example.org/资料/页.html?字=值#段",
    "https://example.org/docs/manual.pdf?view=full",
    "https://example.org/docs/manual.pdf?lang=en#section",
    "https://other.example/a",
    "mailto:user@example.org",
    "https://example.org/docs/manual.pdf?lang=en",
];

fn check(document: &OfdDocument) {
    let annotations = document.annotations().unwrap();
    assert_eq!(annotations.len(), EXPECTED.len());
    for (annotation, expected) in annotations.iter().zip(EXPECTED) {
        assert_eq!(annotation.actions[0].attributes["URI"], expected);
    }
    assert!(document.validate_references().unwrap().is_empty());
}

#[test]
fn catalog_uri_base_survives_ofd_and_pdf_round_trips() {
    let document =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/uri-base-links.pdf")).unwrap();
    check(&document);
    let png = document.page_png(0, 72.).unwrap();
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    check(&reopened);
    assert_eq!(reopened.page_png(0, 72.).unwrap(), png);
    let exported = OfdDocument::from_pdf_bytes(&reopened.to_pdf().unwrap()).unwrap();
    check(&exported);
}
