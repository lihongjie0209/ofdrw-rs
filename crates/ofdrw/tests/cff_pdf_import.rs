use ofdrw::OfdDocument;
use std::collections::HashSet;

#[test]
fn cff_fonts_preserve_unicode_sequences_source_glyphs_and_original_programs() {
    for (source, program, raw, cid) in [
        (
            include_bytes!("fixtures/type1-opentype-cff.pdf").as_slice(),
            include_bytes!("fixtures/test-name-cff.otf").as_slice(),
            false,
            false,
        ),
        (
            include_bytes!("fixtures/type1-raw-cff.pdf").as_slice(),
            include_bytes!("fixtures/test-name.cff").as_slice(),
            true,
            false,
        ),
        (
            include_bytes!("fixtures/type0-opentype-cid-cff.pdf").as_slice(),
            include_bytes!("fixtures/test-cid-cff.otf").as_slice(),
            false,
            true,
        ),
        (
            include_bytes!("fixtures/type0-raw-cid-cff.pdf").as_slice(),
            include_bytes!("fixtures/test-cid.cff").as_slice(),
            true,
            true,
        ),
        (
            include_bytes!("fixtures/type0-name-cff.pdf").as_slice(),
            include_bytes!("fixtures/test-name.cff").as_slice(),
            true,
            false,
        ),
        (
            include_bytes!("fixtures/type0-vertical-cid-cff.pdf").as_slice(),
            include_bytes!("fixtures/test-cid.cff").as_slice(),
            true,
            true,
        ),
    ] {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        assert_eq!(document.page_text(0).unwrap(), ["fiA", "fi"]);
        let objects = document.page_text_objects(0).unwrap();
        assert_eq!(objects[0].cg_transforms[0].glyphs, [2, 1]);
        assert_eq!(objects[0].cg_transforms[0].code_count, 3);
        assert_eq!(objects[1].cg_transforms[0].glyphs, [2]);
        let embedded: Vec<_> = document
            .fonts()
            .unwrap()
            .into_iter()
            .enumerate()
            .filter(|(_, font)| font.embedded_path.is_some())
            .collect();
        assert_eq!(embedded.len(), 1);
        let data = document.font_data(embedded[0].0).unwrap();
        let face = ttf_parser::Face::parse(data, 0).unwrap();
        if raw {
            assert_eq!(
                face.raw_face()
                    .table(ttf_parser::Tag::from_bytes(b"CFF "))
                    .unwrap(),
                program
            );
        } else {
            assert_eq!(data, program);
        }
        if cid {
            assert_eq!(
                face.tables().cff.unwrap().glyph_cid(ttf_parser::GlyphId(2)),
                Some(42)
            );
        }
        let png = document.page_png(0, 72.0).unwrap();
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_text_objects(0).unwrap(), objects);
        assert_eq!(reopened.page_png(0, 72.0).unwrap(), png);
        assert!(reopened.validate_references().unwrap().is_empty());
    }
}

#[test]
fn cff_outlines_match_independent_ghostscript_rasters() {
    for (index, (source, expected)) in [
        (
            include_bytes!("fixtures/type1-opentype-cff.pdf").as_slice(),
            include_bytes!("fixtures/type1-opentype-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type1-raw-cff.pdf").as_slice(),
            include_bytes!("fixtures/type1-raw-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type1-internal-cff.pdf").as_slice(),
            include_bytes!("fixtures/type1-internal-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type0-opentype-cid-cff.pdf").as_slice(),
            include_bytes!("fixtures/type0-opentype-cid-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type0-raw-cid-cff.pdf").as_slice(),
            include_bytes!("fixtures/type0-raw-cid-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type0-name-cff.pdf").as_slice(),
            include_bytes!("fixtures/type0-name-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type0-vertical-cid-cff.pdf").as_slice(),
            include_bytes!("fixtures/type0-vertical-cid-cff.png").as_slice(),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        let actual = image::load_from_memory(&document.page_png(0, 72.0).unwrap())
            .unwrap()
            .to_rgb8();
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
        for (a, b) in [(&actual, &expected), (&expected, &actual)] {
            unmatched += a
                .iter()
                .filter(|(x, y)| {
                    !(-1..=1).any(|dx| (-1..=1).any(|dy| b.contains(&(x + dx, y + dy))))
                })
                .count();
        }
        let ratio = unmatched as f64 / (actual.len() + expected.len()) as f64;
        assert!(ratio < 0.03, "unmatched CFF ink case {index}: {ratio}");
    }
}

#[test]
fn internal_cff_encoding_without_pdf_encoding_or_tounicode_keeps_source_glyphs() {
    let document =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/type1-internal-cff.pdf")).unwrap();
    assert_eq!(document.page_text(0).unwrap(), ["ﬁA", "ﬁ"]);
    let objects = document.page_text_objects(0).unwrap();
    assert_eq!(objects[0].cg_transforms[0].glyphs, [2, 1]);
    assert_eq!(objects[0].cg_transforms[0].code_count, 2);
    let reference =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/type1-raw-cff.pdf")).unwrap();
    assert_eq!(
        document.page_png(0, 72.0).unwrap(),
        reference.page_png(0, 72.0).unwrap()
    );
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text_objects(0).unwrap(), objects);
}
