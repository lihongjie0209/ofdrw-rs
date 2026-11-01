use ofdrw::{
    BoxLayoutItem, BoxLayoutStyle, FlowBlock, FlowDocumentStyle, FlowSpan, FlowStart,
    FlowTextAlign, FlowTextStyle, LayoutBox, OfdDocument,
};

fn setup(shape: bool) -> (OfdDocument, FlowTextStyle) {
    let mut document = OfdDocument::new();
    let font = document
        .add_embedded_font("DejaVu Serif", include_bytes!("fixtures/dejavu-serif.ttf"))
        .unwrap();
    (
        document,
        FlowTextStyle {
            page_size: [24., 200.],
            margins: [2.; 4],
            font_size: 2.,
            line_height: 3.,
            font_id: Some(font.to_string()),
            shape,
            ..Default::default()
        },
    )
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-8, "{actual} != {expected}");
}

fn compare_with_rendered_text(spans: &[FlowSpan], style: FlowTextStyle, mut doc: OfdDocument) {
    let width = style.page_size[0] - style.margins[1] - style.margins[3];
    let before = doc.to_bytes().unwrap();
    let measured = doc.measure_flowed_spans(0, spans, width, &style).unwrap();
    assert_eq!(doc.to_bytes().unwrap(), before);
    assert!(doc.pages().unwrap().is_empty());
    let result = doc
        .add_flow_blocks(
            0,
            &[FlowBlock::Paragraph {
                spans: spans.to_vec(),
                style: None,
            }],
            FlowDocumentStyle {
                text: style.clone(),
                block_gap: 0.,
            },
        )
        .unwrap();
    assert_eq!(result.pages, [0]);
    close(result.next.unwrap().y, style.margins[0] + measured.height);
    let objects = doc.page_text_objects(0).unwrap();
    let mut matched = 0;
    for line in &measured.lines {
        let on_line: Vec<_> = objects
            .iter()
            .filter(|o| (o.boundary.unwrap()[1] - style.margins[0] - line.y).abs() < 1e-8)
            .collect();
        if line.text.is_empty() {
            assert!(on_line.is_empty());
            continue;
        }
        assert!(!on_line.is_empty(), "missing line {line:?}");
        let text: String = on_line
            .iter()
            .flat_map(|o| o.codes.iter().map(|code| code.text.as_str()))
            .collect();
        assert_eq!(text, line.text);
        close(on_line[0].boundary.unwrap()[0], style.margins[3] + line.x);
        close(
            on_line.iter().map(|o| o.boundary.unwrap()[2]).sum(),
            line.width,
        );
        for object in &on_line {
            close(object.boundary.unwrap()[3], line.height);
            close(
                object.boundary.unwrap()[1] + object.codes[0].y.unwrap(),
                style.margins[0] + line.baseline,
            );
        }
        matched += on_line.len();
    }
    assert_eq!(matched, objects.len());
    close(
        measured.width,
        measured.lines.iter().map(|l| l.width).fold(0., f64::max),
    );
}

#[test]
fn plain_and_shaped_measurement_matches_wrapping_blank_lines_and_alignment() {
    for shape in [false, true] {
        for align in [
            FlowTextAlign::Left,
            FlowTextAlign::Center,
            FlowTextAlign::Right,
        ] {
            let (doc, mut style) = setup(shape);
            style.align = align;
            style.paragraph_gap = 1.;
            let mut span = FlowSpan::new("AV fi words that wrap\n\nLast line");
            span.underline = true;
            compare_with_rendered_text(&[span], style, doc);
        }
    }
}

#[test]
fn mixed_sizes_fonts_tracking_and_kept_spans_share_rendered_line_metrics() {
    for shape in [false, true] {
        let (mut doc, style) = setup(shape);
        let second = doc
            .add_embedded_font("DejaVu Sans", include_bytes!("fixtures/dejavu-sans.ttf"))
            .unwrap();
        let mut large = FlowSpan::new("AV ");
        large.font_size = Some(4.);
        large.bold = true;
        let mut tracked = FlowSpan::new("fi fi ");
        tracked.letter_spacing = 0.2;
        tracked.italic = true;
        tracked.font_id = Some(second.to_string());
        let mut kept = FlowSpan::new("keep this");
        kept.keep_together = true;
        compare_with_rendered_text(&[large, tracked, kept], style, doc);
    }
}

