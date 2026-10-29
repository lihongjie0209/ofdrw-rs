mod common;
use ofdrw::OfdDocument;

const FIXTURES: &[(&str, &[u8], &[u8])] = &[
    (
        "image-thirty-two",
        include_bytes!("fixtures/devicen-image-thirty-two.pdf"),
        include_bytes!("fixtures/devicen-image-thirty-two.png"),
    ),
    (
        "paths",
        include_bytes!("fixtures/devicen-paths.pdf"),
        include_bytes!("fixtures/devicen-paths.png"),
    ),
    (
        "sampled",
        include_bytes!("fixtures/devicen-sampled.pdf"),
        include_bytes!("fixtures/devicen-sampled.png"),
    ),
    (
        "cubic",
        include_bytes!("fixtures/devicen-cubic.pdf"),
        include_bytes!("fixtures/devicen-cubic.png"),
    ),
    (
        "cmyk",
        include_bytes!("fixtures/devicen-cmyk.pdf"),
        include_bytes!("fixtures/devicen-cmyk.png"),
    ),
    (
        "three-input",
        include_bytes!("fixtures/devicen-three-input.pdf"),
        include_bytes!("fixtures/devicen-three-input.png"),
    ),
    (
        "image",
        include_bytes!("fixtures/devicen-image.pdf"),
        include_bytes!("fixtures/devicen-image.png"),
    ),
    (
        "image-five",
        include_bytes!("fixtures/devicen-image-five.pdf"),
        include_bytes!("fixtures/devicen-image-five.png"),
    ),
    (
        "image16-mask",
        include_bytes!("fixtures/devicen-image16-mask.pdf"),
        include_bytes!("fixtures/devicen-image16-mask.png"),
    ),
    (
        "indexed",
        include_bytes!("fixtures/devicen-indexed.pdf"),
        include_bytes!("fixtures/devicen-indexed.png"),
    ),
    (
        "none",
        include_bytes!("fixtures/devicen-none.pdf"),
        include_bytes!("fixtures/devicen-none.png"),
    ),
    (
        "mixed-none",
        include_bytes!("fixtures/devicen-mixed-none.pdf"),
        include_bytes!("fixtures/devicen-mixed-none.png"),
    ),
    (
        "none-clipping",
        include_bytes!("fixtures/devicen-none-clipping.pdf"),
        include_bytes!("fixtures/devicen-none-clipping.png"),
    ),
    (
        "nchannel",
        include_bytes!("fixtures/devicen-nchannel.pdf"),
        include_bytes!("fixtures/devicen-nchannel.png"),
    ),
    (
        "shading",
        include_bytes!("fixtures/devicen-shading.pdf"),
        include_bytes!("fixtures/devicen-shading.png"),
    ),
    (
        "mesh",
        include_bytes!("fixtures/devicen-mesh.pdf"),
        include_bytes!("fixtures/devicen-mesh.png"),
    ),
    (
        "patch",
        include_bytes!("fixtures/devicen-patch.pdf"),
        include_bytes!("fixtures/devicen-patch.png"),
    ),
    (
        "plane",
        include_bytes!("fixtures/devicen-plane.pdf"),
        include_bytes!("fixtures/devicen-plane.png"),
    ),
];

#[test]
fn device_n_colors_match_independent_rasters_and_round_trip() {
    for (name, source, reference) in FIXTURES {
        let document =
            OfdDocument::from_pdf_bytes(source).unwrap_or_else(|error| panic!("{name}: {error}"));
        let png = document.page_png(0, 72.).unwrap();
        let actual = image::load_from_memory(&png).unwrap().to_rgb8();
        let expected = image::load_from_memory(reference).unwrap().to_rgb8();
        common::assert_raster_matches(&actual, &expected, name);
        if *name == "none" {
            assert_eq!(document.page_text(0).unwrap(), ["none"]);
            assert!(document.page_images(0).unwrap().is_empty());
            assert!(actual.pixels().all(|pixel| pixel.0 == [204; 3]));
        }
        if *name == "image16-mask" {
            let image = image::load_from_memory(document.page_image_data(0, 0).unwrap())
                .unwrap()
                .to_rgba8();
            for y in 0..16 {
                for x in 0..16 {
                    assert_eq!(
                        image.get_pixel(x, y)[3],
                        if x <= 6 && y <= 6 { 0 } else { 255 }
                    );
                }
            }
        }
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_png(0, 72.).unwrap(), png, "{name}");
        assert!(reopened.validate_references().unwrap().is_empty(), "{name}");
        assert!(
            lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok(),
            "{name}"
        );
    }
}
