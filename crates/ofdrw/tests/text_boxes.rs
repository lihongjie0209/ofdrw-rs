use ofdrw::{
    BoxClear, BoxFloat, BoxLayoutStyle, FlowSpan, FlowStart, FlowTextAlign, FlowTextStyle,
    LayoutBox, OfdDocument, TextBox,
};

fn page() -> BoxLayoutStyle {
    BoxLayoutStyle {
        page_size: [20., 14.],
        margins: [2.; 4],
        start: None,
    }
}
fn setup(shape: bool) -> (OfdDocument, FlowTextStyle) {
    let mut doc = OfdDocument::new();
    let font = doc
        .add_embedded_font("DejaVu Sans", include_bytes!("fixtures/dejavu-sans.ttf"))
        .unwrap();
    let text = FlowTextStyle {
        font_id: Some(font.to_string()),
        font_size: 2.,
        line_height: 3.,
        shape,
        ..Default::default()
    };
    (doc, text)
}
fn text_box(text: &str, style: FlowTextStyle) -> TextBox {
    TextBox {
        layout: LayoutBox {
            size: [8., 0.],
            ..Default::default()
        },
        spans: vec![FlowSpan::new(text)],
        style,
    }
}

#[test]
fn decorated_text_boxes_measure_and_draw_intact_lines_on_multiple_pages() {
    for shape in [false, true] {
        let (mut doc, style) = setup(shape);
        let mut b = text_box("A\nB\nC\nD", style);
        b.layout.padding = [0.5; 4];
        b.layout.border = [0.25; 4];
        b.layout.background_color = Some([240, 220, 200]);
        let result = doc.add_text_boxes(0, &[b], page()).unwrap();
        assert_eq!(result.pages, [0, 1]);
        assert_eq!(result.placements[0].content_box, [2.75, 2.75, 8., 9.]);
        assert_eq!(doc.page_text(0).unwrap(), ["A", "B", "C"]);
        assert_eq!(doc.page_text(1).unwrap(), ["D"]);
        let last = doc.page_text_objects(1).unwrap();
        assert_eq!(last[0].boundary.unwrap()[1], 2.);
        assert_eq!(last[0].boundary.unwrap()[3], 3.);
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_text(1).unwrap(), ["D"]);
        assert!(!reopened.page_png(0, 72.).unwrap().is_empty());
    }
}

#[test]
fn styled_plain_and_shaped_box_painting_matches_the_existing_flow_renderer() {
    for shape in [false, true] {
        let (mut doc, mut style) = setup(shape);
        style.page_size = [24., 60.];
        style.margins = [2.; 4];
        style.align = FlowTextAlign::Center;
        style.paragraph_gap = 1.;
        let mut first = FlowSpan::new("AV ");
        first.fill_color = Some([190, 20, 30]);
        first.underline = true;
        first.underline_offset = 0.2;
        first.underline_width = 0.1;
        first.bold = true;
        first.italic = true;
        first.letter_spacing = 0.1;
        let mut second = FlowSpan::new("مرحبا e\u{301}\nSecond paragraph");
        second.font_size = Some(3.);
        second.letter_spacing = 0.1;
        second.stroke = true;
        second.stroke_color = Some([10, 90, 160]);
        second.stroke_width = Some(0.05);
        let spans = vec![first, second];
        let mut reference = doc.clone();
        reference
            .add_flowed_spans(0, &spans, style.clone())
            .unwrap();
        doc.add_text_boxes(
            0,
            &[TextBox {
                layout: LayoutBox {
                    size: [20., 0.],
                    ..Default::default()
                },
                spans,
                style,
            }],
            BoxLayoutStyle {
                page_size: [24., 60.],
                margins: [2.; 4],
                start: None,
            },
        )
        .unwrap();
        assert_eq!(
            doc.page_text_objects(0).unwrap(),
            reference.page_text_objects(0).unwrap()
        );
        assert_eq!(
            doc.page_png(0, 72.).unwrap(),
            reference.page_png(0, 72.).unwrap()
        );
    }
}

