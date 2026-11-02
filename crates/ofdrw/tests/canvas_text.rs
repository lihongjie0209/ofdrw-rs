use ofdrw::{CanvasCommand::*, CanvasFont, CanvasTextAlign, OfdDocument};
use serde_json::Value;
use std::path::Path;

const FONT: &[u8] = include_bytes!("fixtures/dejavu-sans.ttf");
const BOUNDARY: [f64; 4] = [4., 3., 80., 80.];

#[test]
fn canvas_text_measurement_geometry_and_pixels_match_all_java_directions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canvas-text-java");
    let reference: Value =
        serde_json::from_slice(&std::fs::read(root.join("reference.json")).unwrap()).unwrap();
    assert_eq!(
        reference["java_revision"],
        "84e9eb260ba6fd351ecccf08a01a40305d4af635"
    );
    let java = OfdDocument::open(root.join("directions.ofd")).unwrap();
    let mut doc = OfdDocument::new();
    doc.add_blank_page(90., 90.).unwrap();
    let page = java.pages().unwrap().remove(0);
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
    let font_id = doc
        .add_embedded_font("CanvasTest", FONT)
        .unwrap()
        .to_string();
    let before = doc.to_bytes().unwrap();
    let mut commands = vec![
        Translate { offset: [1., 1.] },
        SetGlobalAlpha { alpha: Some(0.5) },
    ];
    for case in reference["cases"].as_array().unwrap() {
        let rd = case["read_direction"].as_u64().unwrap() as u16;
        let cd = case["char_direction"].as_u64().unwrap() as u16;
        let style = CanvasFont {
            font_id: Some(font_id.clone()),
            size: 4.,
            letter_spacing: 0.25,
            read_direction: rd,
            char_direction: cd,
            align: match case["align"].as_str().unwrap() {
                "center" => CanvasTextAlign::Center,
                "end" => CanvasTextAlign::End,
                "right" => CanvasTextAlign::Right,
                _ => CanvasTextAlign::Start,
            },
            weight: if cd == 90 { 700 } else { 400 },
            italic: rd == 270,
        };
        let metrics = doc.measure_canvas_text(0, "WiA", &style).unwrap();
        assert_eq!(
            metrics.width,
            case["width"].as_f64().unwrap(),
            "rd {rd} cd {cd}"
        );
        assert_eq!(
            serde_json::to_value(metrics.first_offset).unwrap(),
            case["first_offset"]
        );
        assert_eq!(
            serde_json::to_value(metrics.offsets).unwrap(),
            case["offsets"]
        );
        commands.push(SetFont { font: style });
        commands.push(FillText {
            text: "WiA".into(),
            point: [
                12. + f64::from(cd / 90) * 20.,
                12. + f64::from(rd / 90) * 20.,
            ],
        });
    }
    assert_eq!(
        doc.to_bytes().unwrap(),
        before,
        "measurement mutated the document"
    );
    let ids = doc.draw_canvas(0, BOUNDARY, &commands).unwrap();
    assert_eq!(ids.len(), 16);
    let actual = doc.page_text_objects(0).unwrap();
    let expected = java.page_text_objects(0).unwrap();
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.boundary, expected.boundary);
        assert_eq!(actual.size, expected.size);
        assert_eq!(actual.read_direction, expected.read_direction);
        assert_eq!(actual.char_direction, expected.char_direction);
        assert_eq!(actual.weight, expected.weight);
        assert_eq!(actual.italic, expected.italic);
        for (a, b) in actual.codes.iter().zip(expected.codes) {
            assert_eq!(a.text, b.text);
            assert_eq!((a.x, a.y), (b.x, b.y));
            for (a, b) in [
                (a.delta_x.as_ref(), b.delta_x.as_ref()),
                (a.delta_y.as_ref(), b.delta_y.as_ref()),
            ] {
                let numbers = |value: Option<&String>| {
                    value.map(|v| {
                        v.split_whitespace()
                            .map(|v| v.parse::<f64>().unwrap())
                            .collect::<Vec<_>>()
                    })
                };
                assert_eq!(numbers(a), numbers(b));
            }
        }
    }
    for dpi in [25.4, 101.6] {
        let actual = image::load_from_memory(&doc.page_png(0, dpi).unwrap())
            .unwrap()
            .to_rgba8();
        let expected = image::load_from_memory(&java.page_png(0, dpi).unwrap())
            .unwrap()
            .to_rgba8();
        assert!(
            actual == expected,
            "canvas text differs from Java at {dpi} DPI"
        );
    }
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert!(reopened.validate_references().unwrap().is_empty());
    assert_eq!(
        doc.page_png(0, 101.6).unwrap(),
        reopened.page_png(0, 101.6).unwrap()
    );
}