#[test]
fn first_rest_and_character_indentation_match_the_drawn_lines() {
    for shape in [false, true] {
        for chars in [false, true] {
            let (doc, mut style) = setup(shape);
            style.rest_line_indent = 2.;
            style.indent_each_paragraph = true;
            if chars {
                style.first_line_indent_chars = 3;
            } else {
                style.first_line_indent = 3.;
            }
            compare_with_rendered_text(
                &[FlowSpan::new(
                    "Indent enough words to wrap\nRepeat paragraph indentation",
                )],
                style,
                doc,
            );
        }
    }
}

#[test]
fn a_fully_consumed_first_indent_creates_the_same_blank_line_as_rendering() {
    for shape in [false, true] {
        let (doc, mut style) = setup(shape);
        style.page_size[0] = 8.;
        style.first_line_indent = 3.9;
        compare_with_rendered_text(&[FlowSpan::new("A")], style, doc);
    }
}

#[test]
fn bidi_and_combining_graphemes_have_the_same_measured_and_drawn_lines() {
    let (mut doc, mut style) = setup(true);
    let sans = doc
        .add_embedded_font("DejaVu Sans", include_bytes!("fixtures/dejavu-sans.ttf"))
        .unwrap();
    style.font_id = Some(sans.to_string());
    style.language = Some("ar".into());
    compare_with_rendered_text(
        &[FlowSpan::new("abc مرحبا xyz\na\u{301} combining fi")],
        style,
        doc,
    );
}

#[test]
fn measurement_ignores_page_geometry_starts_and_keep_rules_without_creating_pages() {
    let (doc, style) = setup(false);
    let spans = [FlowSpan::new("A\nB\nC")];
    let expected = doc.measure_flowed_spans(0, &spans, 20., &style).unwrap();
    let mut ignored = style;
    ignored.page_size = [f64::NAN, -1.];
    ignored.margins = [-10.; 4];
    ignored.start = Some(FlowStart {
        page_index: 999,
        y: f64::NAN,
    });
    ignored.keep_paragraphs_together = true;
    assert_eq!(
        doc.measure_flowed_spans(0, &spans, 20., &ignored).unwrap(),
        expected
    );
    assert_eq!(expected.height, 9.);
    assert!(doc.pages().unwrap().is_empty());
}

#[test]
fn terminal_empty_paragraphs_follow_the_existing_flow_renderer() {
    for shape in [false, true] {
        let (doc, style) = setup(shape);
        let measured = doc
            .measure_flowed_spans(0, &[FlowSpan::new("A\n\n")], 20., &style)
            .unwrap();
        assert_eq!(measured.lines.len(), 1);
        assert_eq!(measured.height, 3.);
        compare_with_rendered_text(&[FlowSpan::new("A\n\n")], style, doc);
    }
}

#[test]
fn advances_and_pair_kerning_match_the_embedded_font_tables() {
    let (doc, style) = setup(false);
    let face = ttf_parser::Face::parse(include_bytes!("fixtures/dejavu-serif.ttf"), 0).unwrap();
    let a = face.glyph_index('A').unwrap();
    let v = face.glyph_index('V').unwrap();
    let kern = face
        .tables()
        .kern
        .unwrap()
        .subtables
        .into_iter()
        .filter_map(|t| t.glyphs_kerning(a, v))
        .find(|v| *v != 0)
        .unwrap();
    let expected = (f64::from(face.glyph_hor_advance(a).unwrap())
        + f64::from(face.glyph_hor_advance(v).unwrap())
        + f64::from(kern))
        / f64::from(face.units_per_em())
        * style.font_size;
    let measured = doc
        .measure_flowed_spans(0, &[FlowSpan::new("AV")], 20., &style)
        .unwrap();
    close(measured.width, expected);
}

#[test]
fn unpaginated_measurement_can_describe_more_than_one_page() {
    for shape in [false, true] {
        let (doc, mut style) = setup(shape);
        style.page_size = [24., 10.];
        style.keep_paragraphs_together = true;
        let measured = doc
            .measure_flowed_spans(0, &[FlowSpan::new("A\nB\nC\nD")], 20., &style)
            .unwrap();
        assert_eq!(measured.height, 12.);
        assert_eq!(measured.lines.len(), 4);
        assert!(doc.pages().unwrap().is_empty());
    }
}

