use std::collections::HashSet;

use ofdrw::OfdDocument;

const TYPE0: &[u8] = include_bytes!("fixtures/type0-ligature-glyph.pdf");

#[test]
fn imported_ligatures_keep_unicode_text_source_advances_and_original_glyphs() {
    let mut reference_png = None;
    for source in [
        TYPE0,
        include_bytes!("fixtures/truetype-ligature-glyph.pdf").as_slice(),
    ] {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        assert_eq!(document.page_text(0).unwrap(), ["fiA", "fi"]);
        let objects = document.page_text_objects(0).unwrap();
        assert_eq!(objects[0].cg_transforms[0].code_count, 3);
        assert_eq!(objects[0].cg_transforms[0].glyphs, [3315, 36]);
        assert_eq!(objects[1].cg_transforms[0].code_count, 2);
        assert_eq!(objects[1].cg_transforms[0].glyphs, [3315]);
        let advance: f64 = objects[0].codes[0]
            .delta_x
            .as_deref()
            .unwrap()
            .parse()
            .unwrap();
        assert!((advance - 15.6 * 25.4 / 72.0).abs() < 1e-6);
        assert!(
            (objects[1].boundary.unwrap()[0]
                - objects[0].boundary.unwrap()[0]
                - 32.928 * 25.4 / 72.0)
                .abs()
                < 1e-6
        );
        let fonts = document.fonts().unwrap();
        let embedded: Vec<_> = fonts
            .iter()
            .enumerate()
            .filter(|(_, font)| font.embedded_path.is_some())
            .collect();
        assert_eq!(embedded.len(), 1);
        assert_eq!(
            document.font_data(embedded[0].0).unwrap(),
            include_bytes!("fixtures/dejavu-serif.ttf")
        );
        let png = document.page_png(0, 72.0).unwrap();
        if let Some(reference) = &reference_png {
            assert_eq!(&png, reference);
        } else {
            reference_png = Some(png.clone());
        }
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_text_objects(0).unwrap(), objects);
        assert_eq!(reopened.page_png(0, 72.0).unwrap(), png);
        assert!(reopened.validate_references().unwrap().is_empty());
    }
}

#[test]
fn imported_ligature_outlines_match_independent_ghostscript_reference() {
    let document = OfdDocument::from_pdf_bytes(TYPE0).unwrap();
    assert_reference_ink(
        &document.page_png(0, 72.0).unwrap(),
        include_bytes!("fixtures/pdf-ligature-reference.png"),
    );
}

fn assert_reference_ink(actual: &[u8], expected: &[u8]) {
    let actual = image::load_from_memory(actual).unwrap().to_rgb8();
    let expected = image::load_from_memory(expected).unwrap().to_rgb8();
    assert_eq!(actual.dimensions(), expected.dimensions());
    let ink = |image: &image::RgbImage| {
        image
            .enumerate_pixels()
            .filter(|(_, _, pixel)| pixel.0.iter().any(|channel| *channel < 128))
            .map(|(x, y, _)| (x as i32, y as i32))
            .collect::<HashSet<_>>()
    };
    let actual = ink(&actual);
    let expected = ink(&expected);
    assert!(actual.len() > 100 && expected.len() > 100);
    let mut unmatched = 0;
    for (source, target) in [(&actual, &expected), (&expected, &actual)] {
        unmatched += source
            .iter()
            .filter(|(x, y)| {
                !(-1..=1).any(|dx| (-1..=1).any(|dy| target.contains(&(x + dx, y + dy))))
            })
            .count();
    }
    // Independent rasterizers differ in hinting and antialiasing at edges.
    let ratio = unmatched as f64 / (actual.len() + expected.len()) as f64;
    assert!(ratio < 0.03, "unmatched glyph ink: {ratio}");
}

#[test]
fn registered_unicode_variation_sequence_keeps_selector_and_source_zero_glyph() {
    let document =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/type0-variation-glyph.pdf")).unwrap();
    assert_eq!(document.page_text(0).unwrap(), ["0\u{fe00}", "0\u{fe00}"]);
    let objects = document.page_text_objects(0).unwrap();
    for object in &objects {
        assert_eq!(object.cg_transforms[0].code_count, 2);
        assert_eq!(object.cg_transforms[0].glyphs, [19]);
    }
    assert!(
        (objects[1].boundary.unwrap()[0] - objects[0].boundary.unwrap()[0] - 18.0 * 25.4 / 72.0)
            .abs()
            < 1e-6
    );
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text_objects(0).unwrap(), objects);
    assert_eq!(
        reopened.page_png(0, 72.0).unwrap(),
        document.page_png(0, 72.0).unwrap()
    );
    assert!(reopened.validate_references().unwrap().is_empty());
}

