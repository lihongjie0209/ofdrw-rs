//! Verify semantic text survives the SVG/PDF path, including nonpainting text.
use ofdrw::OfdDocument;
#[path = "common/pdf_text.rs"]
mod pdf_text;

fn check(name: &str, source: &[u8], expected: &str) {
    let mut document = OfdDocument::from_pdf_bytes(source).unwrap();
    // These fixtures deliberately have missing/nonembedded CJK fonts. Make
    // semantic export independent of the runner's system fonts. This does not
    // claim that an unavailable glyph can be rendered as a readable character.
    if expected
        .chars()
        .any(|c| matches!(c, '中' | '文' | '\u{31350}'))
    {
        let font = include_bytes!("fixtures/pdf-text-fallback.ttf");
        let face = ttf_parser::Face::parse(font, 0).unwrap();
        for ch in expected.chars() {
            assert!(face.glyph_index(ch).is_some(), "missing fixture glyph {ch}");
        }
        document
            .add_embedded_font("OFD PDF Text Fixture", font)
            .unwrap();
    }
    assert_eq!(
        document.page_text(0).unwrap().concat(),
        expected,
        "{name} OFD"
    );
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    let bytes = reopened.to_pdf().unwrap();
    let extracted = pdf_text::extract(&bytes);
    // lopdf inserts replacement characters for this fixture's unmapped
    // variation-cluster glyphs; Poppler omits them. Keep both reader results
    // explicit rather than silently filtering arbitrary replacement chars.
    let native_expected = if name == "type0-variation-glyph" {
        "\u{fffd}0\u{fe00}\u{fffd}0\u{fe00}"
    } else {
        expected
    };
    assert_eq!(
        extracted.split_whitespace().collect::<String>(),
        native_expected,
        "{name} PDF: {extracted:?}"
    );
    if let Some(program) = std::env::var_os("OFDRW_PDFTOTEXT") {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("export.pdf");
        std::fs::write(&path, &bytes).unwrap();
        let result = std::process::Command::new(program)
            .args(["-enc", "UTF-8"])
            .arg(&path)
            .arg("-")
            .output()
            .unwrap();
        assert!(result.status.success(), "{name}: {result:?}");
        let extracted = String::from_utf8(result.stdout).unwrap();
        assert_eq!(
            extracted.split_whitespace().collect::<String>(),
            expected,
            "{name} Poppler: {extracted:?}"
        );
    }
}
macro_rules! export_case {
    ($test:ident, $fixture:literal, $text:literal) => {
        #[test]
        fn $test() {
            check(
                $fixture,
                include_bytes!(concat!("fixtures/", $fixture, ".pdf")),
                $text,
            );
        }
    };
}
export_case!(masked_text, "soft-mask-text", "A");
export_case!(masked_fill_stroke, "soft-mask-text-fill-stroke-alpha", "A");
export_case!(masked_group, "soft-mask-group-text", "A");
export_case!(masked_nested_group, "soft-mask-nested-group-text", "AB");
export_case!(masked_reused_form, "soft-mask-group-text-reuse", "AA");
export_case!(masked_inner_mask, "soft-mask-group-text-inner-mask", "A");
export_case!(masked_affine_group, "soft-mask-group-text-transform", "A");
export_case!(truetype_ligatures, "truetype-ligature-glyph", "fiAfi");
export_case!(cid_ligatures, "type0-ligature-glyph", "fiAfi");
export_case!(
    glyph_name_sequences,
    "truetype-glyph-name-sequence",
    "fiAfi"
);
export_case!(
    variation_selectors,
    "type0-variation-glyph",
    "0\u{fe00}0\u{fe00}"
);
export_case!(
    supplementary_unicode,
    "type0-supplementary-collection-unicode",
    "\u{31350}"
);
export_case!(
    identity_unicode,
    "type0-identity-collection-unicode",
    "中文"
);
export_case!(
    embedded_collection_unicode,
    "type0-embedded-collection-unicode",
    "中文"
);
export_case!(outline_unicode, "outline-tounicode", "中文");
export_case!(metadata_unicode, "metadata-tounicode", "中文");
export_case!(rotated_unicode, "rotated-tounicode", "中文");
