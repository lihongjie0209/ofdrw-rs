mod common;
use ofdrw::OfdDocument;

fn check(name: &str, source: &[u8], reference: &[u8]) {
    let document =
        OfdDocument::from_pdf_bytes(source).unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(
        document.pages().unwrap().len(),
        1,
        "{name}: temporary page leaked"
    );
    let png = document.page_png(0, 144.).unwrap();
    let actual = image::load_from_memory(&png).unwrap().to_rgb8();
    let expected = image::load_from_memory(reference).unwrap().to_rgb8();
    if matches!(name, "both-zero" | "hidden") {
        assert_eq!(actual, expected, "{name}");
        assert!(document.page_objects(0).unwrap().is_empty());
    } else {
        common::assert_raster_matches(&actual, &expected, name);
    }
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_png(0, 144.).unwrap(), png, "{name}");
    assert!(reopened.validate_references().unwrap().is_empty());
    assert!(lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok());
}
macro_rules! radial_case {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_bytes!(concat!("fixtures/general-radial-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/general-radial-", $name, ".png")),
            );
        }
    };
}
radial_case!(grow, "grow");
radial_case!(shrink, "shrink");
radial_case!(overlap, "overlap");
radial_case!(equal, "equal");
radial_case!(grow_start, "grow-start");
radial_case!(grow_end, "grow-end");
radial_case!(grow_both, "grow-both");
radial_case!(shrink_start, "shrink-start");
radial_case!(shrink_end, "shrink-end");
radial_case!(shrink_both, "shrink-both");
radial_case!(equal_both, "equal-both");
radial_case!(point_start, "point-start");
radial_case!(point_end, "point-end");
radial_case!(both_zero, "both-zero");
radial_case!(tangent, "tangent");
radial_case!(near_tangent, "near-tangent");
radial_case!(affine, "affine");
radial_case!(reflected, "reflected");
radial_case!(clipped, "clipped");
radial_case!(alpha, "alpha");
radial_case!(multiply, "multiply");
radial_case!(pattern_background, "pattern-background");
radial_case!(pattern_affine, "pattern-affine");
radial_case!(pattern_scaled, "pattern-scaled");
radial_case!(pattern_zero_background, "pattern-zero-background");
radial_case!(nested_pattern, "nested-pattern");
radial_case!(sampled, "sampled");
radial_case!(calculator_step, "calculator-step");
radial_case!(component_array, "component-array");
radial_case!(cmyk, "cmyk");
radial_case!(calgray, "calgray");
radial_case!(hidden, "hidden");
radial_case!(form, "form");

#[test]
fn nested_pattern_magnification_preserves_sampling_resolution() {
    use std::io::{Cursor, Read};

    for source in [
        include_bytes!("fixtures/general-radial-grow.pdf").as_slice(),
        include_bytes!("fixtures/general-radial-pattern-scaled.pdf").as_slice(),
        include_bytes!("fixtures/general-radial-nested-pattern.pdf").as_slice(),
    ] {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(document.to_bytes().unwrap())).unwrap();
        let mut sizes = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            if entry.name().ends_with(".png") {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                let image = image::load_from_memory(&bytes).unwrap();
                sizes.push((image.width(), image.height()));
            }
        }
        // All three paints cover the same page in world coordinates. Scaling
        // or nesting their source coordinate systems must not reduce detail.
        assert_eq!(sizes, [(542, 362)]);
    }
}
