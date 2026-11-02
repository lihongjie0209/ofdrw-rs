use ofdrw::{CanvasCommand, CanvasCommand::*, DrawParamStyle, OfdDocument, SolidPaintStyle};
use serde_json::Value;
use std::path::Path;

const BOUNDARY: [f64; 4] = [4., 3., 24., 20.];

fn scenario(name: &str) -> Vec<CanvasCommand> {
    match name {
        "path-state" => vec![
            BeginPath,
            MoveTo { point: [1., 2.] },
            LineTo { point: [9., 2.] },
            Save,
            BeginPath,
            Rect {
                rect: [2., 4., 6., 3.],
            },
            StrokeRect {
                rect: [9., 8., 2., 2.],
            },
            Stroke,
            Restore,
            Stroke,
            FillRect {
                rect: [1., 12., 4., 2.],
            },
            Stroke,
        ],
        "deferred-transform" => vec![
            BeginPath,
            MoveTo { point: [1., 1.] },
            LineTo { point: [4., 2.] },
            Translate { offset: [3., 4.] },
            Scale { factors: [2., 3.] },
            Stroke,
            SetTransform {
                matrix: [1., 0., 0., 1., 0., 0.],
            },
            Transform {
                matrix: [0., 1., -1., 0., 15., 1.],
            },
            Stroke,
        ],
        "curves" => vec![
            MoveTo { point: [2., 2.] },
            QuadraticCurveTo {
                control: [4., 0.],
                end: [6., 2.],
            },
            BezierCurveTo {
                control1: [6., 4.],
                control2: [8., 4.],
                end: [8., 2.],
            },
            ArcTo {
                radii: [3., 2.],
                angle: 25.,
                large: true,
                sweep: false,
                end: [12., 8.],
            },
            ClosePath,
            Stroke,
            BeginPath,
            CircleArc {
                center: [10., 10.],
                radius: 3.,
                start_angle: 0.,
                end_angle: 360.,
                sweep: true,
            },
            Stroke,
        ],
        "fill-alpha" => vec![
            SetLineWidth { width: 0.7 },
            SetFillColor {
                color: [0, 128, 255],
            },
            SetStrokeColor { color: [255, 0, 0] },
            SetGlobalAlpha { alpha: Some(0.5) },
            Rect {
                rect: [2., 2., 10., 8.],
            },
            Fill { even_odd: false },
            FillRect {
                rect: [14., 2., 4., 6.],
            },
            Stroke,
        ],
        "frozen-clip" => vec![
            Translate { offset: [2., 1.] },
            Rect {
                rect: [0., 0., 8., 8.],
            },
            Clip { even_odd: false },
            BeginPath,
            SetTransform {
                matrix: [1., 0., 0., 1., 10., 0.],
            },
            FillRect {
                rect: [0., 0., 14., 14.],
            },
        ],
        _ => panic!("unknown Java canvas reference"),
    }
}

