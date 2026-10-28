use ofdrw::{ColorSpaceSpec, OfdDocument};

#[test]
fn resource_readers_ignore_ofd_nodes_inside_extension_wrappers() {
    let mut document = OfdDocument::new();
    document.add_text_page("namespace", 80.0, 60.0).unwrap();
    let original_fonts = document.fonts().unwrap();
    let original_svg = document.page_svg(0).unwrap();
    let path = "Doc_0/PublicRes.xml";
    let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
    let extension = "<x:Metadata xmlns:x=\"urn:example:extension\"><ofd:Font ID=\"1\"/><ofd:ColorSpace ID=\"900\" Type=\"LAB\"/><ofd:MultiMedia ID=\"901\" Type=\"Image\"/></x:Metadata>";
    document
        .set_entry(
            path,
            source
                .replace("<ofd:Fonts>", &format!("{extension}<ofd:Fonts>"))
                .into_bytes(),
        )
        .unwrap();

    assert_eq!(document.fonts().unwrap(), original_fonts);
    assert!(document.page_color_spaces(0).unwrap().is_empty());
    assert!(document.page_multimedia_resources(0).unwrap().is_empty());
    assert_eq!(document.page_svg(0).unwrap(), original_svg);
    assert_eq!(document.page_text(0).unwrap(), ["namespace"]);

    document
        .set_font_attribute(0, "FontName", Some("NamespaceTest"))
        .unwrap();
    assert_eq!(document.fonts().unwrap()[0].font_name, "NamespaceTest");
    let edited = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
    assert!(edited.contains(extension));

    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.fonts().unwrap()[0].font_name, "NamespaceTest");
    assert!(reopened.page_color_spaces(0).unwrap().is_empty());
    assert!(reopened.page_multimedia_resources(0).unwrap().is_empty());
    let issues = reopened.validate_references().unwrap();
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn resource_edits_target_real_definitions_when_extension_reuses_ids() {
    let mut document = OfdDocument::new();
    document.add_blank_page(40.0, 30.0).unwrap();
    let gray = ColorSpaceSpec {
        color_type: "GRAY".into(),
        bits_per_component: 8,
        profile: None,
        palette: vec![],
    };
    let color_id = document.add_document_color_space(0, &gray).unwrap();
    let media_id = document
        .add_multimedia_resource(0, "Audio", "MP3", b"original audio")
        .unwrap();

    let color_path = "Doc_0/PublicRes.xml";
    let color_xml = std::str::from_utf8(document.entry(color_path).unwrap().unwrap()).unwrap();
    let color_extension = format!(
        "<x:Metadata xmlns:x=\"urn:example:extension\"><ofd:ColorSpace ID=\"{color_id}\" Type=\"LAB\"/></x:Metadata>"
    );
    document
        .set_entry(
            color_path,
            color_xml
                .replace(
                    "<ofd:ColorSpaces>",
                    &format!("{color_extension}<ofd:ColorSpaces>"),
                )
                .into_bytes(),
        )
        .unwrap();

    let media_path = "Doc_0/DocumentRes.xml";
    let media_xml = std::str::from_utf8(document.entry(media_path).unwrap().unwrap()).unwrap();
    let media_extension = format!(
        "<x:Metadata xmlns:x=\"urn:example:extension\"><ofd:MultiMedia ID=\"{media_id}\" Type=\"Audio\"><ofd:MediaFile>missing.mp3</ofd:MediaFile></ofd:MultiMedia></x:Metadata>"
    );
    document
        .set_entry(
            media_path,
            media_xml
                .replace(
                    "<ofd:MultiMedias>",
                    &format!("{media_extension}<ofd:MultiMedias>"),
                )
                .into_bytes(),
        )
        .unwrap();

    assert_eq!(document.page_color_spaces(0).unwrap()[0].id, color_id);
    assert_eq!(
        document.document_multimedia_data(0, &media_id).unwrap(),
        b"original audio"
    );
    document
        .set_document_color_space(
            0,
            &color_id,
            &ColorSpaceSpec {
                palette: vec![vec![128]],
                ..gray
            },
        )
        .unwrap();
    document
        .replace_document_multimedia_data(0, &media_id, b"replacement audio")
        .unwrap();
    let second_media_id = document
        .add_multimedia_resource(0, "Audio", "MP3", b"second audio")
        .unwrap();
    assert_eq!(
        document.page_color_spaces(0).unwrap()[0].palette,
        [vec![128]]
    );
    assert_eq!(
        document.document_multimedia_data(0, &media_id).unwrap(),
        b"replacement audio"
    );
    assert!(
        std::str::from_utf8(document.entry(color_path).unwrap().unwrap())
            .unwrap()
            .contains(&color_extension)
    );
    assert!(
        std::str::from_utf8(document.entry(media_path).unwrap().unwrap())
            .unwrap()
            .contains(&media_extension)
    );
    document
        .remove_document_multimedia_resource(0, &media_id)
        .unwrap();
    assert_eq!(
        document
            .document_multimedia_data(0, &second_media_id)
            .unwrap(),
        b"second audio"
    );
    assert!(
        std::str::from_utf8(document.entry(media_path).unwrap().unwrap())
            .unwrap()
            .contains(&media_extension)
    );
}
