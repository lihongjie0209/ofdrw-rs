use ofdrw::{CanvasCommand::*, OfdDocument};
use std::io::Cursor;

fn image_bytes() -> Vec<u8> {
    let image = image::RgbaImage::from_fn(6, 4, |x, y| {
        image::Rgba([
            if x < 3 { 255 } else { 0 },
            if y < 2 { 255 } else { 0 },
            80,
            255,
        ])
    });
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .unwrap();
    output.into_inner()
}

fn document() -> OfdDocument {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    doc
}

#[test]
fn images_use_canvas_boundary_and_compose_destination_with_current_transform() {
    let mut doc = document();
    let data = image_bytes();
    let ids = doc
        .draw_canvas(
            0,
            [4., 3., 24., 20.],
            &[
                Translate { offset: [2., 1.] },
                Scale { factors: [2., 3.] },
                SetGlobalAlpha { alpha: Some(0.5) },
                DrawImage {
                    data: data.clone(),
                    format: "png".into(),
                    rect: [1., 2., 6., 4.],
                },
                Save,
                SetTransform {
                    matrix: [1., 0., 0., 1., 0., 0.],
                },
                DrawImageAt {
                    data,
                    format: "png".into(),
                    point: [10., 2.],
                    pixels_per_mm: 2.,
                },
                Restore,
            ],
        )
        .unwrap();
    let images = doc.page_images(0).unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(images[0].boundary, Some([4., 3., 24., 20.]));
    assert_eq!(images[0].ctm, Some([12., 0., 0., 12., 4., 7.]));
    assert_eq!(images[1].ctm, Some([3., 0., 0., 2., 10., 2.]));
    assert_eq!(doc.page_objects(0).unwrap()[0].alpha, 127);
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(
        doc.page_png(0, 101.6).unwrap(),
        reopened.page_png(0, 101.6).unwrap()
    );
}

#[test]
fn cropped_images_truncate_source_units_and_pad_outside_the_original() {
    let data = image_bytes();
    let mut doc = document();
    doc.draw_canvas(
        0,
        [0., 0., 40., 30.],
        &[DrawImageRegion {
            data: data.clone(),
            format: "png".into(),
            source: [-0.9, 0.9, 2.9, 2.],
            rect: [2., 2., 10., 8.],
            pixels_per_mm: 2.,
        }],
    )
    .unwrap();
    let info = &doc.page_images(0).unwrap()[0];
    let cropped = image::load_from_memory(
        doc.entry(info.media_path.as_ref().unwrap())
            .unwrap()
            .unwrap(),
    )
    .unwrap()
    .to_rgba8();
    assert_eq!(cropped.dimensions(), (5, 4));
    assert_eq!(cropped.get_pixel(0, 0).0, [0, 0, 0, 0]);
    let original = image::load_from_memory(&data).unwrap().to_rgba8();
    assert_eq!(cropped.get_pixel(1, 0), original.get_pixel(0, 1));
    assert_eq!(cropped.get_pixel(4, 2), original.get_pixel(3, 3));
    assert_eq!(cropped.get_pixel(4, 3).0, [0, 0, 0, 0]);
}

