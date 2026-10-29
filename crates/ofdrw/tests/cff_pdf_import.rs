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
            include_bytes!("fixtures/type1-implicit-base-cff.pdf").as_slice(),
            include_bytes!("fixtures/type1-implicit-base-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type1-dingbats-base-cff.pdf").as_slice(),
            include_bytes!("fixtures/type1-dingbats-base-cff.png").as_slice(),
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
        (
            include_bytes!("fixtures/type1-affine-cff.pdf").as_slice(),
            include_bytes!("fixtures/type1-affine-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type1-reflected-cff.pdf").as_slice(),
            include_bytes!("fixtures/type1-reflected-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type1-rotated-cff.pdf").as_slice(),
            include_bytes!("fixtures/type1-rotated-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type1-opentype-affine-cff.pdf").as_slice(),
            include_bytes!("fixtures/type1-opentype-affine-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type0-fd-matrices-cff.pdf").as_slice(),
            include_bytes!("fixtures/type0-fd-matrices-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type0-fd-ranges-cff.pdf").as_slice(),
            include_bytes!("fixtures/type0-fd-ranges-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type0-fd-only-cff.pdf").as_slice(),
            include_bytes!("fixtures/type0-fd-only-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type0-vertical-fd-cff.pdf").as_slice(),
            include_bytes!("fixtures/type0-vertical-fd-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type1-stroked-affine-cff.pdf").as_slice(),
            include_bytes!("fixtures/type1-stroked-affine-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type0-clipped-fd-cff.pdf").as_slice(),
            include_bytes!("fixtures/type0-clipped-fd-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type0-fd-translated-top-cff.pdf").as_slice(),
            include_bytes!("fixtures/type0-fd-translated-top-cff.png").as_slice(),
        ),
        (
            include_bytes!("fixtures/type0-clipped-ligature.pdf").as_slice(),
            include_bytes!("fixtures/type0-clipped-ligature.png").as_slice(),
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
        assert!(
            actual.len() > 100 && expected.len() > 100,
            "ink counts case {index}: {} {}",
            actual.len(),
            expected.len()
        );
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

#[test]
fn implicit_cff_base_applies_differences_without_losing_native_codes() {
    for (source, text) in [
        (
            include_bytes!("fixtures/type1-implicit-base-cff.pdf").as_slice(),
            ["ﬁﬁ", "ﬁ"],
        ),
        (
            include_bytes!("fixtures/type1-dingbats-base-cff.pdf").as_slice(),
            ["✂✂", "✂"],
        ),
    ] {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        assert_eq!(document.page_text(0).unwrap(), text);
        let objects = document.page_text_objects(0).unwrap();
        assert_eq!(objects[0].cg_transforms[0].glyphs, [2, 2]);
        assert_eq!(objects[0].cg_transforms[0].code_count, 2);
        let delta: f64 = objects[0].codes[0]
            .delta_x
            .as_deref()
            .unwrap()
            .parse()
            .unwrap();
        assert!((delta - 15.6 * 25.4 / 72.0).abs() < 1e-6);
        let png = document.page_png(0, 72.0).unwrap();
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_text_objects(0).unwrap(), objects);
        assert_eq!(reopened.page_png(0, 72.0).unwrap(), png);
        assert!(reopened.validate_references().unwrap().is_empty());
    }
}

#[test]
fn affine_cff_matrices_keep_pdf_advances_and_survive_package_round_trips() {
    for source in [
        include_bytes!("fixtures/type1-affine-cff.pdf").as_slice(),
        include_bytes!("fixtures/type1-reflected-cff.pdf").as_slice(),
        include_bytes!("fixtures/type1-rotated-cff.pdf").as_slice(),
        include_bytes!("fixtures/type1-opentype-affine-cff.pdf").as_slice(),
        include_bytes!("fixtures/type0-fd-matrices-cff.pdf").as_slice(),
        include_bytes!("fixtures/type0-fd-ranges-cff.pdf").as_slice(),
        include_bytes!("fixtures/type0-fd-translated-top-cff.pdf").as_slice(),
        include_bytes!("fixtures/type0-fd-only-cff.pdf").as_slice(),
        include_bytes!("fixtures/type0-vertical-fd-cff.pdf").as_slice(),
        include_bytes!("fixtures/type1-stroked-affine-cff.pdf").as_slice(),
        include_bytes!("fixtures/type0-clipped-fd-cff.pdf").as_slice(),
    ] {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        assert_eq!(document.page_text(0).unwrap(), ["fiA", "fi"]);
        let objects = document.page_text_objects(0).unwrap();
        assert_eq!(objects[0].cg_transforms[0].glyphs, [2, 1]);
        let png = document.page_png(0, 72.0).unwrap();
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_text_objects(0).unwrap(), objects);
        assert_eq!(reopened.page_png(0, 72.0).unwrap(), png);
        assert!(reopened.validate_references().unwrap().is_empty());
    }
    let direct =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/type0-fd-matrices-cff.pdf")).unwrap();
    let ranges =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/type0-fd-ranges-cff.pdf")).unwrap();
    assert_eq!(
        direct.page_text_objects(0).unwrap(),
        ranges.page_text_objects(0).unwrap()
    );
    assert_eq!(
        direct.page_png(0, 72.0).unwrap(),
        ranges.page_png(0, 72.0).unwrap()
    );
    let objects = direct.page_text_objects(0).unwrap();
    let dx: f64 = objects[0].codes[0]
        .delta_x
        .as_deref()
        .unwrap()
        .parse()
        .unwrap();
    assert!((dx - 15.6 * 25.4 / 72.0).abs() < 1e-6);
    assert!(
        (objects[1].boundary.unwrap()[0] - objects[0].boundary.unwrap()[0] - 32.928 * 25.4 / 72.0)
            .abs()
            < 1e-6
    );
}

