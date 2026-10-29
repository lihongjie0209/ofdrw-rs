mod common;
use ofdrw::OfdDocument;

fn check(name: &str, source: &[u8], reference: &[u8]) {
    let mut document =
        OfdDocument::from_pdf_bytes(source).unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(
        document.pages().unwrap().len(),
        1,
        "{name}: scratch page leaked"
    );
    let png = document.page_png(0, 144.).unwrap();
    let actual = image::load_from_memory(&png).unwrap().to_rgb8();
    let expected = image::load_from_memory(reference).unwrap().to_rgb8();
    if name.starts_with("hidden") {
        assert_eq!(actual, expected, "{name}");
        assert!(document.page_objects(0).unwrap().is_empty());
        assert!(document.page_composite_units(0).unwrap().is_empty());
    } else {
        common::assert_raster_matches(&actual, &expected, name);
        assert!(document.page_svg(0).unwrap().contains("<pattern"));
        assert_eq!(document.page_composite_units(0).unwrap().len(), 1);
    }
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_png(0, 144.).unwrap(), png, "{name}");
    assert!(reopened.validate_references().unwrap().is_empty());
    assert!(lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok());
    if name.starts_with("hidden") {
        document.add_blank_page(10., 10.).unwrap();
        assert_eq!(
            document.pages().unwrap()[1].id,
            "4",
            "hidden shading reserved IDs in the original document"
        );
    }
}

macro_rules! affine_case {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_bytes!(concat!("fixtures/affine-shading-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/affine-shading-", $name, ".png")),
            );
        }
    };
}
affine_case!(axial_shear, "axial-shear");
affine_case!(axial_anisotropic, "axial-anisotropic");
affine_case!(axial_clip, "axial-clip");
affine_case!(axial_alpha, "axial-alpha");
affine_case!(axial_background, "axial-background");
affine_case!(axial_form, "axial-form");
affine_case!(axial_rotate, "axial-rotate");
affine_case!(axial_user_unit, "axial-user-unit");
affine_case!(axial_reflect_shear, "axial-reflect-shear");
affine_case!(axial_multiply, "axial-multiply");
affine_case!(radial_shear, "radial-shear");
affine_case!(radial_anisotropic, "radial-anisotropic");
affine_case!(radial_clip, "radial-clip");
affine_case!(radial_alpha, "radial-alpha");
affine_case!(radial_background, "radial-background");
affine_case!(radial_form, "radial-form");
affine_case!(radial_rotate, "radial-rotate");
affine_case!(radial_user_unit, "radial-user-unit");
affine_case!(radial_reflect_shear, "radial-reflect-shear");
affine_case!(radial_multiply, "radial-multiply");
affine_case!(hidden_axial, "hidden-axial");
affine_case!(hidden_radial, "hidden-radial");

affine_case!(axial_small_shear, "axial-small-shear");
affine_case!(radial_small_shear, "radial-small-shear");
