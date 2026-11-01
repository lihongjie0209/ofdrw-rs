use ofdrw::{OfdDocument, PathStrokeStyle, PathStyle};

fn document(width: f64, ctm: Option<[f64; 6]>, alpha: Option<u8>) -> OfdDocument {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(20., 20.).unwrap();
    let commands = if ctm.is_some() {
        "M 0.25 2 L 2.25 2"
    } else {
        "M 2 5 L 18 5"
    };
    let id = doc
        .add_path_object(
            0,
            commands,
            [0., 0., 20., 20.],
            PathStyle {
                line_width: width,
                alpha,
                ..Default::default()
            },
        )
        .unwrap();
    doc.set_page_object_ctm(0, &id, ctm).unwrap();
    doc
}
fn coverage(doc: &OfdDocument, dpi: f64) -> f64 {
    let image = image::load_from_memory(&doc.page_png(0, dpi).unwrap())
        .unwrap()
        .to_rgb8();
    let x = image.width() / 2;
    (0..image.height())
        .map(|y| 1. - f64::from(image.get_pixel(x, y)[0]) / 255.)
        .sum()
}

#[test]
fn zero_width_paths_remain_one_device_pixel_at_multiple_resolutions_and_transforms() {
    for ctm in [None, Some([8., 0., 0., 3., 0., 0.])] {
        let doc = document(0., ctm, None);
        assert_eq!(
            doc.page_object_stroke_styles(0).unwrap()[0]
                .style
                .line_width,
            0.
        );
        for dpi in [25.4, 50.8, 101.6, 300.] {
            let weight = coverage(&doc, dpi);
            assert!(
                (weight - 1.).abs() < 0.06,
                "{ctm:?}, {dpi} DPI: coverage={weight}"
            );
        }
        let bytes = doc.to_bytes().unwrap();
        let reopened = OfdDocument::from_bytes(&bytes).unwrap();
        assert_eq!(
            doc.page_png(0, 101.6).unwrap(),
            reopened.page_png(0, 101.6).unwrap()
        );
        let svg = doc.page_svg(0).unwrap();
        assert!(svg.contains("ofd-device-hairline-"));
        assert!(svg.contains("vector-effect=\"non-scaling-stroke\""));
    }
}

#[test]
fn ordinary_strokes_retain_physical_width_and_hairline_opacity_is_preserved() {
    let ordinary = document(0.5, Some([8., 0., 0., 3., 0., 0.]), None);
    let weight = coverage(&ordinary, 101.6);
    assert!((weight - 6.).abs() < 0.08, "ordinary coverage={weight}");
    let translucent = document(0., Some([8., 0., 0., 3., 0., 0.]), Some(128));
    assert!((coverage(&translucent, 101.6) - 128. / 255.).abs() < 0.04);
}