#[test]
fn vector_canvas_matches_independent_java_paths_styles_and_reference_pixels() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canvas-java");
    let reference: Value =
        serde_json::from_slice(&std::fs::read(root.join("reference.json")).unwrap()).unwrap();
    assert_eq!(
        reference["java_revision"],
        "84e9eb260ba6fd351ecccf08a01a40305d4af635"
    );
    for case in reference["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let java = OfdDocument::open(root.join(format!("{name}.ofd"))).unwrap();
        let area = java.pages().unwrap().remove(0);
        let mut doc = OfdDocument::new();
        doc.add_blank_page(40., 30.).unwrap();
        doc.set_page_area(
            0,
            ofdrw::PageArea {
                physical_box: area.physical_box.unwrap(),
                application_box: area.application_box,
                content_box: area.content_box,
                bleed_box: area.bleed_box,
            },
        )
        .unwrap();
        let ids = doc.draw_canvas(0, BOUNDARY, &scenario(name)).unwrap();
        let paths = doc.page_paths(0).unwrap();
        let styles = doc.page_object_stroke_styles(0).unwrap();
        let objects = doc.page_objects(0).unwrap();
        let expected = case["objects"].as_array().unwrap();
        assert_eq!(ids.len(), expected.len(), "{name}");
        for (index, expected) in expected.iter().enumerate() {
            let path = &paths[index];
            assert_eq!(path.id.as_deref(), Some(ids[index].as_str()));
            assert_eq!(path.boundary, Some(BOUNDARY), "{name}");
            let matrix: Vec<f64> = expected["ctm"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            assert_eq!(
                path.ctm.unwrap_or([1., 0., 0., 1., 0., 0.]).as_slice(),
                matrix,
                "{name}"
            );
            assert_eq!(path.fill, expected["fill"].as_bool().unwrap(), "{name}");
            assert_eq!(path.stroke, expected["stroke"].as_bool().unwrap(), "{name}");
            assert_eq!(
                styles[index].style.line_width,
                expected["line_width"].as_f64().unwrap(),
                "{name}"
            );
            assert_eq!(
                u64::from(objects[index].alpha),
                expected["alpha"].as_u64().unwrap(),
                "{name}"
            );
            // Normalize numeric tokens because equivalent arc rotations include 0 and 360.
            let actual = path
                .abbreviated_data
                .as_deref()
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>();
            let original = expected["path"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>();
            assert_eq!(actual.len(), original.len(), "{name}");
            let mut arc_argument = None;
            for (a, b) in actual.into_iter().zip(original) {
                if a == "A" {
                    arc_argument = Some(0usize);
                } else if let Some(position) = &mut arc_argument {
                    *position += 1;
                }
                match (a.parse::<f64>(), b.parse::<f64>()) {
                    (Ok(a), Ok(b)) => {
                        let difference = if arc_argument == Some(3) {
                            (a - b) % 360.
                        } else {
                            a - b
                        };
                        assert!(difference.abs() < 1e-9, "{name}: {a} != {b}");
                    }
                    _ => {
                        assert_eq!(a, b, "{name}");
                        if a != "A" {
                            arc_argument = None;
                        }
                    }
                }
            }
        }
        let render_reference = if let Some(file) = case["render_reference"].as_str() {
            assert_eq!(name, "frozen-clip");
            assert!(case["normalization"]
                .as_str()
                .unwrap()
                .contains("TransFlag=true"));
            let repaired = OfdDocument::open(root.join(file)).unwrap();
            assert_ne!(
                java.page_png(0, 101.6).unwrap(),
                repaired.page_png(0, 101.6).unwrap(),
                "raw pinned Java clip encoding must remain visible in the regression"
            );
            repaired
        } else {
            java
        };
        for dpi in [25.4, 101.6] {
            assert_eq!(
                doc.page_png(0, dpi).unwrap(),
                render_reference.page_png(0, dpi).unwrap(),
                "{name} at {dpi} DPI"
            );
        }
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        assert_eq!(
            doc.page_png(0, 101.6).unwrap(),
            reopened.page_png(0, 101.6).unwrap()
        );
        assert!(doc.validate_references().unwrap().is_empty());
    }
}

#[test]
fn restore_preserves_all_drawing_state_including_draw_parameters() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    doc.draw_canvas(
        0,
        BOUNDARY,
        &[
            SetLineWidth { width: 2. },
            SetStrokeColor { color: [255, 0, 0] },
            SetGlobalAlpha { alpha: Some(0.25) },
            Translate { offset: [2., 1.] },
            Rect {
                rect: [0., 0., 8., 8.],
            },
            Clip { even_odd: false },
            Save,
            SetDrawParam {
                style: Box::new(DrawParamStyle {
                    line_width: Some(5.),
                    stroke: Some(SolidPaintStyle {
                        color: [0, 0, 255],
                        alpha: 255,
                    }),
                    ..Default::default()
                }),
            },
            SetGlobalAlpha { alpha: None },
            Scale { factors: [2., 3.] },
            BeginPath,
            Rect {
                rect: [3., 3., 2., 2.],
            },
            Clip { even_odd: true },
            Stroke,
            Restore,
            Stroke,
        ],
    )
    .unwrap();
    let paths = doc.page_paths(0).unwrap();
    assert_eq!(
        paths[1].abbreviated_data.as_deref(),
        Some("M 0 0 L 8 0 L 8 8 L 0 8 C")
    );
    assert_eq!(paths[1].ctm, Some([1., 0., 0., 1., 2., 1.]));
    let styles = doc.page_object_stroke_styles(0).unwrap();
    assert_eq!(
        styles
            .iter()
            .map(|p| p.style.line_width)
            .collect::<Vec<_>>(),
        [5., 2.]
    );
    let objects = doc.page_objects(0).unwrap();
    assert_eq!(
        objects.iter().map(|p| p.alpha).collect::<Vec<_>>(),
        [255, 63]
    );
    let paints = doc.page_object_paints(0).unwrap();
    assert_eq!(
        paints[1]
            .effective_stroke
            .as_ref()
            .unwrap()
            .value
            .as_deref(),
        Some("255 0 0")
    );
    let clips = doc.page_object_clips(0).unwrap();
    assert_eq!(clips.len(), 2);
    assert!(!clips[1].follows_ctm);
}