#[test]
fn imported_text_clips_retain_original_glyph_maps_and_emit_direct_svg_shapes() {
    for (source, expected) in [
        (
            include_bytes!("fixtures/type0-clipped-fd-cff.pdf").as_slice(),
            vec![2, 1],
        ),
        (
            include_bytes!("fixtures/type0-clipped-ligature.pdf").as_slice(),
            vec![3315, 36],
        ),
    ] {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        let groups = document.page_object_clips(0).unwrap();
        let group = groups
            .iter()
            .find(|group| group.kind == "PathObject")
            .unwrap();
        assert_eq!(group.clips[0].len(), 2);
        assert_eq!(group.clips[0][0].cg_transforms[0].glyphs, expected);
        assert_eq!(group.clips[0][0].cg_transforms[0].code_count, 3);
        let svg = document.page_svg(0).unwrap();
        let parsed = roxmltree::Document::parse(&svg).unwrap();
        let clip = parsed
            .descendants()
            .find(|node| {
                node.tag_name().name() == "clipPath"
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.starts_with("ofd-clip-"))
            })
            .unwrap();
        assert_eq!(clip.children().filter(|node| node.is_element()).count(), 3);
        assert!(clip
            .children()
            .filter(|node| node.is_element())
            .all(|node| node.tag_name().name() == "path"));
    }
}

#[test]
fn raw_cff_resources_in_existing_ofd_packages_render_their_original_matrices() {
    for source in [
        include_bytes!("fixtures/type1-affine-cff.pdf").as_slice(),
        include_bytes!("fixtures/type0-fd-translated-top-cff.pdf").as_slice(),
    ] {
        let pdf = lopdf::Document::load_mem(source).unwrap();
        let program = pdf
            .get_object((9, 0))
            .unwrap()
            .as_stream()
            .unwrap()
            .get_plain_content()
            .unwrap();
        let mut document = OfdDocument::from_pdf_bytes(source).unwrap();
        let expected = document.page_png(0, 72.0).unwrap();
        let path = document
            .fonts()
            .unwrap()
            .into_iter()
            .find_map(|font| font.embedded_path)
            .unwrap();
        let filename = path.rsplit('/').next().unwrap();
        let raw_filename = filename.replace(".otf", ".cff");
        let raw_path = path.replace(".otf", ".cff");
        let resources = String::from_utf8(
            document
                .entry("Doc_0/PublicRes.xml")
                .unwrap()
                .unwrap()
                .to_vec(),
        )
        .unwrap()
        .replace(filename, &raw_filename);
        document.set_entry(&raw_path, program.clone()).unwrap();
        document
            .set_entry("Doc_0/PublicRes.xml", resources.into_bytes())
            .unwrap();
        assert_eq!(document.page_png(0, 72.0).unwrap(), expected);
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_png(0, 72.0).unwrap(), expected);
        let font_index = reopened
            .fonts()
            .unwrap()
            .iter()
            .position(|font| font.embedded_path.as_deref() == Some(raw_path.as_str()))
            .unwrap();
        assert_eq!(reopened.font_data(font_index).unwrap(), program);
        assert!(reopened.validate_references().unwrap().is_empty());
        let mut missing = reopened.clone();
        let page_path = missing.pages().unwrap()[0].path.clone();
        let xml = String::from_utf8(missing.entry(&page_path).unwrap().unwrap().to_vec())
            .unwrap()
            .replace(
                "<ofd:Glyphs>2 1</ofd:Glyphs>",
                "<ofd:Glyphs>65535 1</ofd:Glyphs>",
            );
        assert!(xml.contains("<ofd:Glyphs>65535 1</ofd:Glyphs>"));
        missing.set_entry(&page_path, xml.into_bytes()).unwrap();
        assert_eq!(missing.page_text(0).unwrap(), ["fiA", "fi"]);
        let svg = missing.page_svg(0).unwrap();
        assert!(!svg.contains("data-ofd-glyph=\"65535\""));
        assert!(svg.contains("data-ofd-glyph=\"1\""));
        assert!(missing.page_png(0, 72.0).unwrap().starts_with(b"\x89PNG"));
    }
}
