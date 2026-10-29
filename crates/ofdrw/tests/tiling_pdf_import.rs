mod common;
use ofdrw::OfdDocument;

fn check(name: &str, source: &[u8], reference: &[u8]) {
    let doc = OfdDocument::from_pdf_bytes(source).unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(doc.pages().unwrap().len(), 1, "{name}: scratch page leaked");
    assert!(
        doc.validate_references().unwrap().is_empty(),
        "{name}: broken references"
    );
    if name == "state-reuse" {
        assert_eq!(
            doc.page_composite_units(0).unwrap().len(),
            1,
            "q/Q should share the same vector cell resource"
        );
    }
    let svg = doc.page_svg(0).unwrap();
    assert!(svg.contains("<pattern"), "{name}: pattern was not retained");
    let png = doc.page_png(0, 288.).unwrap();
    let actual = image::load_from_memory(&png).unwrap().to_rgb8();
    let expected = image::load_from_memory(reference).unwrap().to_rgb8();
    common::assert_raster_matches(&actual, &expected, name);
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_png(0, 288.).unwrap(), png, "{name}");
    assert!(reopened.validate_references().unwrap().is_empty());
    assert!(
        lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok(),
        "{name}: PDF export failed"
    );
}

macro_rules! pattern_case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_bytes!(concat!("fixtures/tiling-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/tiling-", $name, ".png")),
            );
        }
    };
}
pattern_case!(colored, "colored");
pattern_case!(matrix, "matrix");
pattern_case!(ctm_before, "ctm-before");
pattern_case!(ctm_after, "ctm-after");
pattern_case!(uncolored_rgb, "uncolored-rgb");
pattern_case!(uncolored_cmyk, "uncolored-cmyk");
pattern_case!(stroke, "stroke");
pattern_case!(text, "text");
pattern_case!(mask, "mask");
pattern_case!(form, "form");
pattern_case!(nested, "nested");
pattern_case!(bbox_offset, "bbox-offset");
pattern_case!(bbox_clip, "bbox-clip");
pattern_case!(negative_step, "negative-step");
pattern_case!(overlap, "overlap");
pattern_case!(alpha, "alpha");
pattern_case!(rotate, "rotate");
pattern_case!(user_unit, "user-unit");
pattern_case!(cell_image, "cell-image");
pattern_case!(cell_text, "cell-text");
pattern_case!(uncolored_mask, "uncolored-mask");

pattern_case!(state_reuse, "state-reuse");
pattern_case!(fill_stroke, "fill-stroke");

pattern_case!(form_alpha, "form-alpha");
pattern_case!(cell_alpha, "cell-alpha");

pattern_case!(flate, "flate");
