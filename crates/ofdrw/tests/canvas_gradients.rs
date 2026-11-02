use ofdrw::{
    CanvasCommand::*, CanvasFont, DrawParamStyle, GradientFillStyle, GradientKind, GradientMapType,
    GradientSegment, OfdDocument, PageArea,
};
use std::path::Path;

fn gradient(radial: bool) -> GradientFillStyle {
    GradientFillStyle {
        kind: if radial {
            GradientKind::Radial {
                start_radius: 0.,
                end_radius: 9.,
                eccentricity: 0.,
                angle: 0.,
            }
        } else {
            GradientKind::Axial
        },
        start_point: if radial { [11., 8.] } else { [0., 0.] },
        end_point: if radial { [11., 8.] } else { [22., 0.] },
        segments: [(0., [255, 0, 0]), (0.5, [255, 255, 0]), (1., [0, 0, 255])]
            .into_iter()
            .map(|(position, color)| GradientSegment {
                position: Some(position),
                color,
                alpha: None,
            })
            .collect(),
        map_type: GradientMapType::Direct,
        map_unit: None,
        extend: u8::from(radial),
        fill_alpha: None,
    }
}

fn check(name: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canvas-gradient-java");
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/canvas-gradient-java/reference.json")).unwrap();
    assert_eq!(
        reference["java_revision"],
        "84e9eb260ba6fd351ecccf08a01a40305d4af635"
    );
    let default = format!("{name}.ofd");
    let file = reference["reference_overrides"][name]
        .as_str()
        .unwrap_or(&default);
    let java = OfdDocument::open(root.join(file)).unwrap();
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
    let mut style = DrawParamStyle {
        fill_gradient: Some(gradient(name == "radial-fill")),
        ..Default::default()
    };
    if name == "stroke-save" {
        style.fill_gradient = None;
        style.stroke_gradient = Some(gradient(false));
        style.line_width = Some(1.2);
    }
    let mut commands = vec![SetDrawParam {
        style: Box::new(style),
    }];
    match name {
        "axial-fill" | "radial-fill" => commands.push(FillRect {
            rect: [2., 2., 18., 12.],
        }),
        "stroke-save" => commands.extend([
            Rect {
                rect: [2., 2., 18., 10.],
            },
            Save,
            SetStrokeColor { color: [0, 200, 0] },
            Stroke,
            Restore,
            Translate { offset: [0., 3.] },
            Stroke,
        ]),
        "alpha-clip" => commands.extend([
            Rect {
                rect: [3., 3., 12., 10.],
            },
            Clip { even_odd: false },
            BeginPath,
            SetGlobalAlpha { alpha: Some(0.5) },
            FillRect {
                rect: [0., 0., 22., 18.],
            },
        ]),
        "text-fill" => {
            let id = doc
                .add_embedded_font("CanvasTest", include_bytes!("fixtures/dejavu-sans.ttf"))
                .unwrap()
                .to_string();
            commands.extend([
                SetFont {
                    font: CanvasFont {
                        font_id: Some(id),
                        size: 4.,
                        ..Default::default()
                    },
                },
                FillText {
                    text: "Gradient".into(),
                    point: [2., 12.],
                },
            ]);
        }
        "transform-fill" => commands.extend([
            Transform {
                matrix: [1., 0.2, 0.4, 1., 2., 1.],
            },
            FillRect {
                rect: [2., 2., 14., 10.],
            },
        ]),
        _ => panic!("unknown gradient reference"),
    }
    let ids = doc.draw_canvas(0, [4., 3., 24., 20.], &commands).unwrap();
    assert_eq!(ids.len(), java.page_objects(0).unwrap().len(), "{name}");
    assert!(doc.page_images(0).unwrap().is_empty());
    assert!(doc.validate_references().unwrap().is_empty());
    if name == "stroke-save" {
        assert_eq!(
            doc.page_object_stroke_styles(0).unwrap()[1]
                .style
                .line_width,
            1.2
        );
        let raw = OfdDocument::open(root.join("stroke-save.ofd")).unwrap();
        assert_eq!(
            raw.page_object_stroke_styles(0).unwrap()[1]
                .style
                .line_width,
            0.353
        );
    }
    for dpi in [25.4, 101.6] {
        assert_eq!(
            doc.page_png(0, dpi).unwrap(),
            java.page_png(0, dpi).unwrap(),
            "{name}, {dpi} DPI"
        );
    }
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(
        doc.page_png(0, 101.6).unwrap(),
        reopened.page_png(0, 101.6).unwrap()
    );
    assert!(lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok());
}

#[test]
fn axial_fill() {
    check("axial-fill");
}
#[test]
fn radial_fill() {
    check("radial-fill");
}
#[test]
fn stroke_save() {
    check("stroke-save");
}
#[test]
fn alpha_clip() {
    check("alpha-clip");
}
#[test]
fn text_fill() {
    check("text-fill");
}
#[test]
fn transform_fill() {
    check("transform-fill");
}
