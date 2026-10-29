mod common;
use ofdrw::OfdDocument;

fn check(name: &str, source: &[u8], reference: &[u8]) {
    let doc = OfdDocument::from_pdf_bytes(source).unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(doc.pages().unwrap().len(), 1, "{name}: scratch page leaked");
    assert!(
        doc.validate_references().unwrap().is_empty(),
        "{name}: invalid references"
    );
    assert!(
        doc.page_svg(0).unwrap().contains("<pattern"),
        "{name}: pattern paint was lost"
    );
    if name == "state-reuse" {
        assert_eq!(doc.page_composite_units(0).unwrap().len(), 1);
    }
    let png = doc.page_png(0, 144.).unwrap();
    let actual = image::load_from_memory(&png).unwrap().to_rgb8();
    let expected = image::load_from_memory(reference).unwrap().to_rgb8();
    if let Some(rgb) = match name {
        "ext-alpha" => Some([255u8, 127, 127]),
        "combined-alpha" => Some([255u8, 191, 191]),
        "background-alpha" => Some([153u8, 229, 166]),
        _ => None,
    } {
        for (sample, target) in actual.get_pixel(16, 64).0.into_iter().zip(rgb) {
            assert!(
                sample.abs_diff(target) <= 2,
                "{name}: opacity formula gave {sample}, expected {target}"
            );
        }
    }
    common::assert_raster_matches(&actual, &expected, name);
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_png(0, 144.).unwrap(), png, "{name}");
    assert!(reopened.validate_references().unwrap().is_empty());
    assert!(lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok());
}

macro_rules! shading_pattern_case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_bytes!(concat!("fixtures/shading-pattern-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/shading-pattern-", $name, ".png")),
            );
        }
    };
}
shading_pattern_case!(axial, "axial");
shading_pattern_case!(radial, "radial");
shading_pattern_case!(affine_axial, "affine-axial");
shading_pattern_case!(affine_radial, "affine-radial");
shading_pattern_case!(reflect, "reflect");
shading_pattern_case!(ctm_before, "ctm-before");
shading_pattern_case!(ctm_after, "ctm-after");
shading_pattern_case!(background, "background");
shading_pattern_case!(bbox_background, "bbox-background");
shading_pattern_case!(ext_alpha, "ext-alpha");
shading_pattern_case!(outer_alpha, "outer-alpha");
shading_pattern_case!(combined_alpha, "combined-alpha");
shading_pattern_case!(background_alpha, "background-alpha");
shading_pattern_case!(stroke, "stroke");
shading_pattern_case!(text, "text");
shading_pattern_case!(mask, "mask");
shading_pattern_case!(form, "form");
shading_pattern_case!(rotate, "rotate");
shading_pattern_case!(user_unit, "user-unit");
shading_pattern_case!(state_reuse, "state-reuse");
shading_pattern_case!(indirect, "indirect");
shading_pattern_case!(nested, "nested");
shading_pattern_case!(irrelevant_gstate, "irrelevant-gstate");
shading_pattern_case!(plane, "plane");
shading_pattern_case!(mesh, "mesh");
shading_pattern_case!(lattice, "lattice");
shading_pattern_case!(coons, "coons");
shading_pattern_case!(tensor, "tensor");
