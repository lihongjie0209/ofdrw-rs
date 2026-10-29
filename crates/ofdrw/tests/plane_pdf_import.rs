mod common;

use ofdrw::OfdDocument;

const FIXTURES: &[(&[u8], &[u8])] = &[
    (
        include_bytes!("fixtures/plane-calculator-rgb.pdf"),
        include_bytes!("fixtures/plane-calculator-rgb.png"),
    ),
    (
        include_bytes!("fixtures/plane-calculator-step.pdf"),
        include_bytes!("fixtures/plane-calculator-step.png"),
    ),
    (
        include_bytes!("fixtures/plane-component-array.pdf"),
        include_bytes!("fixtures/plane-component-array.png"),
    ),
    (
        include_bytes!("fixtures/plane-sampled-rgb.pdf"),
        include_bytes!("fixtures/plane-sampled-rgb.png"),
    ),
    (
        include_bytes!("fixtures/plane-sampled-packed.pdf"),
        include_bytes!("fixtures/plane-sampled-packed.png"),
    ),
    (
        include_bytes!("fixtures/plane-sampled-cubic.pdf"),
        include_bytes!("fixtures/plane-sampled-cubic.png"),
    ),
    (
        include_bytes!("fixtures/plane-sampled-reversed.pdf"),
        include_bytes!("fixtures/plane-sampled-reversed.png"),
    ),
    (
        include_bytes!("fixtures/plane-calgray.pdf"),
        include_bytes!("fixtures/plane-calgray.png"),
    ),
    (
        include_bytes!("fixtures/plane-transformed.pdf"),
        include_bytes!("fixtures/plane-transformed.png"),
    ),
    (
        include_bytes!("fixtures/plane-bbox-background.pdf"),
        include_bytes!("fixtures/plane-bbox-background.png"),
    ),
    (
        include_bytes!("fixtures/plane-sampled-cubic-small.pdf"),
        include_bytes!("fixtures/plane-sampled-cubic-small.png"),
    ),
    (
        include_bytes!("fixtures/plane-cmyk.pdf"),
        include_bytes!("fixtures/plane-cmyk.png"),
    ),
];

#[test]
fn two_input_shadings_match_independent_rasters_and_round_trip() {
    for (index, (source, reference)) in FIXTURES.iter().enumerate() {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        assert_eq!(document.page_images(0).unwrap().len(), 1);
        let png = document.page_png(0, 72.).unwrap();
        let actual = image::load_from_memory(&png).unwrap().to_rgb8();
        let expected = image::load_from_memory(reference).unwrap().to_rgb8();
        common::assert_raster_matches(&actual, &expected, &format!("function shading {index}"));
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_png(0, 72.).unwrap(), png);
        assert_eq!(
            reopened.page_images(0).unwrap(),
            document.page_images(0).unwrap()
        );
        assert!(reopened.validate_references().unwrap().is_empty());
        assert!(lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok());
    }
}

#[test]
fn plane_shading_keeps_transformed_clips_opacity_and_ignores_direct_sh_background() {
    let document = OfdDocument::from_pdf_bytes(FIXTURES[8].0).unwrap();
    let image = document.page_images(0).unwrap().remove(0);
    assert!(image.ctm.is_some());
    let page = document.pages().unwrap().remove(0);
    let xml = std::str::from_utf8(document.entry(&page.path).unwrap().unwrap()).unwrap();
    assert!(xml.contains("Alpha=\"128\""));
    assert!(xml.contains("BlendMode=\"Multiply\""));
    assert!(xml.contains("Clips"));
    let document = OfdDocument::from_pdf_bytes(FIXTURES[9].0).unwrap();
    let image = image::load_from_memory(&document.page_png(0, 72.).unwrap())
        .unwrap()
        .to_rgb8();
    assert_eq!(image.get_pixel(16, 16).0, [255; 3]);
}