#[test]
fn image_batches_preserve_paths_and_rollback_resources_after_late_errors() {
    let mut doc = document();
    let before = doc.to_bytes().unwrap();
    let data = image_bytes();
    let image = DrawImage {
        data: data.clone(),
        format: "png".into(),
        rect: [2., 2., 6., 4.],
    };
    for invalid in [
        DrawImage {
            data: vec![0],
            format: "png".into(),
            rect: [2., 2., 6., 4.],
        },
        DrawImageAt {
            data: data.clone(),
            format: "png".into(),
            point: [0., 0.],
            pixels_per_mm: 0.,
        },
        DrawImageRegion {
            data: data.clone(),
            format: "png".into(),
            source: [0., 0., 0.1, 1.],
            rect: [0., 0., 1., 1.],
            pixels_per_mm: 1.,
        },
        DrawImageRegion {
            data: data.clone(),
            format: "png".into(),
            source: [0., 0., 5000., 5000.],
            rect: [0., 0., 1., 1.],
            pixels_per_mm: 1.,
        },
        DrawImage {
            data: data.clone(),
            format: "png".into(),
            rect: [0., 0., f64::INFINITY, 1.],
        },
    ] {
        assert!(doc
            .draw_canvas(0, [0., 0., 40., 30.], &[image.clone(), invalid])
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    let ids = doc
        .draw_canvas(
            0,
            [0., 0., 40., 30.],
            &[
                MoveTo { point: [1., 1.] },
                LineTo { point: [8., 1.] },
                image,
                Stroke,
            ],
        )
        .unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(
        doc.page_paths(0).unwrap()[0].abbreviated_data.as_deref(),
        Some("M 1 1 L 8 1")
    );
    assert_eq!(doc.page_images(0).unwrap().len(), 1);
}

#[test]
fn frozen_canvas_clip_applies_to_images() {
    let mut clipped = document();
    let data = image_bytes();
    clipped
        .draw_canvas(
            0,
            [0., 0., 40., 30.],
            &[
                Rect {
                    rect: [2., 2., 3., 4.],
                },
                Clip { even_odd: false },
                DrawImage {
                    data,
                    format: "png".into(),
                    rect: [2., 2., 6., 4.],
                },
            ],
        )
        .unwrap();
    let png = image::load_from_memory(&clipped.page_png(0, 101.6).unwrap())
        .unwrap()
        .to_rgb8();
    assert_eq!(png.get_pixel(28, 12).0, [255, 255, 255]);
    assert_ne!(png.get_pixel(12, 12).0, [255, 255, 255]);
}

#[test]
fn image_canvas_matches_independent_pinned_java_references() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canvas-images-java");
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("reference.json")).unwrap()).unwrap();
    assert_eq!(
        metadata["java_revision"],
        "84e9eb260ba6fd351ecccf08a01a40305d4af635"
    );
    let data = std::fs::read(root.join("source.png")).unwrap();
    for (name, commands) in [
        (
            "mirrored-x",
            vec![DrawImage {
                data: data.clone(),
                format: "png".into(),
                rect: [14., 2., -6., 4.],
            }],
        ),
        (
            "mirrored-y",
            vec![DrawImage {
                data: data.clone(),
                format: "png".into(),
                rect: [2., 10., 6., -4.],
            }],
        ),
        (
            "zero-width",
            vec![DrawImage {
                data: data.clone(),
                format: "png".into(),
                rect: [2., 2., 0., 4.],
            }],
        ),
        (
            "sized",
            vec![
                Translate { offset: [2., 1.] },
                Scale { factors: [2., 3.] },
                SetGlobalAlpha { alpha: Some(0.5) },
                DrawImage {
                    data: data.clone(),
                    format: "png".into(),
                    rect: [1., 2., 6., 4.],
                },
            ],
        ),
        (
            "native-size",
            vec![DrawImageAt {
                data: data.clone(),
                format: "png".into(),
                point: [10., 2.],
                pixels_per_mm: 2.,
            }],
        ),
        (
            "cropped",
            vec![DrawImageRegion {
                data: data.clone(),
                format: "png".into(),
                source: [-0.9, 0.9, 2.9, 2.],
                rect: [2., 2., 10., 8.],
                pixels_per_mm: 2.,
            }],
        ),
    ] {
        let reference = OfdDocument::open(root.join(format!("{name}.ofd"))).unwrap();
        let page = reference.pages().unwrap().remove(0);
        let mut doc = document();
        doc.set_page_area(
            0,
            ofdrw::PageArea {
                physical_box: page.physical_box.unwrap(),
                application_box: page.application_box,
                content_box: page.content_box,
                bleed_box: page.bleed_box,
            },
        )
        .unwrap();
        doc.draw_canvas(0, [4., 3., 24., 20.], &commands).unwrap();
        let actual = doc.page_images(0).unwrap();
        let expected = reference.page_images(0).unwrap();
        assert_eq!(actual[0].boundary, expected[0].boundary, "{name}");
        assert_eq!(actual[0].ctm, expected[0].ctm, "{name}");
        for dpi in [25.4, 101.6] {
            let actual = image::load_from_memory(&doc.page_png(0, dpi).unwrap())
                .unwrap()
                .to_rgba8();
            let expected = image::load_from_memory(&reference.page_png(0, dpi).unwrap())
                .unwrap()
                .to_rgba8();
            assert!(actual == expected, "{name} differs from Java at {dpi} DPI");
        }
    }
}
