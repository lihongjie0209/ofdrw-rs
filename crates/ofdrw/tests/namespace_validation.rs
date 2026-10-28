use ofdrw::OfdDocument;

#[test]
fn validation_ignores_ofd_graphics_inside_extension_wrappers() {
    let mut document = OfdDocument::new();
    document.add_blank_page(40.0, 30.0).unwrap();
    let path = document.pages().unwrap()[0].path.clone();
    let source = std::str::from_utf8(document.entry(&path).unwrap().unwrap()).unwrap();
    let spoof = "<x:Metadata xmlns:x=\"urn:example:extension\"><ofd:TextObject ID=\"999999\" Boundary=\"0 0 10 10\" Font=\"999999\" Size=\"5\"><ofd:TextCode>X</ofd:TextCode></ofd:TextObject><ofd:FillColor ColorSpace=\"999999\" Value=\"bad\"/></x:Metadata>";
    let updated = source.replace("</ofd:Content>", &format!("{spoof}</ofd:Content>"));
    assert_ne!(updated, source);
    document.set_entry(&path, updated.into_bytes()).unwrap();

    assert!(document.page_text(0).unwrap().is_empty());
    let issues = document.validate_references().unwrap();
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn page_removal_ignores_extension_wrapped_destinations() {
    let mut document = OfdDocument::new();
    document.add_blank_page(40.0, 30.0).unwrap();
    document.append_blank_page(0, 20.0, 10.0).unwrap();
    let pages = document.pages().unwrap();
    let path = pages[1].path.clone();
    let source = std::str::from_utf8(document.entry(&path).unwrap().unwrap()).unwrap();
    let spoof = format!(
        "<x:Metadata xmlns:x=\"urn:example:extension\"><ofd:Dest PageID=\"{}\"/></x:Metadata>",
        pages[0].id
    );
    let updated = source.replace("</ofd:Content>", &format!("{spoof}</ofd:Content>"));
    document.set_entry(&path, updated.into_bytes()).unwrap();

    document.remove_page(0).unwrap();
    assert_eq!(document.pages().unwrap().len(), 1);
    assert!(std::str::from_utf8(document.entry(&path).unwrap().unwrap())
        .unwrap()
        .contains(&spoof));
}