#[test]
fn canvas_failures_roll_back_all_objects_resources_and_allocated_ids() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let before = doc.to_bytes().unwrap();
    let invalid = [
        MoveTo {
            point: [f64::NAN, 0.],
        },
        LineTo {
            point: [0., f64::INFINITY],
        },
        SetLineWidth { width: -1. },
        SetGlobalAlpha { alpha: Some(1.1) },
        SetGlobalAlpha {
            alpha: Some(f64::NAN),
        },
        Scale {
            factors: [f64::MAX, f64::MAX],
        },
        SetTransform {
            matrix: [1., 0., 0., 1., f64::NAN, 0.],
        },
        Rect {
            rect: [f64::MAX, 0., f64::MAX, 1.],
        },
        ArcTo {
            radii: [-1., 2.],
            angle: 0.,
            large: false,
            sweep: false,
            end: [0., 0.],
        },
        CircleArc {
            center: [0., 0.],
            radius: -1.,
            start_angle: 0.,
            end_angle: 90.,
            sweep: true,
        },
        SetDrawParam {
            style: Box::new(DrawParamStyle {
                dash_sequence: Some(vec![0.]),
                ..Default::default()
            }),
        },
    ];
    for bad in invalid {
        let mut commands = vec![
            Rect {
                rect: [1., 1., 4., 4.],
            },
            Stroke,
            Scale { factors: [2., 2.] },
        ];
        commands.push(bad);
        assert!(doc.draw_canvas(0, BOUNDARY, &commands).is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    let id = doc
        .draw_canvas(
            0,
            BOUNDARY,
            &[
                Rect {
                    rect: [1., 1., 4., 4.],
                },
                Stroke,
            ],
        )
        .unwrap();
    let mut clean = OfdDocument::from_bytes(&before).unwrap();
    assert_eq!(
        id,
        clean
            .draw_canvas(
                0,
                BOUNDARY,
                &[
                    Rect {
                        rect: [1., 1., 4., 4.]
                    },
                    Stroke
                ]
            )
            .unwrap()
    );
}

#[test]
fn canvas_enforces_page_boundary_command_and_state_stack_limits() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let before = doc.to_bytes().unwrap();
    for boundary in [
        [0., 0., 0., 20.],
        [0., 0., 20., -1.],
        [0., 0., f64::INFINITY, 20.],
    ] {
        assert!(doc.draw_canvas(0, boundary, &[]).is_err());
    }
    assert!(doc.draw_canvas(1, BOUNDARY, &[]).is_err());
    assert!(doc
        .draw_canvas(0, BOUNDARY, &vec![BeginPath; 10_001])
        .is_err());
    assert!(doc.draw_canvas(0, BOUNDARY, &vec![Save; 257]).is_err());
    assert!(doc
        .draw_canvas(
            0,
            BOUNDARY,
            &[
                Restore,
                BeginPath,
                ClosePath,
                Stroke,
                Clip { even_odd: false }
            ]
        )
        .unwrap()
        .is_empty());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn canvas_reuses_parameters_and_style_changes_do_not_mutate_prior_objects() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let ids = doc
        .draw_canvas(
            0,
            BOUNDARY,
            &[
                Rect {
                    rect: [0., 0., 4., 4.],
                },
                Stroke,
                Stroke,
                Save,
                SetLineWidth { width: 3. },
                Stroke,
                Restore,
                Stroke,
            ],
        )
        .unwrap();
    let objects = doc.page_objects(0).unwrap();
    assert_eq!(objects[0].draw_param, objects[1].draw_param);
    assert_eq!(objects[0].draw_param, objects[3].draw_param);
    assert_ne!(objects[0].draw_param, objects[2].draw_param);
    assert_eq!(ids.len(), 4);
    assert_eq!(
        doc.page_object_stroke_styles(0)
            .unwrap()
            .iter()
            .map(|p| p.style.line_width)
            .collect::<Vec<_>>(),
        [0.353, 0.353, 3., 0.353]
    );
}

