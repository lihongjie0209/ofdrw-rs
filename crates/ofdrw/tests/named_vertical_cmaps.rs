use ofdrw::OfdDocument;

#[test]
fn named_vertical_cjk_maps_preserve_text_positions_and_registered_unicode() {
    for (mapped, registered, identity_codes, text) in [
        (
            include_bytes!("fixtures/type0-gbk-vertical-cmap.pdf").as_slice(),
            include_bytes!("fixtures/type0-gbk-vertical-collection.pdf").as_slice(),
            [4559_u16, 3795],
            "中文",
        ),
        (
            include_bytes!("fixtures/type0-cns-vertical-cmap.pdf").as_slice(),
            include_bytes!("fixtures/type0-cns-vertical-collection.pdf").as_slice(),
            [661, 726],
            "中文",
        ),
        (
            include_bytes!("fixtures/type0-jis-vertical-cmap.pdf").as_slice(),
            include_bytes!("fixtures/type0-jis-vertical-collection.pdf").as_slice(),
            [3284, 3722],
            "日本",
        ),
        (
            include_bytes!("fixtures/type0-ks-vertical-cmap.pdf").as_slice(),
            include_bytes!("fixtures/type0-ks-vertical-collection.pdf").as_slice(),
            [3296, 1204],
            "한국",
        ),
    ] {
        let document = OfdDocument::from_pdf_bytes(mapped).unwrap();
        assert_eq!(document.page_text(0).unwrap(), [text]);
        let objects = document.page_text_objects(0).unwrap();
        let dx: f64 = objects[0].codes[0]
            .delta_x
            .as_deref()
            .unwrap()
            .parse()
            .unwrap();
        let dy: f64 = objects[0].codes[0]
            .delta_y
            .as_deref()
            .unwrap()
            .parse()
            .unwrap();
        assert!((dx - 0.6 * 25.4 / 72.0).abs() < 1e-6);
        assert!((dy - 12.0 * 25.4 / 72.0).abs() < 1e-6);
        let png = document.page_png(0, 72.0).unwrap();
        let registered = OfdDocument::from_pdf_bytes(registered).unwrap();
        assert_eq!(registered.page_text_objects(0).unwrap(), objects);
        assert_eq!(registered.page_png(0, 72.0).unwrap(), png);
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_text_objects(0).unwrap(), objects);
        assert_eq!(reopened.page_png(0, 72.0).unwrap(), png);
        assert!(reopened.validate_references().unwrap().is_empty());

        // Independent Identity-V source with the same original CID selectors.
        let mut pdf = lopdf::Document::load_mem(mapped).unwrap();
        pdf.get_object_mut((5, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Encoding", lopdf::Object::Name(b"Identity-V".to_vec()));
        let map = pdf.get_object_mut((6, 0)).unwrap().as_stream_mut().unwrap();
        let source = format!("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /IdentityUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <ffff>\nendcodespacerange\n2 beginbfchar\n<{:04X}> <{:04X}>\n<{:04X}> <{:04X}>\nendbfchar\nendcmap\nCMapName currentdict /CMap defineresource pop\nend end", identity_codes[0], text.chars().next().unwrap() as u32, identity_codes[1], text.chars().nth(1).unwrap() as u32);
        map.set_plain_content(source.into_bytes());
        pdf.get_object_mut((4, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_plain_content(
                format!(
                    "BT /F1 12 Tf 40 60 Td [<{:04X}> 100 <{:04X}>] TJ ET",
                    identity_codes[0], identity_codes[1]
                )
                .into_bytes(),
            );
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        let identity = OfdDocument::from_pdf_bytes(&bytes).unwrap();
        assert_eq!(identity.page_text_objects(0).unwrap(), objects);
        assert_eq!(identity.page_png(0, 72.0).unwrap(), png);
    }
}

#[test]
fn named_vertical_punctuation_selects_vertical_cid_before_embedded_glyph_lookup() {
    let mut pdf =
        lopdf::Document::load_mem(include_bytes!("fixtures/type0-ligature-glyph.pdf")).unwrap();
    pdf.get_object_mut((5, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Encoding", lopdf::Object::Name(b"UniJIS-UTF16-V".to_vec()));
    let descendant = pdf.get_object_mut((7, 0)).unwrap().as_dict_mut().unwrap();
    let mut info = lopdf::Dictionary::new();
    info.set("Registry", lopdf::Object::string_literal("Adobe"));
    info.set("Ordering", lopdf::Object::string_literal("Japan1"));
    info.set("Supplement", 7);
    descendant.set("CIDSystemInfo", info);
    descendant.set(
        "W",
        lopdf::Object::Array(vec![7887.into(), lopdf::Object::Array(vec![750.into()])]),
    );
    let mut glyph_map = vec![0; (7887 + 1) * 2];
    glyph_map[634 * 2..635 * 2].copy_from_slice(&36_u16.to_be_bytes());
    glyph_map[7887 * 2..7888 * 2].copy_from_slice(&3315_u16.to_be_bytes());
    pdf.get_object_mut((10, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_plain_content(glyph_map);
    let map = pdf.get_object_mut((6, 0)).unwrap().as_stream_mut().unwrap();
    let data = String::from_utf8(map.get_plain_content().unwrap())
        .unwrap()
        .replace("<0001>", "<3001>");
    map.set_plain_content(data.into_bytes());
    pdf.get_object_mut((4, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_plain_content(b"BT /F1 24 Tf 60 88 Td <3001> Tj ET".to_vec());
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    let vertical = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    assert_eq!(vertical.page_text(0).unwrap(), ["fi"]);
    assert_eq!(
        vertical.page_text_objects(0).unwrap()[0].cg_transforms[0].glyphs,
        [3315]
    );

    let mut registered = pdf.clone();
    registered
        .get_object_mut((5, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .remove(b"ToUnicode");
    let mut registered_bytes = Vec::new();
    registered.save_to(&mut registered_bytes).unwrap();
    let registered = OfdDocument::from_pdf_bytes(&registered_bytes).unwrap();
    assert_eq!(registered.page_text(0).unwrap(), ["、"]);
    assert_eq!(
        registered.page_text_objects(0).unwrap()[0].cg_transforms[0].glyphs,
        [3315]
    );
    assert_eq!(
        registered.page_png(0, 72.0).unwrap(),
        vertical.page_png(0, 72.0).unwrap()
    );

    pdf.get_object_mut((5, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Encoding", lopdf::Object::Name(b"UniJIS-UTF16-H".to_vec()));
    bytes.clear();
    pdf.save_to(&mut bytes).unwrap();
    let horizontal = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    assert_eq!(horizontal.page_text(0).unwrap(), ["fi"]);
    assert_eq!(
        horizontal.page_text_objects(0).unwrap()[0].cg_transforms[0].glyphs,
        [36]
    );
    assert_ne!(
        vertical.page_png(0, 72.0).unwrap(),
        horizontal.page_png(0, 72.0).unwrap()
    );
}
