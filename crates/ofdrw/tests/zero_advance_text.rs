use ofdrw::{
    BoxLayoutStyle, FlowContainer, FlowContainerChild, FlowSpan, FlowTextStyle, LayoutBox,
    OfdDocument, TextBox,
};

const FONT: &[u8] = include_bytes!("fixtures/dejavu-sans.ttf");
fn setup() -> (OfdDocument, FlowTextStyle) {
    let mut doc = OfdDocument::new();
    let font = doc.add_embedded_font("DejaVu Sans", FONT).unwrap();
    (
        doc,
        FlowTextStyle {
            font_id: Some(font.to_string()),
            font_size: 2.,
            line_height: 3.,
            page_size: [30., 20.],
            margins: [2.; 4],
            ..Default::default()
        },
    )
}
fn advance(ch: char) -> f64 {
    let face = ttf_parser::Face::parse(FONT, 0).unwrap();
    f64::from(
        face.glyph_hor_advance(face.glyph_index(ch).unwrap())
            .unwrap(),
    ) / f64::from(face.units_per_em())
        * 2.
}

#[test]
fn natural_zero_advance_combining_glyphs_measure_from_the_font_and_keep_measurement_read_only() {
    let (doc, style) = setup();
    assert_eq!(advance('\u{301}'), 0.);
    let before = doc.to_bytes().unwrap();
    let measured = doc
        .measure_flowed_spans(0, &[FlowSpan::new("e\u{301}")], 20., &style)
        .unwrap();
    assert!((measured.width - advance('e')).abs() < 1e-12);
    assert_eq!(measured.lines[0].text, "e\u{301}");
    assert_eq!(measured.height, 3.);
    let mark = doc
        .measure_flowed_spans(0, &[FlowSpan::new("\u{301}")], 20., &style)
        .unwrap();
    assert_eq!(mark.width, 0.);
    assert_eq!(mark.lines.len(), 1);
    assert_eq!(mark.height, 3.);
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn zero_advance_glyphs_across_styled_spans_share_the_next_pen_position_and_roundtrip() {
    let (mut doc, style) = setup();
    let spans = vec![
        FlowSpan::new("e"),
        FlowSpan {
            fill_color: Some([255, 0, 0]),
            underline: true,
            ..FlowSpan::new("\u{301}")
        },
        FlowSpan::new("X"),
    ];
    let measured = doc.measure_flowed_spans(0, &spans, 26., &style).unwrap();
    assert!((measured.width - advance('e') - advance('X')).abs() < 1e-12);
    doc.add_flowed_spans(0, &spans, style).unwrap();
    let objects = doc.page_text_objects(0).unwrap();
    assert_eq!(objects.len(), 3);
    assert_eq!(
        objects[1].boundary.unwrap()[0],
        objects[2].boundary.unwrap()[0]
    );
    assert_eq!(doc.page_text(0).unwrap(), ["e", "\u{301}", "X"]);
    assert!(!doc.page_png(0, 72.).unwrap().is_empty());
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text_objects(0).unwrap(), objects);
    assert!(reopened.validate_references().unwrap().is_empty());
}

#[test]
fn combining_graphemes_stay_whole_at_wraps_in_plain_styled_text() {
    let (mut doc, mut style) = setup();
    let width = advance('e') * 1.5;
    let spans = [FlowSpan::new("e\u{301}e\u{301}")];
    let measured = doc.measure_flowed_spans(0, &spans, width, &style).unwrap();
    assert_eq!(measured.lines.len(), 2);
    assert!(measured.lines.iter().all(|line| line.text == "e\u{301}"));
    style.page_size[0] = width + 4.;
    doc.add_flowed_spans(0, &spans, style).unwrap();
    assert_eq!(doc.page_text(0).unwrap(), ["e\u{301}", "e\u{301}"]);
}

#[test]
fn tracking_cannot_cancel_an_advancing_glyph_or_make_a_combining_advance_negative() {
    let (mut doc, style) = setup();
    let before = doc.to_bytes().unwrap();
    for span in [
        FlowSpan {
            letter_spacing: -advance('e'),
            ..FlowSpan::new("e")
        },
        FlowSpan {
            letter_spacing: -0.1,
            ..FlowSpan::new("\u{301}")
        },
        FlowSpan {
            letter_spacing: f64::NAN,
            ..FlowSpan::new("\u{301}")
        },
    ] {
        assert!(doc
            .measure_flowed_spans(0, std::slice::from_ref(&span), 20., &style)
            .is_err());
        assert!(doc.add_flowed_spans(0, &[span], style.clone()).is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    let measured = doc
        .measure_flowed_spans(
            0,
            &[FlowSpan {
                letter_spacing: 0.1,
                ..FlowSpan::new("\u{301}")
            }],
            20.,
            &style,
        )
        .unwrap();
    assert_eq!(measured.width, 0.1);
}

#[test]
fn text_boxes_and_mixed_containers_accept_natural_zero_advances() {
    let (mut boxes, style) = setup();
    let spans = vec![FlowSpan::new("e\u{301}")];
    boxes
        .add_text_boxes(
            0,
            &[TextBox {
                layout: LayoutBox {
                    size: [12., 0.],
                    ..Default::default()
                },
                spans: spans.clone(),
                style: style.clone(),
            }],
            BoxLayoutStyle::default(),
        )
        .unwrap();
    assert_eq!(boxes.page_text(0).unwrap(), ["e\u{301}"]);
    let (mut containers, style) = setup();
    containers
        .add_flow_containers(
            0,
            &[FlowContainer {
                layout: LayoutBox {
                    size: [12., 0.],
                    ..Default::default()
                },
                children: vec![FlowContainerChild::Paragraph {
                    spans,
                    style: Box::new(style),
                }],
                gap: 0.,
            }],
            BoxLayoutStyle::default(),
        )
        .unwrap();
    assert_eq!(containers.page_text(0).unwrap(), ["e\u{301}"]);
}
