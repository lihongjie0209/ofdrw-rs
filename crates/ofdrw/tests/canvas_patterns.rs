use ofdrw::{
    CanvasCommand::*, DrawParamStyle, OfdDocument, PageArea, PatternCell, PatternReflectMethod,
};
use serde_json::Value;
use std::{io::Cursor, path::Path};

const SOURCE: &[u8] = include_bytes!("fixtures/canvas-images-java/source.png");

fn array<const N: usize>(value: &Value) -> [f64; N] {
    assert_eq!(value.as_array().unwrap().len(), N);
    std::array::from_fn(|index| value[index].as_f64().unwrap())
}

#[test]
fn canvas_patterns_match_java_dimensions_matrices_and_writer_pixels() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canvas-pattern-java");
    let reference: Value =
        serde_json::from_str(include_str!("fixtures/canvas-pattern-java/reference.json")).unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let java = OfdDocument::open(root.join(format!("{name}.ofd"))).unwrap();
        let mut doc = OfdDocument::new();
        doc.add_blank_page(40., 30.).unwrap();
        let page = java.pages().unwrap().remove(0);
        doc.set_page_area(
            0,
            PageArea {
                physical_box: page.physical_box.unwrap(),
                application_box: page.application_box,
                content_box: page.content_box,
                bleed_box: page.bleed_box,
            },
        )
        .unwrap();
        let mut pattern = doc
            .create_canvas_pattern(
                0,
                SOURCE,
                "png",
                case["repetition"].as_str().unwrap(),
                case["ppm"].as_f64().unwrap(),
            )
            .unwrap();
        if !case["size"].is_null() {
            let [width, height] = array(&case["size"]);
            pattern.set_image_size(width, height).unwrap();
        }
        if !case["transform"].is_null() {
            pattern.set_transform(array(&case["transform"])).unwrap();
        }
        let style = pattern.as_pattern_style();
        let expected = &case["expected"];
        assert_eq!(
            [style.width, style.height],
            [
                expected["width"].as_f64().unwrap(),
                expected["height"].as_f64().unwrap()
            ],
            "{name}"
        );
        assert_eq!(
            style.reflect,
            match expected["reflect"].as_str().unwrap() {
                "Normal" => PatternReflectMethod::Normal,
                "Column" => PatternReflectMethod::Column,
                "Row" => PatternReflectMethod::Row,
                "RowAndColumn" => PatternReflectMethod::RowAndColumn,
                other => panic!("unknown reflect {other}"),
            }
        );
        match &style.cells[0] {
            PatternCell::TransformedImage {
                resource_id,
                boundary,
                ctm,
            } => {
                assert_eq!(*boundary, array(&expected["image_boundary"]), "{name}");
                assert_eq!(*ctm, array(&expected["image_ctm"]), "{name}");
                assert_eq!(
                    doc.document_multimedia_data(0, resource_id).unwrap(),
                    SOURCE
                );
            }
            other => panic!("unexpected pattern cell {other:?}"),
        }
        assert!(doc.page_images(0).unwrap().is_empty());
        assert!(doc.page_paths(0).unwrap().is_empty());
        let stroke = case["stroke"].as_bool().unwrap_or(false);
        let draw = if stroke {
            DrawParamStyle {
                line_width: Some(1.2),
                stroke_pattern: Some(pattern.into_pattern_style()),
                ..Default::default()
            }
        } else {
            DrawParamStyle {
                fill_pattern: Some(pattern.into_pattern_style()),
                ..Default::default()
            }
        };
        let mut commands = vec![SetDrawParam {
            style: Box::new(draw),
        }];
        if !case["canvas_ctm"].is_null() {
            commands.push(SetTransform {
                matrix: array(&case["canvas_ctm"]),
            });
        }
        if let Some(alpha) = case["alpha"].as_f64() {
            commands.push(SetGlobalAlpha { alpha: Some(alpha) });
        }
        if stroke {
            commands.extend([
                Rect {
                    rect: [2., 2., 18., 12.],
                },
                Stroke,
            ]);
        } else {
            commands.push(FillRect {
                rect: [2., 2., 18., 12.],
            });
        }
        doc.draw_canvas(0, [4., 3., 24., 20.], &commands).unwrap();
        assert!(doc.validate_references().unwrap().is_empty());
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        for dpi in [25.4, 101.6] {
            let png = doc.page_png(0, dpi).unwrap();
            assert!(png == java.page_png(0, dpi).unwrap(), "{name} at {dpi} DPI");
            assert!(
                reopened.page_png(0, dpi).unwrap() == png,
                "round trip: {name}"
            );
        }
        assert!(lopdf::Document::load_mem(&doc.to_pdf().unwrap()).is_ok());
    }
}

