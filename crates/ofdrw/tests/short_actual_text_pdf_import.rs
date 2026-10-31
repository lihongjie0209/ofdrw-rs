mod common;
use ofdrw::OfdDocument;

fn check(name: &str, source: &[u8], reference: &[u8]) {
    let document = OfdDocument::from_pdf_bytes(source).unwrap();
    assert_eq!(document.page_text(0).unwrap(), ["X"], "{name}");
    let objects = document.page_text_objects(0).unwrap();
    assert_eq!(objects.len(), 1, "{name}");
    assert_eq!(objects[0].cg_transforms[0].code_count, 1, "{name}");
    assert_eq!(objects[0].cg_transforms[0].glyph_count, 3, "{name}");
    assert!(document.page_images(0).unwrap().is_empty(), "{name}");
    assert!(document.page_paths(0).unwrap().is_empty(), "{name}");
    let png = document.page_png(0, 144.).unwrap();
    common::assert_raster_matches(
        &image::load_from_memory(&png).unwrap().to_rgb8(),
        &image::load_from_memory(reference).unwrap().to_rgb8(),
        name,
    );
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_png(0, 144.).unwrap(), png, "{name}");
    assert!(reopened.validate_references().unwrap().is_empty(), "{name}");
    let exported = OfdDocument::from_pdf_bytes(&reopened.to_pdf().unwrap()).unwrap();
    assert_eq!(exported.page_text(0).unwrap().concat(), "X", "{name}");
    assert!(exported.validate_references().unwrap().is_empty(), "{name}");
}

macro_rules! case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_bytes!(concat!("fixtures/actual-text-short-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/actual-text-short-", $name, ".png")),
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
