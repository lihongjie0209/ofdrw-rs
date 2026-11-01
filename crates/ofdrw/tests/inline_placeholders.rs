use ofdrw::{
    BoxLayoutStyle, FixedContainer, FlowContainer, FlowContainerChild, FlowSpan, FlowTextStyle,
    LayoutBox, OfdDocument, TextBox,
};

const FONT: &[u8] = include_bytes!("fixtures/dejavu-sans.ttf");
fn style(shape: bool) -> FlowTextStyle {
    FlowTextStyle {
        font_size: 2.,
        line_height: 3.,
        page_size: [30., 20.],
        margins: [2.; 4],
        shape,
        ..Default::default()
    }
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
fn placeholder_metrics_match_independent_java_lines_without_embedded_fonts() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/inline-placeholders-java.json")).unwrap();
    for shape in [false, true] {
        let doc = OfdDocument::new();
        let before = doc.to_bytes().unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let spans = case["spans"]
                .as_array()
                .unwrap()
                .iter()
                .map(|span| {
                    FlowSpan::placeholder(
                        span["placeholder_width"].as_f64().unwrap(),
                        span["font_size"].as_f64().unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            let metrics = doc
                .measure_flowed_spans(
                    0,
                    &spans,
                    case["content_width"].as_f64().unwrap(),
                    &FlowTextStyle {
                        first_line_indent: case["style"]["first_line_indent"].as_f64().unwrap(),
                        ..style(shape)
                    },
                )
                .unwrap();
            let expected = case["lines"].as_array().unwrap();
            assert_eq!(
                metrics.lines.len(),
                expected.len(),
                "{} {shape}",
                case["name"]
            );
            for (line, reference) in metrics.lines.iter().zip(expected) {
                assert!(line.text.is_empty());
                for (actual, key) in [
                    (line.width, "width"),
                    (line.height, "height"),
                    (line.baseline, "baseline"),
                    (line.x, "x"),
                    (line.y, "y"),
                ] {
                    assert!(
                        (actual - reference[key].as_f64().unwrap()).abs() < 1e-10,
                        "{} {shape}: {key}: {actual}",
                        case["name"]
                    );
                }
            }
            assert_eq!(
                metrics.width,
                metrics
                    .lines
                    .iter()
                    .map(|line| line.width)
                    .fold(0., f64::max)
            );
            assert_eq!(
                metrics.height,
                metrics.lines.iter().map(|line| line.height).sum::<f64>()
            );
        }
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
}

#[test]
fn placeholder_only_flow_paginates_without_objects_or_font_resources() {
    for shape in [false, true] {
        let mut doc = OfdDocument::new();
        let fonts = doc.fonts().unwrap();
        let spans = vec![FlowSpan::placeholder(4., 3.); 3];
        let pages = doc
            .add_flowed_spans(
                0,
                &spans,
                FlowTextStyle {
                    page_size: [6., 8.],
                    margins: [1.; 4],
                    ..style(shape)
                },
            )
            .unwrap();
        assert_eq!(pages, [0, 1, 2]);
        assert_eq!(doc.fonts().unwrap(), fonts);
        for page in pages {
            assert!(doc.page_text(page).unwrap().is_empty());
            assert!(doc.page_objects(page).unwrap().is_empty());
        }
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.pages().unwrap().len(), 3);
    }
}

#[test]
fn styled_placeholder_reserves_pen_and_baseline_without_painting_any_decoration() {
    let mut doc = OfdDocument::new();
    let font = doc
        .add_embedded_font("DejaVu Sans", FONT)
        .unwrap()
        .to_string();
    let mut reference = doc.clone();
    let mut placeholder = FlowSpan::placeholder(4., 6.);
    placeholder.underline = true;
    placeholder.stroke = true;
    placeholder.bold = true;
    placeholder.italic = true;
    placeholder.font_id = Some("not-a-font".into());
    let spans = [FlowSpan::new("A"), placeholder, FlowSpan::new("B")];
    let text_style = FlowTextStyle {
        font_id: Some(font.clone()),
        ..style(false)
    };
    let metrics = doc
        .measure_flowed_spans(0, &spans, 26., &text_style)
        .unwrap();
    assert_eq!(metrics.lines[0].text, "AB");
    assert_eq!(metrics.lines[0].height, 7.);
    assert_eq!(metrics.lines[0].baseline, 6.);
    assert!((metrics.width - (advance('A') + 4. + advance('B'))).abs() < 1e-12);
    doc.add_flowed_spans(0, &spans, text_style).unwrap();
    assert_eq!(doc.page_text(0).unwrap(), ["A", "B"]);
    assert_eq!(doc.page_objects(0).unwrap().len(), 2);
    reference.add_blank_page(30., 20.).unwrap();
    reference
        .add_text_object(0, "A", [2., 6., advance('A'), 3.], 2., Some(&font))
        .unwrap();
    reference
        .add_text_object(
            0,
            "B",
            [2. + advance('A') + 4., 6., advance('B'), 3.],
            2.,
            Some(&font),
        )
        .unwrap();
    assert_eq!(
        doc.page_png(0, 144.).unwrap(),
        reference.page_png(0, 144.).unwrap()
    );
}

#[test]
fn placeholders_do_not_swallow_combining_marks_or_leak_replacement_characters() {
    for shape in [false, true] {
        let mut doc = OfdDocument::new();
        let font = doc
            .add_embedded_font("DejaVu Sans", FONT)
            .unwrap()
            .to_string();
        let spans = [
            FlowSpan::new("e"),
            FlowSpan::placeholder(3., 2.),
            FlowSpan::new("\u{301}X\u{fffc}"),
        ];
        let text_style = FlowTextStyle {
            font_id: Some(font),
            ..style(shape)
        };
        let metrics = doc
            .measure_flowed_spans(0, &spans, 26., &text_style)
            .unwrap();
        assert_eq!(
            metrics
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<String>(),
            "e\u{301}X\u{fffc}"
        );
        doc.add_flowed_spans(0, &spans, text_style).unwrap();
        assert_eq!(doc.page_text(0).unwrap().concat(), "e\u{301}X\u{fffc}");
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_text(0).unwrap().concat(), "e\u{301}X\u{fffc}");
        assert!(!reopened.page_png(0, 72.).unwrap().is_empty());
    }
}

#[test]
fn placeholder_character_counts_and_leading_indent_replacement_preserve_later_paragraph_indents() {
    let doc = OfdDocument::new();
    let span = FlowSpan::placeholder_characters(3, 2., 0.5);
    assert_eq!(
        doc.measure_flowed_spans(0, &[span], 20., &style(false))
            .unwrap()
            .width,
        7.5
    );
    for shape in [false, true] {
        let mut leading = FlowSpan::placeholder(30., 2.);
        leading.line_break_after = true;
        let metrics = doc
            .measure_flowed_spans(
                0,
                &[leading, FlowSpan::placeholder(2., 2.)],
                20.,
                &FlowTextStyle {
                    first_line_indent: 4.,
                    indent_each_paragraph: true,
                    ..style(shape)
                },
            )
            .unwrap();
        assert_eq!(metrics.lines.len(), 2);
        assert_eq!((metrics.lines[0].x, metrics.lines[0].width), (0., 4.));
        assert_eq!((metrics.lines[1].x, metrics.lines[1].width), (4., 2.));
    }
}

#[test]
fn placeholder_character_indent_replaces_width_and_crlf_does_not_cross_a_placeholder() {
    for shape in [false, true] {
        let mut doc = OfdDocument::new();
        let font = doc
            .add_embedded_font("DejaVu Sans", FONT)
            .unwrap()
            .to_string();
        let text_style = FlowTextStyle {
            font_id: Some(font),
            ..style(shape)
        };
        let mut placeholder = FlowSpan::placeholder(17., 2.);
        placeholder.letter_spacing = 0.5;
        let metrics = doc
            .measure_flowed_spans(
                0,
                &[placeholder],
                26.,
                &FlowTextStyle {
                    first_line_indent_chars: 3,
                    ..text_style.clone()
                },
            )
            .unwrap();
        assert_eq!((metrics.lines[0].x, metrics.width), (0., 7.5));
        let spans = [
            FlowSpan::new("A\r"),
            FlowSpan::placeholder(0., 4.),
            FlowSpan::new("\nB"),
        ];
        let metrics = doc
            .measure_flowed_spans(0, &spans, 26., &text_style)
            .unwrap();
        assert_eq!(
            metrics
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            ["A", "", "B"]
        );
        assert_eq!(metrics.lines[1].height, 5.);
        doc.add_flowed_spans(0, &spans, text_style).unwrap();
        assert_eq!(doc.page_text(0).unwrap(), ["A", "B"]);
    }
}

#[test]
fn placeholder_errors_and_late_paint_failures_leave_document_unchanged() {
    for shape in [false, true] {
        let mut doc = OfdDocument::new();
        let before = doc.to_bytes().unwrap();
        for placeholder in [
            FlowSpan::placeholder(-1., 2.),
            FlowSpan::placeholder(f64::NAN, 2.),
            FlowSpan::placeholder(f64::INFINITY, 2.),
            FlowSpan::placeholder(27., 2.),
            FlowSpan::placeholder(1., 0.),
            FlowSpan::placeholder(1., f64::INFINITY),
            FlowSpan::placeholder_characters(usize::MAX, f64::MAX, f64::MAX),
            FlowSpan {
                text: "hidden?".into(),
                ..FlowSpan::placeholder(1., 2.)
            },
        ] {
            assert!(doc
                .measure_flowed_spans(0, std::slice::from_ref(&placeholder), 26., &style(shape))
                .is_err());
            assert!(doc
                .add_flowed_spans(0, &[placeholder], style(shape))
                .is_err());
            assert_eq!(doc.to_bytes().unwrap(), before);
        }
        // A later oversized line fails after a valid placeholder already placed a page.
        assert!(doc
            .add_flowed_spans(
                0,
                &[
                    FlowSpan::placeholder(2., 2.),
                    FlowSpan::placeholder(2., 40.)
                ],
                style(shape)
            )
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
}

#[test]
fn automatic_text_boxes_and_fixed_and_flow_containers_share_placeholder_height_and_width() {
    for shape in [false, true] {
        let text_style = style(shape);
        let spans = vec![FlowSpan::placeholder(3., 5.), FlowSpan::placeholder(2., 2.)];
        let layout = LayoutBox {
            size: [10., 0.],
            ..Default::default()
        };
        let container = FlowContainer {
            layout: layout.clone(),
            gap: 0.,
            children: vec![FlowContainerChild::Paragraph {
                spans: spans.clone(),
                style: Box::new(text_style.clone()),
            }],
        };
        let page_style = BoxLayoutStyle {
            page_size: [30., 20.],
            margins: [2.; 4],
            ..Default::default()
        };
        let mut boxes = OfdDocument::new();
        let result = boxes
            .add_text_boxes(
                0,
                &[TextBox {
                    layout: layout.clone(),
                    spans,
                    style: text_style,
                }],
                page_style.clone(),
            )
            .unwrap();
        assert_eq!(result.placements[0].content_box, [2., 2., 10., 6.]);
        let mut flowed = OfdDocument::new();
        let result = flowed
            .add_flow_containers(0, std::slice::from_ref(&container), page_style)
            .unwrap();
        assert_eq!(result.placements[0].content_box, [2., 2., 10., 6.]);
        let mut fixed = OfdDocument::new();
        fixed.add_blank_page(30., 20.).unwrap();
        let result = fixed
            .add_fixed_containers(
                0,
                &[FixedContainer {
                    position: [2., 2.],
                    container,
                }],
            )
            .unwrap();
        assert_eq!(result[0].content_box, [2., 2., 10., 6.]);
        for doc in [&boxes, &flowed, &fixed] {
            assert!(doc.page_objects(0).unwrap().is_empty());
        }
    }
}

#[test]
fn shaped_zero_advance_marks_keep_visible_ink_and_reject_invalid_tracking() {
    for variant in 0..4 {
        let mut doc = OfdDocument::new();
        let font = doc
            .add_embedded_font("DejaVu Sans", FONT)
            .unwrap()
            .to_string();
        let mut span = FlowSpan::new("\u{301}");
        span.fill_color = Some([255, 0, 0]);
        span.stroke_color = Some([255, 0, 0]);
        span.stroke = variant == 1;
        span.stroke_width = (variant == 1).then_some(0.2);
        span.bold = variant == 2;
        span.italic = variant == 3;
        let text_style = FlowTextStyle {
            font_id: Some(font),
            ..style(true)
        };
        let metrics = doc
            .measure_flowed_spans(0, std::slice::from_ref(&span), 26., &text_style)
            .unwrap();
        assert_eq!(metrics.width, 0.);
        assert_eq!(metrics.lines[0].text, "\u{301}");
        doc.add_flowed_spans(0, std::slice::from_ref(&span), text_style.clone())
            .unwrap();
        let image = image::load_from_memory(&doc.page_png(0, 600.).unwrap())
            .unwrap()
            .to_rgba8();
        assert!(
            image
                .pixels()
                .any(|pixel| pixel[0] > 100 && pixel[1] < 80 && pixel[2] < 80 && pixel[3] > 50),
            "variant {variant} lost its mark"
        );
        let before = doc.to_bytes().unwrap();
        span.letter_spacing = -0.1;
        assert!(doc.add_flowed_spans(0, &[span], text_style).is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    let mut doc = OfdDocument::new();
    let font = doc
        .add_embedded_font("DejaVu Sans", FONT)
        .unwrap()
        .to_string();
    let mut span = FlowSpan::new("A");
    span.letter_spacing = -advance('A');
    assert!(doc
        .measure_flowed_spans(
            0,
            &[span],
            26.,
            &FlowTextStyle {
                font_id: Some(font),
                ..style(true)
            }
        )
        .is_err());
}