#[test]
fn width_is_clamped_before_wrapping_and_measuring_height() {
    let (mut doc, style) = setup(false);
    let mut b = text_box("AAAA AAAA AAAA AAAA", style.clone());
    b.layout.size = [100., 999.];
    b.layout.padding = [1.; 4];
    let metrics = doc.measure_flowed_spans(0, &b.spans, 14., &style).unwrap();
    let result = doc.add_text_boxes(0, &[b], page()).unwrap();
    let consumed: f64 = result.placements.iter().map(|p| p.content_box[3]).sum();
    assert!((consumed - metrics.height).abs() < 1e-9);
    assert!(result.placements.iter().all(|p| p.content_box[2] == 14.));
}

#[test]
fn complete_paragraphs_are_kept_and_oversized_paragraphs_roll_back() {
    let (mut doc, mut style) = setup(false);
    style.keep_paragraphs_together = true;
    let mut b = text_box("AAAA AAAA AAAA", style);
    b.layout.size[0] = 8.;
    doc.append_blank_page(0, 20., 14.).unwrap();
    let mut geometry = page();
    geometry.start = Some(FlowStart {
        page_index: 0,
        y: 10.,
    });
    let result = doc.add_text_boxes(0, &[b.clone()], geometry).unwrap();
    assert_eq!(result.pages, [1]);
    let before = doc.to_bytes().unwrap();
    b.spans = vec![FlowSpan::new("AAAA ".repeat(30))];
    assert!(doc.add_text_boxes(0, &[b], page()).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn floating_boxes_keep_independent_lines_and_placeholder_suppresses_paint() {
    let (mut doc, style) = setup(true);
    let mut left = text_box("A\nB\nC\nD", style.clone());
    left.layout.size[0] = 6.;
    left.layout.clear = BoxClear::None;
    let mut right = text_box("E\nF", style.clone());
    right.layout.size[0] = 6.;
    right.layout.clear = BoxClear::None;
    right.layout.float = BoxFloat::Right;
    let result = doc.add_text_boxes(0, &[left, right], page()).unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(doc.page_text(0).unwrap(), ["A", "B", "C", "E", "F"]);
    assert_eq!(doc.page_text(1).unwrap(), ["D"]);
    let mut placeholder = text_box("hidden", style);
    placeholder.layout.placeholder = true;
    let result = doc.add_text_boxes(0, &[placeholder], page()).unwrap();
    assert!(doc
        .page_text(*result.pages.last().unwrap())
        .unwrap()
        .is_empty());
}

#[test]
fn invalid_later_boxes_and_unrenderable_text_roll_back_resources_and_pages() {
    for shape in [false, true] {
        let (mut doc, style) = setup(shape);
        let valid = text_box("A", style.clone());
        let before = doc.to_bytes().unwrap();
        for width in [0., -1., f64::NAN, f64::INFINITY] {
            let mut bad = valid.clone();
            bad.layout.size[0] = width;
            assert!(doc
                .add_text_boxes(0, &[valid.clone(), bad], page())
                .is_err());
            assert_eq!(doc.to_bytes().unwrap(), before);
        }
        let mut bad = text_box("Missing font", style.clone());
        bad.spans[0].font_id = Some("unknown".into());
        assert!(doc
            .add_text_boxes(0, &[valid.clone(), bad], page())
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
        let mut bad = text_box("too tall", style);
        bad.style.line_height = 30.;
        assert!(doc.add_text_boxes(0, &[valid, bad], page()).is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
}

#[test]
fn shared_box_geometry_overrides_text_geometry_and_relative_offsets_only_affect_drawing() {
    let (mut doc, mut style) = setup(false);
    style.page_size = [f64::NAN, 0.];
    style.margins = [-1.; 4];
    style.start = Some(FlowStart {
        page_index: usize::MAX,
        y: -1.,
    });
    let mut b = text_box("A", style);
    b.layout.offset = [1., 2.];
    let result = doc.add_text_boxes(0, &[b], page()).unwrap();
    assert_eq!(result.placements[0].allocation, [2., 2., 8., 3.]);
    assert_eq!(result.placements[0].content_box, [3., 4., 8., 3.]);
    assert_eq!(
        doc.page_text_objects(0).unwrap()[0].boundary.unwrap()[..2],
        [3., 4.]
    );
    assert_eq!(result.next.unwrap().y, 5.);
}
