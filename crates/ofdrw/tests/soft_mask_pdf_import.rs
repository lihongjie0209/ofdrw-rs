mod common;
use ofdrw::OfdDocument;
fn check(name: &str, source: &[u8], reference: &[u8]) {
    let document = OfdDocument::from_pdf_bytes(source).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(document.pages().unwrap().len(), 1);
    let png = document.page_png(0, 144.).unwrap();
    let actual = image::load_from_memory(&png).unwrap().to_rgb8();
    let expected = image::load_from_memory(reference).unwrap().to_rgb8();
    common::assert_raster_matches(&actual, &expected, name);
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_png(0, 144.).unwrap(), png);
    assert!(reopened.validate_references().unwrap().is_empty());
    let pdf = reopened.to_pdf().unwrap();
    assert!(lopdf::Document::load_mem(&pdf).is_ok());
    if let Some(gs) = std::env::var_os("OFDRW_GHOSTSCRIPT") {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("export.pdf");
        let target = temp.path().join("export.png");
        std::fs::write(&source, pdf).unwrap();
        let result = std::process::Command::new(gs)
            .args([
                "-q",
                "-dSAFER",
                "-dBATCH",
                "-dNOPAUSE",
                "-dUseFastColor=true",
                "-sDEVICE=png16m",
                "-r144",
                "-dGraphicsAlphaBits=1",
            ])
            .arg(format!("-sOutputFile={}", target.display()))
            .arg(source)
            .output()
            .unwrap();
        assert!(result.status.success(), "{name}: {result:?}");
        let exported = image::open(target).unwrap().to_rgb8();
        common::assert_raster_matches(&exported, &expected, &format!("{name} PDF export"));
    }
}
macro_rules! mask_case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_bytes!(concat!("fixtures/soft-mask-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/soft-mask-", $name, ".png")),
            );
        }
    };
}
mask_case!(alpha, "alpha");
mask_case!(luminosity, "luminosity");
mask_case!(alpha_invert, "alpha-invert");
mask_case!(luminosity_bc, "luminosity-bc");
mask_case!(mask_matrix, "mask-matrix");
mask_case!(captured_ctm, "captured-ctm");
mask_case!(q_restore, "q-restore");
mask_case!(none, "none");
mask_case!(repeat, "repeat");
mask_case!(group, "group");
mask_case!(parent_alpha, "parent-alpha");
mask_case!(multiply, "multiply");
mask_case!(stroke, "stroke");
mask_case!(fill_stroke, "fill-stroke");
mask_case!(text, "text");
mask_case!(gradient, "gradient");
mask_case!(lum_nonisolated, "lum-nonisolated");
mask_case!(indirect, "indirect");
mask_case!(image, "image");
mask_case!(inline_image, "inline-image");
mask_case!(ordinary_form, "ordinary-form");
mask_case!(shaded_paint, "shaded-paint");
mask_case!(pattern, "pattern");
mask_case!(pattern_cell_mask, "pattern-cell-mask");
mask_case!(sampled, "sampled");
mask_case!(calculator, "calculator");
mask_case!(stitching, "stitching");
mask_case!(nested_masks, "nested-masks");
mask_case!(resource_scope, "resource-scope");
mask_case!(fill_stroke_alpha, "fill-stroke-alpha");
mask_case!(replace, "replace");
mask_case!(rotate, "rotate");
mask_case!(user_unit, "user-unit");
mask_case!(lum_nonisolated_blend, "lum-nonisolated-blend");
mask_case!(text_fill_stroke_alpha, "text-fill-stroke-alpha");

fn pixel(name: &str, x: u32, y: u32) -> [u8; 3] {
    let source = std::fs::read(format!(
        "{}/tests/fixtures/soft-mask-{name}.pdf",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let document = OfdDocument::from_pdf_bytes(&source).unwrap();
    image::load_from_memory(&document.page_png(0, 144.).unwrap())
        .unwrap()
        .to_rgb8()
        .get_pixel(x * 2, (64 - y) * 2)
        .0
}
fn assert_color(actual: [u8; 3], expected: [u8; 3]) {
    assert!(
        actual
            .into_iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff(b) <= 2),
        "{actual:?} != {expected:?}"
    );
}
#[test]
fn inline_image_uses_fractional_group_alpha() {
    assert_color(pixel("inline-image", 24, 24), [128, 128, 128]);
}
#[test]
fn nested_masks_multiply_group_alpha() {
    assert_color(pixel("nested-masks", 24, 24), [191, 191, 64]);
}
#[test]
fn setting_a_mask_replaces_the_previous_mask() {
    assert_color(pixel("replace", 72, 24), [128, 128, 128]);
}
#[test]
fn combined_fill_stroke_knocks_out_fill_under_translucent_stroke() {
    assert_color(pixel("fill-stroke-alpha", 14, 24), [255, 217, 0]);
}
#[test]
fn image_soft_mask_overrides_graphics_mask_outside_group_shape() {
    let color = pixel("image", 64, 24);
    assert!(
        color[2] > 180 && color[0] < 80 && color[1] < 80,
        "{color:?}"
    );
}

mask_case!(text_clip, "text-clip");

mask_case!(compressed, "compressed");

mask_case!(mask_group_oc, "mask-group-oc");
mask_case!(hidden_mask_content, "hidden-mask-content");