#[test]
fn canvas_text_restores_fonts_tracking_and_paths_and_preserves_unicode_scalars() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(90., 90.).unwrap();
    let font_id = doc
        .add_embedded_font("CanvasTest", FONT)
        .unwrap()
        .to_string();
    let style = CanvasFont {
        font_id: Some(font_id),
        size: 4.,
        letter_spacing: 0.5,
        ..Default::default()
    };
    let metrics = doc.measure_canvas_text(0, "A😀B", &style).unwrap();
    assert_eq!(metrics.offsets.len(), 2);
    let ids = doc
        .draw_canvas(
            0,
            BOUNDARY,
            &[
                SetFont {
                    font: style.clone(),
                },
                MoveTo { point: [1., 1.] },
                LineTo { point: [8., 1.] },
                Save,
                SetFont {
                    font: CanvasFont {
                        size: 2.,
                        letter_spacing: 0.,
                        ..style.clone()
                    },
                },
                FillText {
                    text: "A😀B".into(),
                    point: [2., 8.],
                },
                Restore,
                FillText {
                    text: "A😀B".into(),
                    point: [2., 16.],
                },
                Stroke,
            ],
        )
        .unwrap();
    assert_eq!(ids.len(), 3);
    let objects = doc.page_text_objects(0).unwrap();
    assert_eq!(objects[0].size, Some(2.));
    assert_eq!(objects[1].size, Some(4.));
    assert_eq!(objects[1].codes[0].text, "A😀B");
    let deltas: Vec<f64> = objects[1].codes[0]
        .delta_x
        .as_ref()
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(deltas.len(), metrics.offsets.len());
    for (actual, expected) in deltas.iter().zip(metrics.offsets) {
        assert!((actual - expected).abs() <= 0.0005);
    }
    assert_eq!(
        doc.page_paths(0).unwrap()[0].abbreviated_data.as_deref(),
        Some("M 1 1 L 8 1")
    );
    assert!(doc.validate_references().unwrap().is_empty());
}

#[test]
fn canvas_default_font_is_created_only_for_paint_and_failure_rolls_it_back() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(90., 90.).unwrap();
    doc.set_font_attribute(0, "FontName", Some("Other"))
        .unwrap();
    doc.set_font_attribute(0, "FamilyName", Some("Other"))
        .unwrap();
    let before = doc.to_bytes().unwrap();
    assert!(
        doc.measure_canvas_text(0, "text", &CanvasFont::default())
            .unwrap()
            .width
            > 0.
    );
    assert_eq!(doc.to_bytes().unwrap(), before);
    assert!(doc
        .draw_canvas(
            0,
            BOUNDARY,
            &[FillText {
                text: " \t\r\n".into(),
                point: [f64::NAN, f64::NAN]
            }]
        )
        .unwrap()
        .is_empty());
    assert_eq!(doc.to_bytes().unwrap(), before);
    assert!(doc
        .draw_canvas(
            0,
            BOUNDARY,
            &[
                FillText {
                    text: "text".into(),
                    point: [2., 5.]
                },
                SetFont {
                    font: CanvasFont {
                        size: -1.,
                        ..Default::default()
                    }
                },
            ]
        )
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
    doc.draw_canvas(
        0,
        BOUNDARY,
        &[FillText {
            text: "text\u{a0}".into(),
            point: [2., 5.],
        }],
    )
    .unwrap();
    let fonts = doc.fonts().unwrap();
    assert_eq!(
        fonts
            .iter()
            .filter(|font| font.font_name == "SimSun")
            .count(),
        1
    );
    doc.add_embedded_font("Extra", FONT).unwrap();
    assert!(doc.validate_references().unwrap().is_empty());
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(
        doc.page_png(0, 101.6).unwrap(),
        reopened.page_png(0, 101.6).unwrap()
    );
}

