mod common;
use ofdrw::{CanvasCommand, OfdDocument};
use serde_json::Value;
use std::path::Path;

fn array<const N: usize>(value: &Value) -> [f64; N] {
    assert_eq!(value.as_array().unwrap().len(), N);
    std::array::from_fn(|index| value[index].as_f64().unwrap())
}

fn commands(case: &Value) -> Vec<CanvasCommand> {
    use CanvasCommand::*;
    let mut result = Vec::new();
    for command in case["commands"].as_array().unwrap() {
        let next = match command["type"].as_str().unwrap() {
            "set_transform" => SetTransform {
                matrix: array(&command["matrix"]),
            },
            "begin_path" => BeginPath,
            "rect" => Rect {
                rect: array(&command["rect"]),
            },
            "move_to" => MoveTo {
                point: array(&command["point"]),
            },
            "bezier_curve_to" => BezierCurveTo {
                control1: array(&command["control1"]),
                control2: array(&command["control2"]),
                end: array(&command["end"]),
            },
            "quadratic_curve_to" => QuadraticCurveTo {
                control: array(&command["control"]),
                end: array(&command["end"]),
            },
            "circle_arc" => CircleArc {
                center: array(&command["center"]),
                radius: command["radius"].as_f64().unwrap(),
                start_angle: 0.,
                end_angle: 360.,
                sweep: true,
            },
            "close_path" => ClosePath,
            "clip" => Clip { even_odd: false },
            "save" => Save,
            "restore" => Restore,
            "fill_rect" => {
                let color = serde_json::from_value(command["color"].clone()).unwrap();
                result.extend([
                    SetLineWidth { width: 0.353 },
                    SetFillColor { color },
                    SetStrokeColor { color },
                ]);
                FillRect {
                    rect: array(&command["rect"]),
                }
            }
            other => panic!("unknown reference command {other}"),
        };
        result.push(next);
    }
    result
}

#[test]
fn frozen_affine_clips_match_independent_java2d_geometry_and_survive_round_trips() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canvas-affine-clips-java");
    let metadata: Value = serde_json::from_str(include_str!(
        "fixtures/canvas-affine-clips-java/reference.json"
    ))
    .unwrap();
    assert_eq!(
        metadata["java_commit"],
        "84e9eb260ba6fd351ecccf08a01a40305d4af635"
    );
    let cases = metadata["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 5);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let mut doc = OfdDocument::new();
        doc.add_blank_page(40., 30.).unwrap();
        doc.draw_canvas(0, [4., 3., 28., 22.], &commands(case))
            .unwrap();
        assert!(doc.validate_references().unwrap().is_empty());
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        for ppm in [4, 12] {
            let png = doc.page_png(0, f64::from(ppm) * 25.4).unwrap();
            let actual = image::load_from_memory(&png).unwrap().to_rgb8();
            let expected = image::open(root.join(format!("{name}-geometry-{ppm}.png")))
                .unwrap()
                .to_rgb8();
            common::assert_raster_matches(&actual, &expected, &format!("{name} at {ppm} px/mm"));
            assert_eq!(reopened.page_png(0, f64::from(ppm) * 25.4).unwrap(), png);
        }
        assert!(lopdf::Document::load_mem(&doc.to_pdf().unwrap()).is_ok());
    }
}

#[test]
fn pinned_java_imagemaker_ignores_object_clips_and_cannot_serve_as_a_positive_oracle() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canvas-affine-clips-java");
    for name in [
        "sheared-rect",
        "reflected-curves",
        "replacement",
        "save-restore",
        "sheared-circle",
    ] {
        let raw = image::open(root.join(format!("{name}-imagemaker-4.png")))
            .unwrap()
            .to_rgb8();
        let stripped = image::open(root.join(format!("{name}-imagemaker-no-clips-4.png")))
            .unwrap()
            .to_rgb8();
        assert!(
            raw.as_raw() == stripped.as_raw(),
            "{name}: update the recorded Java limitation if its renderer now applies Clips"
        );
        let geometry = image::open(root.join(format!("{name}-geometry-4.png")))
            .unwrap()
            .to_rgb8();
        assert!(raw.as_raw() != geometry.as_raw(), "{name}");
        let java = OfdDocument::open(root.join(format!("{name}.ofd"))).unwrap();
        assert!(java
            .page_object_clips(0)
            .unwrap()
            .iter()
            .any(|clip| !clip.clips.is_empty()));
    }
}