#[test]
fn invalid_canvas_pattern_creation_never_changes_document_bytes() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let before = doc.to_bytes().unwrap();
    for repetition in [
        "repeat-x",
        "repeat-y",
        "no-repeat",
        "normal",
        " repeat",
        "repeat ",
        "invalid",
        "\0",
        &"x".repeat(5000),
    ] {
        assert!(doc
            .create_canvas_pattern(0, SOURCE, "png", repetition, 3.78)
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    for ppm in [0., -1., f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 5e-324] {
        assert!(doc
            .create_canvas_pattern(0, SOURCE, "png", "repeat", ppm)
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    for (data, format) in [
        (b"".as_slice(), "png"),
        (b"bad".as_slice(), "png"),
        (SOURCE, "jpeg"),
        (&SOURCE[..33], "png"),
    ] {
        assert!(doc
            .create_canvas_pattern(0, data, format, "repeat", 3.78)
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    assert!(doc
        .create_canvas_pattern(1, SOURCE, "png", "repeat", 3.78)
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn pattern_mutators_preserve_painted_objects_and_reject_invalid_edits_atomically() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let mut pattern = doc
        .create_canvas_pattern(0, SOURCE, "png", "column", 4.)
        .unwrap();
    doc.draw_canvas(
        0,
        [0., 0., 40., 30.],
        &[
            SetDrawParam {
                style: Box::new(DrawParamStyle {
                    fill_pattern: Some(pattern.as_pattern_style().clone()),
                    ..Default::default()
                }),
            },
            FillRect {
                rect: [2., 2., 18., 12.],
            },
        ],
    )
    .unwrap();
    let before = doc.to_bytes().unwrap();
    let saved = pattern.clone();
    for (width, height) in [
        (0., 1.),
        (-1., 1.),
        (1., f64::NAN),
        (1., f64::INFINITY),
        (f64::MAX, 1.),
        (1e-12, 1.),
    ] {
        assert!(pattern.set_image_size(width, height).is_err());
        assert_eq!(pattern, saved);
    }
    assert!(pattern
        .set_transform([1., 0., 0., 1., f64::NAN, 0.])
        .is_err());
    assert_eq!(pattern, saved);
    pattern
        .set_transform([1., 0.2, 0., 1., 2., 3.])
        .unwrap()
        .set_image_size(5., 4.)
        .unwrap();
    assert_eq!(
        pattern.as_pattern_style().ctm,
        Some([1., 0.2, 0., 1., 2., 3.])
    );
    assert_eq!(doc.to_bytes().unwrap(), before);
    assert_eq!(doc.document_multimedia_resources(0).unwrap().len(), 1);
    let mut invalid = pattern.into_pattern_style();
    if let PatternCell::TransformedImage { ctm, .. } = &mut invalid.cells[0] {
        ctm[0] = f64::NAN;
    }
    assert!(doc
        .draw_canvas(
            0,
            [0., 0., 40., 30.],
            &[SetDrawParam {
                style: Box::new(DrawParamStyle {
                    fill_pattern: Some(invalid),
                    ..Default::default()
                })
            }]
        )
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn patterns_use_the_page_owning_document_and_reject_signed_packages() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    assert_eq!(doc.append_document_body().unwrap(), 1);
    doc.append_blank_page(1, 40., 30.).unwrap();
    let root = doc.document_info().unwrap()[0].root.clone();
    let original = doc.entry(&root).unwrap().unwrap().to_vec();
    let pattern = doc
        .create_canvas_pattern(1, SOURCE, "png", "repeat", 4.)
        .unwrap();
    assert!(doc.document_multimedia_resources(0).unwrap().is_empty());
    assert_eq!(doc.document_multimedia_resources(1).unwrap().len(), 1);
    assert_eq!(doc.entry(&root).unwrap().unwrap(), original);
    doc.draw_canvas(
        1,
        [0., 0., 40., 30.],
        &[
            SetDrawParam {
                style: Box::new(DrawParamStyle {
                    fill_pattern: Some(pattern.into_pattern_style()),
                    ..Default::default()
                }),
            },
            FillRect {
                rect: [2., 2., 18., 12.],
            },
        ],
    )
    .unwrap();
    assert!(doc.validate_references().unwrap().is_empty());
    let mut signed = OfdDocument::open(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-sign/src/test/resources/signedFile.ofd"),
    )
    .unwrap();
    let before = signed.to_bytes().unwrap();
    assert!(signed
        .create_canvas_pattern(0, SOURCE, "png", "repeat", 4.)
        .is_err());
    assert_eq!(signed.to_bytes().unwrap(), before);
}

#[test]
fn pattern_helper_preserves_encoded_raster_formats_including_jpeg2000_and_jbig2() {
    let source = image::DynamicImage::ImageRgb8(image::load_from_memory(SOURCE).unwrap().to_rgb8());
    let mut cases = Vec::new();
    for (format, name) in [
        (image::ImageFormat::Png, "png"),
        (image::ImageFormat::Jpeg, "jpeg"),
        (image::ImageFormat::Gif, "gif"),
        (image::ImageFormat::Bmp, "bmp"),
        (image::ImageFormat::Tiff, "tiff"),
    ] {
        let mut encoded = Cursor::new(Vec::new());
        source.write_to(&mut encoded, format).unwrap();
        cases.push((name, encoded.into_inner()));
    }
    cases.push((
        "jpeg2000",
        include_bytes!("fixtures/red-32x32.jp2").to_vec(),
    ));
    let jbig2 = OfdDocument::open(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-layout/src/test/resources/keyword2.ofd"),
    )
    .unwrap();
    cases.push(("jbig2", jbig2.page_image_data(0, 0).unwrap().to_vec()));
    for (format, data) in cases {
        let mut doc = OfdDocument::new();
        doc.add_blank_page(40., 30.).unwrap();
        let mut pattern = doc
            .create_canvas_pattern(0, &data, format, "repeat", 4.)
            .unwrap();
        if let PatternCell::TransformedImage { resource_id, .. } =
            &pattern.as_pattern_style().cells[0]
        {
            assert_eq!(
                doc.document_multimedia_data(0, resource_id).unwrap(),
                data,
                "{format}"
            );
        }
        pattern.set_image_size(4., 3.).unwrap();
        doc.draw_canvas(
            0,
            [0., 0., 40., 30.],
            &[
                SetDrawParam {
                    style: Box::new(DrawParamStyle {
                        fill_pattern: Some(pattern.into_pattern_style()),
                        ..Default::default()
                    }),
                },
                FillRect {
                    rect: [2., 2., 18., 12.],
                },
            ],
        )
        .unwrap();
        assert!(
            doc.page_png(0, 25.4).unwrap().starts_with(b"\x89PNG"),
            "{format}"
        );
        assert!(doc.validate_references().unwrap().is_empty(), "{format}");
    }
}

#[test]
fn explicit_pattern_image_ctm_survives_direct_fill_and_stroke_setters() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let mut style = doc
        .create_canvas_pattern(0, SOURCE, "png", "repeat", 4.)
        .unwrap()
        .into_pattern_style();
    if let PatternCell::TransformedImage { ctm, .. } = &mut style.cells[0] {
        *ctm = [3., 1., 0.5, 2., 0., 0.];
    }
    let ids = doc
        .draw_canvas(
            0,
            [0., 0., 40., 30.],
            &[FillRect {
                rect: [2., 2., 18., 12.],
            }],
        )
        .unwrap();
    doc.set_page_object_pattern_fill(0, &ids[0], &style)
        .unwrap();
    doc.set_page_object_pattern_stroke(0, &ids[0], &style)
        .unwrap();
    let path = doc.pages().unwrap()[0].path.clone();
    let xml = std::str::from_utf8(doc.entry(&path).unwrap().unwrap()).unwrap();
    assert_eq!(xml.matches("CTM=\"3 1 0.5 2 0 0\"").count(), 2);
    assert!(doc.validate_references().unwrap().is_empty());
    let png = doc.page_png(0, 101.6).unwrap();
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert!(reopened.page_png(0, 101.6).unwrap() == png);
    assert!(lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok());
    let before = doc.to_bytes().unwrap();
    if let PatternCell::TransformedImage { ctm, .. } = &mut style.cells[0] {
        ctm[1] = f64::INFINITY;
    }
    assert!(doc
        .set_page_object_pattern_fill(0, &ids[0], &style)
        .is_err());
    assert!(doc
        .set_page_object_pattern_stroke(0, &ids[0], &style)
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}
