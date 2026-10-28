use ofdrw::OfdDocument;

#[test]
fn document_body_uses_direct_doc_root_after_extension_wrapper() {
    let mut document = OfdDocument::new();
    document.add_text_page("real page", 80.0, 60.0).unwrap();
    let source = std::str::from_utf8(document.entry("OFD.xml").unwrap().unwrap()).unwrap();
    let spoof = "<x:Metadata xmlns:x=\"urn:example:extension\"><ofd:DocRoot>Missing/Document.xml</ofd:DocRoot></x:Metadata>";
    let updated = source.replacen("<ofd:DocRoot>", &format!("{spoof}<ofd:DocRoot>"), 1);
    assert_ne!(updated, source);
    document.set_entry("OFD.xml", updated.into_bytes()).unwrap();

    assert_eq!(document.page_text(0).unwrap(), ["real page"]);
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(0).unwrap(), ["real page"]);
}

#[test]
fn document_body_rejects_doc_root_only_inside_extension_wrapper() {
    let mut document = OfdDocument::new();
    document.add_blank_page(40.0, 30.0).unwrap();
    let source = std::str::from_utf8(document.entry("OFD.xml").unwrap().unwrap()).unwrap();
    let start = source.find("<ofd:DocRoot>").unwrap();
    let end = source[start..].find("</ofd:DocRoot>").unwrap() + start + "</ofd:DocRoot>".len();
    let spoof = "<x:Metadata xmlns:x=\"urn:example:extension\"><ofd:DocRoot>Doc_0/Document.xml</ofd:DocRoot></x:Metadata>";
    let mut updated = source.to_owned();
    updated.replace_range(start..end, spoof);
    document.set_entry("OFD.xml", updated.into_bytes()).unwrap();

    assert!(document.pages().is_err());
}

#[test]
fn document_body_rejects_multiple_direct_doc_roots() {
    let mut document = OfdDocument::new();
    document.add_blank_page(40.0, 30.0).unwrap();
    let source = std::str::from_utf8(document.entry("OFD.xml").unwrap().unwrap()).unwrap();
    let duplicate = "<ofd:DocRoot>Missing/Document.xml</ofd:DocRoot>";
    let updated = source.replacen("<ofd:DocRoot>", &format!("{duplicate}<ofd:DocRoot>"), 1);
    document.set_entry("OFD.xml", updated.into_bytes()).unwrap();

    assert!(document.pages().is_err());
    assert!(document.validate_references().is_err());
}

#[test]
fn document_readers_reject_ambiguous_direct_sections() {
    let mut original = OfdDocument::new();
    original.add_blank_page(40.0, 30.0).unwrap();
    let path = "Doc_0/Document.xml";
    let source = std::str::from_utf8(original.entry(path).unwrap().unwrap())
        .unwrap()
        .to_owned();
    for (extra, check_area) in [("<ofd:CommonData/>", true), ("<ofd:Pages/>", false)] {
        let mut document = original.clone();
        let updated = source.replace("</ofd:Document>", &format!("{extra}</ofd:Document>"));
        document.set_entry(path, updated.into_bytes()).unwrap();
        assert!(document.pages().is_err(), "{extra}");
        if check_area {
            assert!(document.document_page_area(0).is_err());
        }
    }
}

#[test]
fn document_readers_ignore_ofd_sections_inside_extension_wrappers() {
    let mut document = OfdDocument::new();
    document.add_blank_page(40.0, 30.0).unwrap();
    let path = "Doc_0/Document.xml";
    let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
    let spoof = "<x:Metadata xmlns:x=\"urn:example:extension\"><ofd:CommonData><ofd:MaxUnitID>broken</ofd:MaxUnitID></ofd:CommonData><ofd:Pages><ofd:Page ID=\"999\" BaseLoc=\"Missing.xml\"/></ofd:Pages></x:Metadata>";
    let updated = source.replacen("<ofd:CommonData>", &format!("{spoof}<ofd:CommonData>"), 1);
    assert_ne!(updated, source);
    document.set_entry(path, updated.into_bytes()).unwrap();

    assert_eq!(document.pages().unwrap().len(), 1);
    assert_eq!(
        document.pages().unwrap()[0].physical_box,
        Some([0.0, 0.0, 40.0, 30.0])
    );
    assert_eq!(
        document
            .document_page_area(0)
            .unwrap()
            .unwrap()
            .physical_box,
        [0.0, 0.0, 210.0, 297.0]
    );
    document.append_blank_page(0, 20.0, 10.0).unwrap();
    assert_eq!(document.pages().unwrap().len(), 2);
    assert!(document.validate_references().unwrap().is_empty());
    assert!(std::str::from_utf8(document.entry(path).unwrap().unwrap())
        .unwrap()
        .contains(spoof));
}