#[test]
fn canvas_bounds_retained_path_memory_and_transform_overflow() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let before = doc.to_bytes().unwrap();
    let large = MoveTo {
        point: [f64::MIN_POSITIVE; 2],
    };
    assert!(doc
        .draw_canvas(0, BOUNDARY, &vec![large.clone(); 10_000])
        .is_err());
    let mut commands = vec![large; 100];
    commands.extend(vec![Save; 256]);
    // The path is small enough by itself, but saved copies exceed the shared budget.
    assert!(doc.draw_canvas(0, BOUNDARY, &commands).is_err());
    assert!(doc
        .draw_canvas(
            0,
            BOUNDARY,
            &[
                SetTransform {
                    matrix: [f64::MAX, 0., 0., 1., 0., 0.]
                },
                Scale { factors: [2., 1.] }
            ]
        )
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn canvas_uses_the_selected_global_page_and_its_own_document_resources() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let first_page = doc.pages().unwrap()[0].path.clone();
    let original = doc.entry(&first_page).unwrap().unwrap().to_vec();
    let second = doc.append_document_body().unwrap();
    doc.append_blank_page(second, 40., 30.).unwrap();
    let ids = doc
        .draw_canvas(
            1,
            BOUNDARY,
            &[
                Rect {
                    rect: [1., 1., 8., 6.],
                },
                Stroke,
            ],
        )
        .unwrap();
    assert_eq!(doc.entry(&first_page).unwrap().unwrap(), original);
    assert!(doc.page_paths(0).unwrap().is_empty());
    assert_eq!(doc.page_paths(1).unwrap()[0].id.as_ref(), Some(&ids[0]));
    assert!(doc.validate_references().unwrap().is_empty());
    assert!(doc.page_svg(1).unwrap().contains("Doc_1/"));
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(
        doc.page_png(1, 101.6).unwrap(),
        reopened.page_png(1, 101.6).unwrap()
    );
}