#[test]
fn invalid_canvas_font_metrics_and_late_text_errors_are_atomic() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(90., 90.).unwrap();
    let before = doc.to_bytes().unwrap();
    for style in [
        CanvasFont {
            size: f64::INFINITY,
            ..Default::default()
        },
        CanvasFont {
            letter_spacing: f64::NAN,
            ..Default::default()
        },
        CanvasFont {
            weight: 450,
            ..Default::default()
        },
        CanvasFont {
            read_direction: 1,
            ..Default::default()
        },
        CanvasFont {
            char_direction: 360,
            ..Default::default()
        },
        CanvasFont {
            font_id: Some("99999".into()),
            ..Default::default()
        },
    ] {
        assert!(doc
            .draw_canvas(
                0,
                BOUNDARY,
                &[
                    FillText {
                        text: "first".into(),
                        point: [2., 5.]
                    },
                    SetFont { font: style },
                    FillText {
                        text: "invalid".into(),
                        point: [2., 10.]
                    },
                ]
            )
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    assert!(doc
        .draw_canvas(
            0,
            BOUNDARY,
            &[FillText {
                text: "x".repeat(1_000_001),
                point: [0., 0.]
            }]
        )
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn canvas_text_numbers_match_independent_java_serialization() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canvas-text-java");
    let reference: Value =
        serde_json::from_slice(&std::fs::read(root.join("reference.json")).unwrap()).unwrap();
    let mut doc = OfdDocument::new();
    doc.add_blank_page(90., 90.).unwrap();
    for case in reference["number_format"].as_array().unwrap() {
        let value = case["value"].as_f64().unwrap();
        let expected: f64 = case["formatted"].as_str().unwrap().parse().unwrap();
        let ids = doc
            .draw_canvas(
                0,
                BOUNDARY,
                &[
                    SetFont {
                        font: CanvasFont {
                            size: 1.0625,
                            ..Default::default()
                        },
                    },
                    FillText {
                        text: "x".into(),
                        point: [value, value],
                    },
                ],
            )
            .unwrap();
        let objects = doc.page_text_objects(0).unwrap();
        let object = objects
            .iter()
            .find(|object| object.id.as_deref() == Some(&ids[0]))
            .unwrap();
        assert_eq!(object.size, Some(1.063));
        assert_eq!(
            (object.codes[0].x, object.codes[0].y),
            (Some(expected), Some(expected)),
            "{value}"
        );
    }
}

#[test]
fn canvas_text_uses_frozen_clipping_fill_color_and_global_alpha() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(30., 20.).unwrap();
    let id = doc
        .add_embedded_font("CanvasTest", FONT)
        .unwrap()
        .to_string();
    doc.draw_canvas(
        0,
        [0., 0., 30., 20.],
        &[
            SetFont {
                font: CanvasFont {
                    font_id: Some(id),
                    size: 8.,
                    ..Default::default()
                },
            },
            SetFillColor { color: [255, 0, 0] },
            SetGlobalAlpha { alpha: Some(0.5) },
            Rect {
                rect: [0., 0., 7., 20.],
            },
            Clip { even_odd: false },
            FillText {
                text: "MMMM".into(),
                point: [1., 10.],
            },
        ],
    )
    .unwrap();
    let pixels = image::load_from_memory(&doc.page_png(0, 101.6).unwrap())
        .unwrap()
        .to_rgb8();
    assert!(pixels.pixels().any(|p| p[0] > p[1] && p[1] > 0));
    assert!((32..pixels.width())
        .all(|x| (0..pixels.height()).all(|y| pixels.get_pixel(x, y).0 == [255, 255, 255])));
}

#[test]
fn canvas_text_creates_missing_public_resources_for_an_imported_document() {
    let mut source = OfdDocument::new();
    source.add_blank_page(90., 90.).unwrap();
    let mut doc = OfdDocument::from_bytes(&source.to_bytes().unwrap()).unwrap();
    let xml = std::str::from_utf8(doc.entry("Doc_0/Document.xml").unwrap().unwrap()).unwrap();
    let removed = xml.replace("<ofd:PublicRes>PublicRes.xml</ofd:PublicRes>", "");
    assert_ne!(xml, removed);
    doc.set_entry("Doc_0/Document.xml", removed.into_bytes())
        .unwrap();
    doc.remove_entry("Doc_0/PublicRes.xml").unwrap();
    let before = doc.to_bytes().unwrap();
    doc.measure_canvas_text(0, "canvas", &CanvasFont::default())
        .unwrap();
    assert_eq!(doc.to_bytes().unwrap(), before);
    doc.draw_canvas(
        0,
        BOUNDARY,
        &[FillText {
            text: "canvas".into(),
            point: [2., 5.],
        }],
    )
    .unwrap();
    assert!(doc.validate_references().unwrap().is_empty());
    assert_eq!(doc.fonts().unwrap().len(), 1);
    assert_eq!(doc.fonts().unwrap()[0].font_name, "SimSun");
    doc.page_png(0, 101.6).unwrap();
}

#[test]
fn canvas_control_character_advances_match_independent_java_metrics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canvas-text-java");
    let reference: Value =
        serde_json::from_slice(&std::fs::read(root.join("controls.json")).unwrap()).unwrap();
    assert_eq!(
        reference["java_revision"],
        "84e9eb260ba6fd351ecccf08a01a40305d4af635"
    );
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let font_id = doc
        .add_embedded_font("CanvasTest", FONT)
        .unwrap()
        .to_string();
    let style = CanvasFont {
        font_id: Some(font_id),
        size: 4.,
        ..Default::default()
    };
    let before = doc.to_bytes().unwrap();
    for case in reference["cases"].as_array().unwrap() {
        let code = case["code"].as_u64().unwrap() as u32;
        let text = format!("A{}B", char::from_u32(code).unwrap());
        let actual = doc.measure_canvas_text(0, &text, &style).unwrap();
        assert_eq!(
            actual.width,
            case["metrics"]["width"].as_f64().unwrap(),
            "U+{code:04X}"
        );
        assert_eq!(
            serde_json::to_value(actual.offsets).unwrap(),
            case["metrics"]["offsets"],
            "U+{code:04X}"
        );
    }
    assert_eq!(doc.to_bytes().unwrap(), before);
}
