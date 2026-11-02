use ofdrw::{CanvasCommand, DrawParamStyle, OfdDocument};

fn dashed_canvas(offset: f64, sequence: Vec<f64>) -> OfdDocument {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 20.).unwrap();
    doc.draw_canvas(
        0,
        [0., 0., 40., 20.],
        &[
            CanvasCommand::SetDrawParam {
                style: Box::new(DrawParamStyle {
                    line_width: Some(1.),
                    dash_sequence: Some(sequence),
                    dash_offset: Some(offset),
                    ..Default::default()
                }),
            },
            CanvasCommand::MoveTo { point: [2., 10.] },
            CanvasCommand::LineTo { point: [38., 10.] },
            CanvasCommand::Stroke,
        ],
    )
    .unwrap();
    doc
}

#[test]
fn negative_dash_offset_round_trips_and_matches_equivalent_positive_phase() {
    for sequence in [vec![3., 2.], vec![3., 2., 1.]] {
        let period = sequence.iter().sum::<f64>() * if sequence.len() % 2 == 0 { 1. } else { 2. };
        let negative = dashed_canvas(-1.5, sequence.clone());
        let positive = dashed_canvas(period - 1.5, sequence);
        let reopened = OfdDocument::from_bytes(&negative.to_bytes().unwrap()).unwrap();
        assert_eq!(
            reopened.page_object_stroke_styles(0).unwrap()[0]
                .style
                .dash_offset,
            -1.5
        );
        for dpi in [25.4, 101.6] {
            assert_eq!(
                negative.page_png(0, dpi).unwrap(),
                positive.page_png(0, dpi).unwrap()
            );
            assert_eq!(
                negative.page_png(0, dpi).unwrap(),
                reopened.page_png(0, dpi).unwrap()
            );
        }
    }
}

#[test]
fn nonfinite_dash_offsets_and_invalid_widths_remain_atomic() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 20.).unwrap();
    let before = doc.to_bytes().unwrap();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(doc
            .add_document_draw_param(
                0,
                &DrawParamStyle {
                    dash_offset: Some(value),
                    ..Default::default()
                }
            )
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    for style in [
        DrawParamStyle {
            line_width: Some(-1.),
            ..Default::default()
        },
        DrawParamStyle {
            miter_limit: Some(0.),
            ..Default::default()
        },
        DrawParamStyle {
            miter_limit: Some(-1.),
            ..Default::default()
        },
    ] {
        assert!(doc.add_document_draw_param(0, &style).is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
}
