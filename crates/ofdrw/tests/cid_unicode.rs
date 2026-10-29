use ofdrw::OfdDocument;

#[test]
fn embedded_encoding_and_registered_collection_recover_text_without_tounicode() {
    let source = include_bytes!("fixtures/type0-embedded-collection-unicode.pdf");
    let document = OfdDocument::from_pdf_bytes(source).unwrap();
    let reference =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/type0-gbk-cmap.pdf")).unwrap();
    assert_eq!(document.page_text(0).unwrap(), ["中文"]);
    assert_eq!(
        document.page_text_objects(0).unwrap(),
        reference.page_text_objects(0).unwrap()
    );
    assert_eq!(
        document.page_png(0, 72.0).unwrap(),
        reference.page_png(0, 72.0).unwrap()
    );
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(
        reopened.page_text_objects(0).unwrap(),
        document.page_text_objects(0).unwrap()
    );
    assert!(reopened.validate_references().unwrap().is_empty());
}