#[test]
fn path_stroke_updates_allow_zero_and_reject_negative_values_atomically() {
    let mut doc = document(0.4, None, None);
    let id = doc.page_paths(0).unwrap()[0].id.clone().unwrap();
    doc.set_page_path_stroke_style(
        0,
        &id,
        PathStrokeStyle {
            line_width: 0.,
            ..Default::default()
        },
    )
    .unwrap();
    assert!((coverage(&doc, 101.6) - 1.).abs() < 0.06);
    let before = doc.to_bytes().unwrap();
    assert!(doc
        .set_page_path_stroke_style(
            0,
            &id,
            PathStrokeStyle {
                line_width: -0.1,
                ..Default::default()
            }
        )
        .is_err());
    assert!(doc
        .add_path_object(
            0,
            "M 0 0 L 5 5",
            [0., 0., 10., 10.],
            PathStyle {
                line_width: -0.1,
                ..Default::default()
            }
        )
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn pdf_export_preserves_a_zero_width_operator_and_reimport_keeps_visible_hairlines() {
    let doc = document(0., Some([8., 0., 0., 3., 0., 0.]), None);
    let pdf = doc.to_pdf().unwrap();
    let parsed = lopdf::Document::load_mem(&pdf).unwrap();
    let zero = parsed
        .objects
        .values()
        .filter_map(|value| value.as_stream().ok())
        .any(|stream| {
            let bytes = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            lopdf::content::Content::decode(&bytes).is_ok_and(|content| {
                content.operations.iter().any(|op| {
                    op.operator == "w"
                        && op
                            .operands
                            .first()
                            .is_some_and(|v| v.as_float().is_ok_and(|v| v == 0.))
                })
            })
        });
    assert!(zero, "export omitted the PDF hairline width operator");
    let reopened = OfdDocument::from_pdf_bytes(&pdf).unwrap();
    let weight = coverage(&reopened, 101.6);
    assert!((weight - 1.).abs() < 0.08, "round-trip coverage={weight}");
}

#[test]
fn inherited_draw_parameters_preserve_device_hairlines() {
    let mut doc = document(0.4, None, None);
    let id = doc.page_paths(0).unwrap()[0].id.clone().unwrap();
    let param = doc
        .add_document_draw_param(
            0,
            &ofdrw::DrawParamStyle {
                line_width: Some(0.),
                ..Default::default()
            },
        )
        .unwrap();
    doc.set_page_object_draw_param(0, &id, Some(&param))
        .unwrap();
    let path = doc.pages().unwrap()[0].path.clone();
    let xml = String::from_utf8(doc.entry(&path).unwrap().unwrap().to_vec()).unwrap();
    assert!(xml.contains("LineWidth=\"0.4\""));
    doc.set_entry(&path, xml.replace(" LineWidth=\"0.4\"", "").into_bytes())
        .unwrap();
    for dpi in [25.4, 101.6, 300.] {
        assert!((coverage(&doc, dpi) - 1.).abs() < 0.06);
    }
}

#[test]
fn composite_and_annotation_paths_keep_device_hairlines() {
    for annotation in [false, true] {
        let mut doc = OfdDocument::new();
        doc.add_blank_page(20., 20.).unwrap();
        let style = PathStyle {
            line_width: 0.,
            ..Default::default()
        };
        if annotation {
            doc.add_path_annotation(0, "M 2 5 L 18 5", [0., 0., 20., 20.], style)
                .unwrap();
        } else {
            doc.add_composite_path_object(0, "M 2 5 L 18 5", [0., 0., 20., 20.], style)
                .unwrap();
        }
        for dpi in [25.4, 101.6, 300.] {
            let weight = coverage(&doc, dpi);
            assert!(
                (weight - 1.).abs() < 0.06,
                "annotation={annotation}, dpi={dpi}, coverage={weight}"
            );
        }
    }
}

#[test]
fn stroke_only_path_clip_uses_device_hairline_mask() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(20., 20.).unwrap();
    let id = doc
        .add_path_object(
            0,
            "M 0 0 L 20 0 L 20 20 L 0 20 C",
            [0., 0., 20., 20.],
            PathStyle {
                fill: true,
                stroke: false,
                ..Default::default()
            },
        )
        .unwrap();
    let param = doc
        .add_document_draw_param(
            0,
            &ofdrw::DrawParamStyle {
                line_width: Some(0.),
                ..Default::default()
            },
        )
        .unwrap();
    doc.set_page_object_path_clips(
        0,
        &id,
        &[vec![ofdrw::PathClipArea {
            boundary: [0., 0., 20., 20.],
            commands: "M 2 5 L 18 5".into(),
            area_ctm: None,
            path_ctm: None,
            even_odd: false,
            fill: Some(false),
            stroke: Some(true),
            draw_param: Some(param),
        }]],
        false,
    )
    .unwrap();
    for dpi in [25.4, 101.6, 300.] {
        let weight = coverage(&doc, dpi);
        assert!((weight - 1.).abs() < 0.08, "dpi={dpi}, coverage={weight}");
    }
}

#[test]
fn repeated_pattern_cells_keep_one_device_pixel_per_hairline() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(20., 20.).unwrap();
    let id = doc
        .add_path_object(
            0,
            "M 0 0 L 20 0 L 20 20 L 0 20 C",
            [0., 0., 20., 20.],
            PathStyle {
                fill: true,
                stroke: false,
                ..Default::default()
            },
        )
        .unwrap();
    doc.set_page_object_pattern_fill(
        0,
        &id,
        &ofdrw::PatternStyle {
            width: 5.,
            height: 5.,
            x_step: None,
            y_step: None,
            reflect: ofdrw::PatternReflectMethod::Normal,
            relative_to: ofdrw::PatternRelativeTo::Page,
            ctm: None,
            alpha: None,
            thumbnail: None,
            cells: vec![ofdrw::PatternCell::Path {
                commands: "M 0 2 L 5 2".into(),
                boundary: [0., 0., 5., 5.],
                style: PathStyle {
                    line_width: 0.,
                    ..Default::default()
                },
            }],
        },
    )
    .unwrap();
    for dpi in [25.4, 101.6] {
        let weight = coverage(&doc, dpi);
        assert!((weight - 4.).abs() < 0.12, "dpi={dpi}, coverage={weight}");
    }
}

#[test]
fn dashed_hairlines_preserve_physical_dash_lengths() {
    let mut doc = document(0., None, None);
    let id = doc.page_paths(0).unwrap()[0].id.clone().unwrap();
    doc.set_page_path_stroke_style(
        0,
        &id,
        PathStrokeStyle {
            line_width: 0.,
            dash_pattern: Some([2., 2.]),
            ..Default::default()
        },
    )
    .unwrap();
    for dpi in [25.4, 101.6] {
        let image = image::load_from_memory(&doc.page_png(0, dpi).unwrap())
            .unwrap()
            .to_rgb8();
        for (x_mm, expected) in [(3., 1.), (5., 0.), (7., 1.), (9., 0.)] {
            let x = (x_mm * dpi / 25.4) as u32;
            let weight: f64 = (0..image.height())
                .map(|y| 1. - f64::from(image.get_pixel(x, y)[0]) / 255.)
                .sum();
            assert!(
                (weight - expected).abs() < 0.06,
                "dpi={dpi}, x={x_mm}, coverage={weight}"
            );
        }
    }
}
