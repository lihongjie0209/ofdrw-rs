mod common;
use ofdrw::OfdDocument;

fn check(name: &str, source: &[u8], reference: &[u8]) {
    let document = OfdDocument::from_pdf_bytes(source).unwrap();
    assert!(document.page_text(0).unwrap().is_empty(), "{name}");
    assert!(document.page_text_objects(0).unwrap().is_empty(), "{name}");
    assert!(document.page_images(0).unwrap().is_empty(), "{name}");
    assert!(
        !document.page_composite_objects(0).unwrap().is_empty(),
        "{name}"
    );
    assert!(document.validate_references().unwrap().is_empty(), "{name}");
    // High-resolution color boundaries reduce font-grid-fitting differences.
    let dpi = if name.ends_with("stroke") { 576. } else { 144. };
    let png = document.page_png(0, dpi).unwrap();
    let reference = image::load_from_memory(reference).unwrap().to_rgb8();
    common::assert_raster_matches(
        &image::load_from_memory(&png).unwrap().to_rgb8(),
        &reference,
        name,
    );
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert!(reopened.page_text(0).unwrap().is_empty(), "{name}");
    assert_eq!(reopened.page_png(0, dpi).unwrap(), png, "{name}");
    let pdf = reopened.to_pdf().unwrap();
    let exported = OfdDocument::from_pdf_bytes(&pdf).unwrap();
    assert!(exported.page_text(0).unwrap().is_empty(), "{name}");
    assert!(exported.validate_references().unwrap().is_empty(), "{name}");
    common::assert_raster_matches(
        &image::load_from_memory(&exported.page_png(0, dpi).unwrap())
            .unwrap()
            .to_rgb8(),
        &reference,
        name,
    );
}

macro_rules! case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_bytes!(concat!("fixtures/actual-text-empty-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/actual-text-empty-", $name, ".png")),
            );
        }
    };
}
case!(truetype, "truetype");
case!(affine, "affine");
case!(reflected, "reflected");
case!(cff, "cff");
case!(cff_matrix, "cff-matrix");
case!(cid_cff_matrix, "cid-cff-matrix");

case!(truetype_stroke, "truetype-stroke");
case!(cff_stroke, "cff-stroke");

case!(truetype_dashed_stroke, "truetype-dashed-stroke");
case!(cff_dashed_stroke, "cff-dashed-stroke");

case!(cff_opentype, "cff-opentype");

case!(truetype_matrix_stroke, "truetype-matrix-stroke");
case!(cff_matrix_stroke, "cff-matrix-stroke");

case!(cff_pattern, "cff-pattern");
case!(cff_pattern_matrix_stroke, "cff-pattern-matrix-stroke");