#[test]
fn measured_height_sizes_a_decorated_box_and_contains_all_rendered_lines() {
    for shape in [false, true] {
        let (mut doc, mut text) = setup(shape);
        text.page_size = [24., 80.];
        let spans = [FlowSpan::new("A few words wrapped inside a narrow box")];
        let metrics = doc.measure_flowed_spans(0, &spans, 8., &text).unwrap();
        let boxes = doc
            .add_box_layout(
                0,
                &[BoxLayoutItem::Box(LayoutBox {
                    size: [8., metrics.height],
                    padding: [1.; 4],
                    border: [0.2; 4],
                    integrity: true,
                    ..Default::default()
                })],
                BoxLayoutStyle {
                    page_size: [24., 80.],
                    margins: [2.; 4],
                    start: None,
                },
            )
            .unwrap();
        let p = &boxes.placements[0];
        let [x, y, w, h] = p.content_box;
        close(h, metrics.height);
        text.margins = [y, 24. - x - w, 80. - y - h, x];
        text.start = Some(FlowStart {
            page_index: p.page_index,
            y,
        });
        let rendered = doc
            .add_flow_blocks(
                0,
                &[FlowBlock::Paragraph {
                    spans: spans.to_vec(),
                    style: None,
                }],
                FlowDocumentStyle {
                    text,
                    block_gap: 0.,
                },
            )
            .unwrap();
        assert_eq!(rendered.pages, [p.page_index]);
        close(rendered.next.unwrap().y, y + h);
        assert!(doc
            .page_text_objects(p.page_index)
            .unwrap()
            .iter()
            .all(|o| o.boundary.unwrap()[1] + o.boundary.unwrap()[3] <= y + h + 1e-8));
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        assert_eq!(
            reopened.page_png(0, 144.).unwrap(),
            doc.page_png(0, 144.).unwrap()
        );
    }
}

#[test]
fn invalid_measurements_leave_resources_and_package_bytes_unchanged() {
    let (doc, style) = setup(false);
    let before = doc.to_bytes().unwrap();
    for width in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(doc
            .measure_flowed_spans(0, &[FlowSpan::new("A")], width, &style)
            .is_err());
    }
    assert!(doc.measure_flowed_spans(0, &[], 20., &style).is_err());
    assert!(doc
        .measure_flowed_spans(1, &[FlowSpan::new("A")], 20., &style)
        .is_err());
    let mut bad = FlowSpan::new("A");
    bad.font_id = Some("missing".into());
    assert!(doc.measure_flowed_spans(0, &[bad], 20., &style).is_err());
    let mut bad = FlowSpan::new("A");
    bad.font_size = Some(f64::NAN);
    assert!(doc.measure_flowed_spans(0, &[bad], 20., &style).is_err());
    let mut bad = FlowSpan::new("A");
    bad.language = Some("en".into());
    assert!(doc.measure_flowed_spans(0, &[bad], 20., &style).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn overflowing_logical_height_and_unrenderable_input_return_errors_without_mutation() {
    for shape in [false, true] {
        let (doc, mut style) = setup(shape);
        let before = doc.to_bytes().unwrap();
        style.line_height = 1e308;
        assert!(doc
            .measure_flowed_spans(0, &[FlowSpan::new("A\nB")], 20., &style)
            .is_err());
        style.line_height = 3.;
        assert!(doc
            .measure_flowed_spans(0, &[FlowSpan::new("\n\n")], 20., &style)
            .is_err());
        let mut span = FlowSpan::new("unbreakable");
        span.keep_together = true;
        assert!(doc.measure_flowed_spans(0, &[span], 1., &style).is_err());
        style.shape = false;
        style.language = Some("en".into());
        assert!(doc
            .measure_flowed_spans(0, &[FlowSpan::new("A")], 20., &style)
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
}

#[test]
fn measurement_uses_the_selected_document_body_font() {
    let (mut doc, style) = setup(false);
    doc.append_document_body().unwrap();
    let font = doc
        .add_embedded_font_to_document(1, "DejaVu Sans", include_bytes!("fixtures/dejavu-sans.ttf"))
        .unwrap();
    let other = FlowTextStyle {
        font_id: Some(font.to_string()),
        ..style.clone()
    };
    let original = doc
        .measure_flowed_spans(0, &[FlowSpan::new("AV fi")], 20., &style)
        .unwrap();
    let second = doc
        .measure_flowed_spans(1, &[FlowSpan::new("AV fi")], 20., &other)
        .unwrap();
    assert!((original.width - second.width).abs() > 0.01);
    assert!(doc.pages().unwrap().is_empty());
}

#[test]
fn shaped_measurement_rejects_missing_glyphs_just_like_the_writer() {
    let (mut doc, style) = setup(true);
    let spans = [FlowSpan::new("\u{10ffff}")];
    let before = doc.to_bytes().unwrap();
    assert!(doc.measure_flowed_spans(0, &spans, 20., &style).is_err());
    assert!(doc.add_flowed_spans(0, &spans, style).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}
