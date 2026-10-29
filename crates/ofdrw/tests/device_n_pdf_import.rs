mod common;
use ofdrw::OfdDocument;

const FIXTURES: &[(&str, &[u8], &[u8])] = &[
    (
        "jpx-five-container",
        include_bytes!("fixtures/devicen-jpx-five-container.pdf"),
        include_bytes!("fixtures/devicen-jpx-five-container.png"),
    ),
    (
        "jpx-four",
        include_bytes!("fixtures/devicen-jpx-four.pdf"),
        include_bytes!("fixtures/devicen-jpx-four.png"),
    ),
    (
        "jpx-three-alpha",
        include_bytes!("fixtures/devicen-jpx-three-alpha.pdf"),
        include_bytes!("fixtures/devicen-jpx-three-alpha.png"),
    ),
    (
        "jpx-five-no-bpc",
        include_bytes!("fixtures/devicen-jpx-five-no-bpc.pdf"),
        include_bytes!("fixtures/devicen-jpx-five-no-bpc.png"),
    ),
    (
        "jpx-two",
        include_bytes!("fixtures/devicen-jpx-two.pdf"),
        include_bytes!("fixtures/devicen-jpx-two.png"),
    ),
    (
        "jpx-five",
        include_bytes!("fixtures/devicen-jpx-five.pdf"),
        include_bytes!("fixtures/devicen-jpx-five.png"),
    ),
    (
        "jpx-thirty-two",
        include_bytes!("fixtures/devicen-jpx-thirty-two.pdf"),
        include_bytes!("fixtures/devicen-jpx-thirty-two.png"),
    ),
    (
        "jpx-five-mask",
        include_bytes!("fixtures/devicen-jpx-five-mask.pdf"),
        include_bytes!("fixtures/devicen-jpx-five-mask.png"),
    ),
    (
        "jpx-five-sixteen-mask",
        include_bytes!("fixtures/devicen-jpx-five-sixteen-mask.pdf"),
        include_bytes!("fixtures/devicen-jpx-five-sixteen-mask.png"),
    ),
    (
        "jpx-five-soft-mask",
        include_bytes!("fixtures/devicen-jpx-five-soft-mask.pdf"),
        include_bytes!("fixtures/devicen-jpx-five-soft-mask.png"),
    ),
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
        if matches!(
            *name,
            "jpx-five-mask" | "jpx-five-sixteen-mask" | "jpx-five-soft-mask" | "jpx-three-alpha"
        ) {
            let image = image::load_from_memory(document.page_image_data(0, 0).unwrap())
                .unwrap()
                .to_rgba8();
            for y in 0..16 {
                for x in 0..16 {
                    let expected = if matches!(*name, "jpx-five-soft-mask" | "jpx-three-alpha") {
                        128
                    } else if x <= 7 && y <= 7 {
                        0
                    } else {
                        255
                    };
                    assert_eq!(image.get_pixel(x, y)[3], expected, "{name}: {x},{y}");
                }
            }
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

#[test]
fn multichannel_jpx_rejects_mismatched_colors_and_excessive_decoded_samples() {
    let original =
        lopdf::Document::load_mem(include_bytes!("fixtures/devicen-jpx-five.pdf")).unwrap();
    let import = |mut pdf: lopdf::Document| {
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        OfdDocument::from_pdf_bytes(&bytes)
    };
    let mut mismatch = original.clone();
    let image = mismatch
        .get_object_mut((6, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap();
    image
        .dict
        .get_mut(b"ColorSpace")
        .unwrap()
        .as_array_mut()
        .unwrap()[1]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(import(mismatch).is_err());
    let mut missing = original.clone();
    missing
        .get_object_mut((6, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .remove(b"ColorSpace");
    assert!(import(missing).is_err());
    let mut oversized = original.clone();
    let image = oversized
        .get_object_mut((6, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap();
    let mut data = image.content.clone();
    assert_eq!(&data[..4], b"\xff\x4f\xff\x51");
    for offset in [8, 12, 24, 28] {
        data[offset..offset + 4].copy_from_slice(&4096_u32.to_be_bytes());
    }
    image.set_content(data);
    image.dict.set("Width", 4096);
    image.dict.set("Height", 4096);
    assert!(matches!(import(oversized), Err(ofdrw::OfdError::Limit(_))));
    let mut truncated = original;
    let image = truncated
        .get_object_mut((6, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap();
    image.set_content(image.content[..image.content.len() / 2].to_vec());
    assert!(import(truncated).is_err());
}

#[test]
fn explicit_device_n_jpx_ignores_container_color_and_redundant_sample_metadata() {
    let plain =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/devicen-jpx-five.pdf")).unwrap();
    let container =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/devicen-jpx-five-container.pdf"))
            .unwrap();
    assert_eq!(
        container.page_image_data(0, 0).unwrap(),
        plain.page_image_data(0, 0).unwrap()
    );
    let mut pdf =
        lopdf::Document::load_mem(include_bytes!("fixtures/devicen-jpx-five.pdf")).unwrap();
    let image = pdf.get_object_mut((6, 0)).unwrap().as_stream_mut().unwrap();
    image.dict.set("BitsPerComponent", 1);
    image.dict.set(
        "Decode",
        (0..5)
            .flat_map(|_| [lopdf::Object::Integer(1), lopdf::Object::Integer(0)])
            .collect::<Vec<_>>(),
    );
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    let changed = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    assert_eq!(
        changed.page_image_data(0, 0).unwrap(),
        plain.page_image_data(0, 0).unwrap()
    );
}
