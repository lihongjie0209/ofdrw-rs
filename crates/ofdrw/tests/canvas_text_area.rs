use ofdrw::{CanvasFont, OfdDocument};
use serde_json::Value;

const FONT: &[u8] = include_bytes!("fixtures/dejavu-sans.ttf");

fn document() -> (OfdDocument, String) {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let id = doc
        .add_embedded_font("CanvasTest", FONT)
        .unwrap()
        .to_string();
    (doc, id)
}

#[test]
fn logical_areas_match_java_across_directions_tracking_and_control_characters() {
    let reference: Value =
        serde_json::from_str(include_str!("fixtures/canvas-text-java/areas.json")).unwrap();
    assert_eq!(
        reference["java_revision"],
        "84e9eb260ba6fd351ecccf08a01a40305d4af635"
    );
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let mut fonts = std::collections::HashMap::new();
    for (name, data) in [
        ("dejavu-sans.ttf", FONT),
        (
            "dejavu-serif.ttf",
            include_bytes!("fixtures/dejavu-serif.ttf").as_slice(),
        ),
        (
            "kerning-simhei.ttf",
            include_bytes!("fixtures/kerning-simhei.ttf").as_slice(),
        ),
        (
            "test-name-cff.otf",
            include_bytes!("fixtures/test-name-cff.otf").as_slice(),
        ),
    ] {
        fonts.insert(name, doc.add_embedded_font(name, data).unwrap().to_string());
    }
    let before = doc.to_bytes().unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 960);
    for case in cases {
        let font = CanvasFont {
            font_id: Some(fonts[case["font"].as_str().unwrap()].clone()),
            size: 4.,
            letter_spacing: case["letter_spacing"].as_f64().unwrap(),
            read_direction: case["read_direction"].as_u64().unwrap() as u16,
            char_direction: case["char_direction"].as_u64().unwrap() as u16,
            ..Default::default()
        };
        let actual = doc
            .measure_canvas_text_area(0, case["text"].as_str().unwrap(), &font)
            .unwrap();
        // AWT's FreeType CFF scaler quantizes metrics; Rust retains font-unit
        // precision. Bound the difference to 0.0001 mm per character while
        // retaining exact comparisons for the three TrueType references.
        // https://cr.openjdk.org/~andrew/aarch32-8/8u212/jdk/src/share/native/sun/font/freetypeScaler.c.html
        let tolerance = if case["font"] == "test-name-cff.otf" {
            0.0001
        } else {
            0.
        };
        let close = |a: f64, b: f64, error: f64| {
            assert!(
                (a - b).abs() <= error,
                "{case}: {a} != {b}, tolerance {error}"
            );
        };
        let count = actual.char_areas.len().max(1) as f64;
        close(
            actual.width,
            case["width"].as_f64().unwrap(),
            tolerance * count,
        );
        close(
            actual.height,
            case["height"].as_f64().unwrap(),
            tolerance * count,
        );
        assert_eq!(
            actual.letter_spacing,
            case["effective_letter_spacing"].as_f64().unwrap()
        );
        let expected: Vec<[f64; 4]> = serde_json::from_value(case["char_areas"].clone()).unwrap();
        assert_eq!(actual.char_areas.len(), expected.len());
        for (actual, expected) in actual.char_areas.iter().zip(expected) {
            for (a, b) in actual.iter().zip(expected) {
                close(*a, b, tolerance);
            }
        }
    }
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn supplementary_scalars_have_one_logical_box_and_survive_document_round_trips() {
    let (doc, id) = document();
    let font = CanvasFont {
        font_id: Some(id),
        size: 4.,
        ..Default::default()
    };
    let measured = doc.measure_canvas_text_area(0, "A😀", &font).unwrap();
    assert_eq!(measured.char_areas.len(), 2);
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(
        measured,
        reopened.measure_canvas_text_area(0, "A😀", &font).unwrap()
    );
}

#[test]
fn invalid_area_requests_return_errors_without_mutating_the_document() {
    let (doc, id) = document();
    let font = CanvasFont {
        font_id: Some(id),
        size: 4.,
        ..Default::default()
    };
    let before = doc.to_bytes().unwrap();
    assert!(doc.measure_canvas_text_area(1, "A", &font).is_err());
    for invalid in [
        CanvasFont {
            size: 0.,
            ..font.clone()
        },
        CanvasFont {
            read_direction: 45,
            ..font.clone()
        },
        CanvasFont {
            letter_spacing: f64::INFINITY,
            ..font.clone()
        },
        CanvasFont {
            letter_spacing: f64::MAX,
            ..font.clone()
        },
        CanvasFont {
            font_id: Some("999999".into()),
            ..font.clone()
        },
    ] {
        assert!(
            doc.measure_canvas_text_area(0, "ABC", &invalid).is_err(),
            "{invalid:?}"
        );
    }
    assert!(doc
        .measure_canvas_text_area(0, &"A".repeat(1_000_001), &font)
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn default_font_measurement_is_read_only_and_empty_text_reports_effective_tracking() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let before = doc.to_bytes().unwrap();
    let font = CanvasFont {
        letter_spacing: -0.25,
        ..Default::default()
    };
    let empty = doc.measure_canvas_text_area(0, "", &font).unwrap();
    assert_eq!(
        (empty.width, empty.height, empty.letter_spacing),
        (0., 0., 0.)
    );
    assert!(empty.char_areas.is_empty());
    let measured = doc.measure_canvas_text_area(0, "Canvas", &font).unwrap();
    assert!(measured.width > 0. && measured.height > 0.);
    assert_eq!(measured.char_areas.len(), 6);
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn negative_canvas_tracking_matches_java_zero_tracking_for_metrics_and_paint() {
    let (mut doc, id) = document();
    let zero = CanvasFont {
        font_id: Some(id),
        size: 4.,
        ..Default::default()
    };
    let negative = CanvasFont {
        letter_spacing: -100.,
        ..zero.clone()
    };
    assert_eq!(
        doc.measure_canvas_text(0, "WiA", &zero).unwrap(),
        doc.measure_canvas_text(0, "WiA", &negative).unwrap()
    );
    let mut other = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    for (document, font) in [(&mut doc, zero), (&mut other, negative)] {
        document
            .draw_canvas(
                0,
                [0., 0., 40., 30.],
                &[
                    ofdrw::CanvasCommand::SetFont { font },
                    ofdrw::CanvasCommand::FillText {
                        text: "WiA".into(),
                        point: [2., 8.],
                    },
                ],
            )
            .unwrap();
    }
    assert_eq!(
        doc.page_text_objects(0).unwrap(),
        other.page_text_objects(0).unwrap()
    );
}

#[test]
fn cff_area_metrics_retain_exact_font_unit_precision() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let id = doc
        .add_embedded_font("CffArea", include_bytes!("fixtures/test-name-cff.otf"))
        .unwrap()
        .to_string();
    let font = CanvasFont {
        font_id: Some(id),
        size: 4.,
        ..Default::default()
    };
    let area = doc.measure_canvas_text_area(0, "A", &font).unwrap();
    assert_eq!(area.width, 2.887969970703125);
    assert_eq!(area.height, 3.999948501586914);
    assert_eq!(
        area.char_areas,
        vec![[0., 0., 2.887969970703125, 3.999948501586914]]
    );
}