#[test]
fn utf16_mapping_spans_count_unescaped_unicode_and_do_not_change_source_outlines() {
    let reference = OfdDocument::from_pdf_bytes(TYPE0).unwrap();
    let mut pdf = lopdf::Document::load_mem(TYPE0).unwrap();
    let map = pdf.get_object_mut((6, 0)).unwrap().as_stream_mut().unwrap();
    let text = String::from_utf8(map.get_plain_content().unwrap())
        .unwrap()
        .replace("<00660069>", "<006600690000003c0026>")
        .replace("<0041>", "<d83dde000041>");
    map.set_plain_content(text.into_bytes());
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    let document = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    assert_eq!(document.page_text(0).unwrap(), ["fi\0<&😀A", "fi\0<&"]);
    let objects = document.page_text_objects(0).unwrap();
    assert_eq!(objects[0].cg_transforms[0].code_count, 8);
    assert_eq!(objects[0].cg_transforms[0].glyphs, [3315, 36]);
    assert_eq!(
        document.page_png(0, 72.0).unwrap(),
        reference.page_png(0, 72.0).unwrap()
    );
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text_objects(0).unwrap(), objects);
}

#[test]
fn cid_glyph_mapping_uses_outlines_even_when_the_font_has_no_unicode_cmap() {
    let reference = OfdDocument::from_pdf_bytes(TYPE0).unwrap();
    let document =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/type0-no-unicode-cmap-glyph.pdf"))
            .unwrap();
    assert_eq!(document.page_text(0).unwrap(), ["fiA", "fi"]);
    assert_eq!(
        document.page_text_objects(0).unwrap(),
        reference.page_text_objects(0).unwrap()
    );
    assert_eq!(
        document.page_png(0, 72.0).unwrap(),
        reference.page_png(0, 72.0).unwrap()
    );
    let fonts = document.fonts().unwrap();
    let index = fonts
        .iter()
        .position(|font| font.embedded_path.is_some())
        .unwrap();
    let bytes = document.font_data(index).unwrap();
    assert!(ttf_parser::Face::parse(bytes, 0)
        .unwrap()
        .tables()
        .cmap
        .is_none());
    assert!(document.validate_references().unwrap().is_empty());
}

#[test]
fn invalid_source_glyph_indices_and_odd_maps_do_not_fall_back_to_unicode_glyphs() {
    for map in [vec![0, 0, 255, 255, 0, 36], vec![0, 0, 0]] {
        let mut pdf = lopdf::Document::load_mem(TYPE0).unwrap();
        pdf.get_object_mut((10, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_plain_content(map);
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        assert!(OfdDocument::from_pdf_bytes(&bytes).is_err());
    }
}

#[test]
fn mapped_glyph_overhangs_expand_the_boundary_and_match_ghostscript() {
    let mut pdf = lopdf::Document::load_mem(TYPE0).unwrap();
    pdf.get_object_mut((10, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_plain_content(vec![0, 0, 0, 77, 0, 36]);
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    let document = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    assert!(document.page_text_objects(0).unwrap()[0].boundary.unwrap()[0] < 10.0 * 25.4 / 72.0);
    assert_reference_ink(
        &document.page_png(0, 72.0).unwrap(),
        include_bytes!("fixtures/pdf-overhang-reference.png"),
    );
}

#[test]
fn adobe_glyph_name_sequences_without_tounicode_keep_source_ligatures() {
    let document =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/truetype-glyph-name-sequence.pdf"))
            .unwrap();
    let reference = OfdDocument::from_pdf_bytes(TYPE0).unwrap();
    assert_eq!(document.page_text(0).unwrap(), ["fiA", "fi"]);
    let objects = document.page_text_objects(0).unwrap();
    assert_eq!(objects[0].cg_transforms[0].glyphs, [3315, 36]);
    assert_eq!(objects[0].cg_transforms[0].code_count, 3);
    assert_eq!(
        document.page_png(0, 72.0).unwrap(),
        reference.page_png(0, 72.0).unwrap()
    );
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text_objects(0).unwrap(), objects);
    assert!(reopened.validate_references().unwrap().is_empty());
}

#[test]
fn glyph_name_expansion_is_bounded_before_font_fallback() {
    let mut pdf =
        lopdf::Document::load_mem(include_bytes!("fixtures/truetype-glyph-name-sequence.pdf"))
            .unwrap();
    let font = pdf.get_object_mut((5, 0)).unwrap().as_dict_mut().unwrap();
    font.get_mut(b"Encoding")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "Differences",
            lopdf::Object::Array(vec![
                65.into(),
                lopdf::Object::Name(b"uni006600690041".to_vec()),
            ]),
        );
    let mut content = b"BT /F1 24 Tf 10 60 Td (".to_vec();
    content.extend(std::iter::repeat_n(b'A', 70_000));
    content.extend_from_slice(b") Tj ET");
    pdf.get_object_mut((4, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_plain_content(content);
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    assert!(
        matches!(OfdDocument::from_pdf_bytes(&bytes), Err(ofdrw::OfdError::Limit(message)) if message.contains("decoded text"))
    );
}
