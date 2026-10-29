mod common;

use ofdrw::OfdDocument;

const FIXTURES: &[(&str, &[u8], &[u8])] = &[
    (
        "paths",
        include_bytes!("fixtures/separation-paths.pdf"),
        include_bytes!("fixtures/separation-paths.png"),
    ),
    (
        "cmyk",
        include_bytes!("fixtures/separation-cmyk.pdf"),
        include_bytes!("fixtures/separation-cmyk.png"),
    ),
    (
        "sampled",
        include_bytes!("fixtures/separation-sampled.pdf"),
        include_bytes!("fixtures/separation-sampled.png"),
    ),
    (
        "stitching",
        include_bytes!("fixtures/separation-stitching.pdf"),
        include_bytes!("fixtures/separation-stitching.png"),
    ),
    (
        "calculator",
        include_bytes!("fixtures/separation-calculator.pdf"),
        include_bytes!("fixtures/separation-calculator.png"),
    ),
    (
        "image",
        include_bytes!("fixtures/separation-image.pdf"),
        include_bytes!("fixtures/separation-image.png"),
    ),
    (
        "image16-mask",
        include_bytes!("fixtures/separation-image16-mask.pdf"),
        include_bytes!("fixtures/separation-image16-mask.png"),
    ),
    (
        "jpeg",
        include_bytes!("fixtures/separation-jpeg.pdf"),
        include_bytes!("fixtures/separation-jpeg.png"),
    ),
    (
        "indexed",
        include_bytes!("fixtures/separation-indexed.pdf"),
        include_bytes!("fixtures/separation-indexed.png"),
    ),
    (
        "all",
        include_bytes!("fixtures/separation-all.pdf"),
        include_bytes!("fixtures/separation-all.png"),
    ),
    (
        "none",
        include_bytes!("fixtures/separation-none.pdf"),
        include_bytes!("fixtures/separation-none.png"),
    ),
    (
        "none-clipping",
        include_bytes!("fixtures/separation-none-clipping.pdf"),
        include_bytes!("fixtures/separation-none-clipping.png"),
    ),
    (
        "none-independent",
        include_bytes!("fixtures/separation-none-independent.pdf"),
        include_bytes!("fixtures/separation-none-independent.png"),
    ),
    (
        "none-text-clip",
        include_bytes!("fixtures/separation-none-text-clip.pdf"),
        include_bytes!("fixtures/separation-none-text-clip.png"),
    ),
    (
        "shading",
        include_bytes!("fixtures/separation-shading.pdf"),
        include_bytes!("fixtures/separation-shading.png"),
    ),
    (
        "mesh",
        include_bytes!("fixtures/separation-mesh.pdf"),
        include_bytes!("fixtures/separation-mesh.png"),
    ),
    (
        "patch",
        include_bytes!("fixtures/separation-patch.pdf"),
        include_bytes!("fixtures/separation-patch.png"),
    ),
    (
        "plane",
        include_bytes!("fixtures/separation-plane.pdf"),
        include_bytes!("fixtures/separation-plane.png"),
    ),
];

#[test]
fn separation_paints_match_independent_rasters_and_round_trip() {
    for (name, source, reference) in FIXTURES {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        if *name == "image16-mask" {
            let image = image::load_from_memory(document.page_image_data(0, 0).unwrap())
                .unwrap()
                .to_rgba8();
            assert_eq!(image.dimensions(), (16, 16));
            for y in 0..16 {
                for x in 0..16 {
                    assert_eq!(image.get_pixel(x, y)[3], if x <= 6 { 0 } else { 255 });
                }
            }
        }
        let png = document.page_png(0, 72.).unwrap();
        let actual = image::load_from_memory(&png).unwrap().to_rgb8();
        let expected = image::load_from_memory(reference).unwrap().to_rgb8();
        common::assert_raster_matches(&actual, &expected, name);
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_png(0, 72.).unwrap(), png, "{name}");
        assert!(reopened.validate_references().unwrap().is_empty(), "{name}");
        assert!(
            lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok(),
            "{name}"
        );
    }
}

#[test]
fn none_paints_nothing_but_preserves_text_and_clipping() {
    let source = FIXTURES
        .iter()
        .find(|(name, _, _)| *name == "none")
        .unwrap()
        .1;
    let document = OfdDocument::from_pdf_bytes(source).unwrap();
    assert_eq!(document.page_text(0).unwrap(), ["none"]);
    assert!(document.page_images(0).unwrap().is_empty());
    let image = image::load_from_memory(&document.page_png(0, 72.).unwrap())
        .unwrap()
        .to_rgb8();
    assert!(image.pixels().all(|pixel| pixel.0 == [255, 0, 0]));
    let paints = document.page_object_paints(0).unwrap();
    assert_eq!(
        paints
            .iter()
            .filter(|paint| paint.kind == "TextObject")
            .count(),
        1
    );
    assert!(paints
        .iter()
        .filter(|paint| paint.kind == "TextObject")
        .all(|paint| !paint.fill_enabled && !paint.stroke_enabled));
    let source = FIXTURES
        .iter()
        .find(|(name, _, _)| *name == "none-clipping")
        .unwrap()
        .1;
    let document = OfdDocument::from_pdf_bytes(source).unwrap();
    let image = image::load_from_memory(&document.page_png(0, 72.).unwrap())
        .unwrap()
        .to_rgb8();
    assert_eq!(image.get_pixel(16, 48).0, [0, 255, 0]);
    assert_eq!(image.get_pixel(64, 48).0, [255, 0, 0]);
    let source = FIXTURES
        .iter()
        .find(|(name, _, _)| *name == "none-independent")
        .unwrap()
        .1;
    let document = OfdDocument::from_pdf_bytes(source).unwrap();
    assert_eq!(document.page_images(0).unwrap().len(), 1);
    let image = image::load_from_memory(&document.page_png(0, 72.).unwrap())
        .unwrap()
        .to_rgb8();
    assert_eq!(image.get_pixel(16, 72).0, [0, 255, 0]);
    assert_eq!(image.get_pixel(64, 32).0[1], 255);
    let source = FIXTURES
        .iter()
        .find(|(name, _, _)| *name == "none-text-clip")
        .unwrap()
        .1;
    let document = OfdDocument::from_pdf_bytes(source).unwrap();
    assert_eq!(document.page_text(0).unwrap(), ["fiA", "fi"]);
    assert!(document
        .page_object_clips(0)
        .unwrap()
        .iter()
        .any(|paint| !paint.clips.is_empty()));
}