#[test]
fn canvas_draw_parameters_support_gradient_pattern_paints_and_color_resets() {
    use ofdrw::{
        GradientFillStyle, GradientKind, GradientMapType, GradientSegment, PathStyle, PatternCell,
        PatternReflectMethod, PatternRelativeTo, PatternStyle,
    };
    let gradient = GradientFillStyle {
        kind: GradientKind::Axial,
        start_point: [0., 0.],
        end_point: [8., 0.],
        segments: vec![
            GradientSegment {
                position: Some(0.),
                color: [255, 0, 0],
                alpha: None,
            },
            GradientSegment {
                position: Some(1.),
                color: [0, 0, 255],
                alpha: None,
            },
        ],
        map_type: GradientMapType::Direct,
        map_unit: None,
        extend: 3,
        fill_alpha: Some(200),
    };
    let pattern = PatternStyle {
        width: 2.,
        height: 2.,
        x_step: None,
        y_step: None,
        reflect: PatternReflectMethod::Normal,
        relative_to: PatternRelativeTo::Object,
        ctm: None,
        alpha: None,
        thumbnail: None,
        cells: vec![PatternCell::Path {
            commands: "M 0 0 L 2 0 L 2 2 L 0 2 C".into(),
            boundary: [0., 0., 2., 2.],
            style: PathStyle {
                fill: true,
                stroke: false,
                fill_color: Some([0, 180, 0]),
                ..Default::default()
            },
        }],
    };
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    doc.draw_canvas(
        0,
        BOUNDARY,
        &[
            SetDrawParam {
                style: Box::new(DrawParamStyle {
                    line_width: Some(0.7),
                    fill_gradient: Some(gradient),
                    stroke_pattern: Some(pattern),
                    ..Default::default()
                }),
            },
            Rect {
                rect: [1., 1., 8., 6.],
            },
            Fill { even_odd: false },
            Save,
            SetFillColor {
                color: [120, 40, 200],
            },
            FillRect {
                rect: [11., 1., 8., 6.],
            },
            Restore,
            FillRect {
                rect: [1., 10., 8., 6.],
            },
            SetStrokeColor {
                color: [10, 20, 30],
            },
            StrokeRect {
                rect: [11., 10., 8., 6.],
            },
        ],
    )
    .unwrap();
    let paints = doc.page_object_paints(0).unwrap();
    assert_eq!(
        paints
            .iter()
            .map(|p| p.effective_fill.as_ref().unwrap().kind.as_str())
            .collect::<Vec<_>>(),
        ["Axial", "Solid", "Axial", "Axial"]
    );
    assert_eq!(
        paints
            .iter()
            .map(|p| p.effective_stroke.as_ref().unwrap().kind.as_str())
            .collect::<Vec<_>>(),
        ["Pattern", "Pattern", "Pattern", "Solid"]
    );
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(
        doc.page_png(0, 101.6).unwrap(),
        reopened.page_png(0, 101.6).unwrap()
    );
    assert!(doc.validate_references().unwrap().is_empty());
}

#[test]
fn canvas_rotation_partial_arcs_and_even_odd_fills_render_and_roundtrip() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    doc.draw_canvas(
        0,
        BOUNDARY,
        &[
            LineTo { point: [2., 3.] },
            Rotate { angle: 90. },
            Translate { offset: [2., -10.] },
            CircleArc {
                center: [8., 8.],
                radius: 4.,
                start_angle: 30.,
                end_angle: 160.,
                sweep: false,
            },
            Stroke,
            SetTransform {
                matrix: [1., 0., 0., 1., 0., 0.],
            },
            BeginPath,
            Rect {
                rect: [1., 1., 12., 12.],
            },
            Rect {
                rect: [4., 4., 6., 6.],
            },
            Fill { even_odd: true },
        ],
    )
    .unwrap();
    let paths = doc.page_paths(0).unwrap();
    assert_eq!(paths.len(), 2);
    let matrix = paths[0].ctm.unwrap();
    for (a, b) in matrix.into_iter().zip([0., 1., -1., 0., 10., 2.]) {
        assert!((a - b).abs() < 1e-12);
    }
    assert!(doc.page_svg(0).unwrap().contains("fill-rule=\"evenodd\""));
    let png = doc.page_png(0, 101.6).unwrap();
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(png, reopened.page_png(0, 101.6).unwrap());
}
