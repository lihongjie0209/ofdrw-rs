use ofdrw::{BoxContentRanges, BoxLayoutItem, BoxLayoutStyle, LayoutBox, OfdDocument};

fn style() -> BoxLayoutStyle {
    BoxLayoutStyle {
        page_size: [20., 14.],
        margins: [2.; 4],
        start: None,
    }
}
fn item(height: f64) -> BoxLayoutItem {
    BoxLayoutItem::Box(LayoutBox {
        size: [8., height],
        ..Default::default()
    })
}

#[test]
fn protected_lines_move_intact_and_preserve_source_offsets() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_box_layout_with_content_ranges(
            0,
            &[item(15.)],
            &[BoxContentRanges {
                source_index: 0,
                ranges: vec![[0., 5.], [5., 11.], [11., 15.]],
            }],
            style(),
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(
        result
            .placements
            .iter()
            .map(|p| p.content_box[3])
            .collect::<Vec<_>>(),
        [5., 10.]
    );
    assert_eq!(result.placements[1].source_offset, 5.);
    assert_eq!(result.next.unwrap().y, 12.);
}

#[test]
fn exact_line_edges_can_fill_the_page_without_extra_blank_pages() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_box_layout_with_content_ranges(
            0,
            &[item(20.)],
            &[BoxContentRanges {
                source_index: 0,
                ranges: vec![[0., 5.], [5., 10.], [10., 20.]],
            }],
            style(),
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(result.placements.len(), 2);
    assert_eq!(result.placements[0].allocation[3], 10.);
    assert!(result.placements[1].last);
}

#[test]
fn decoration_bands_are_consumed_once_before_protected_content() {
    let mut doc = OfdDocument::new();
    let b = LayoutBox {
        size: [8., 15.],
        padding: [1.; 4],
        border: [0.5; 4],
        margin: [1.; 4],
        background_color: Some([240, 230, 220]),
        ..Default::default()
    };
    let result = doc
        .add_box_layout_with_content_ranges(
            0,
            &[BoxLayoutItem::Box(b)],
            &[BoxContentRanges {
                source_index: 0,
                ranges: vec![[0., 5.], [5., 15.]],
            }],
            style(),
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1, 2]);
    assert_eq!(result.placements[0].content_box, [4.5, 4.5, 8., 5.]);
    assert_eq!(result.placements[1].content_box[3], 10.);
    assert_eq!(result.placements[1].borders[0], 0.);
    assert_eq!(result.placements[1].borders[2], 0.);
    assert_eq!(result.placements[2].content_box[3], 0.);
    assert_eq!(result.placements[2].borders[2], 0.5);
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert!(!reopened.page_png(1, 30.).unwrap().is_empty());
}

#[test]
fn partial_existing_page_defers_a_whole_first_line() {
    let mut doc = OfdDocument::new();
    doc.append_blank_page(0, 20., 14.).unwrap();
    let mut page_style = style();
    page_style.start = Some(ofdrw::FlowStart {
        page_index: 0,
        y: 10.,
    });
    let result = doc
        .add_box_layout_with_content_ranges(
            0,
            &[item(5.)],
            &[BoxContentRanges {
                source_index: 0,
                ranges: vec![[0., 5.]],
            }],
            page_style,
        )
        .unwrap();
    assert_eq!(result.pages, [1]);
    assert_eq!(result.placements[0].content_box, [2., 2., 8., 5.]);
}

#[test]
fn invalid_ranges_and_unfit_lines_leave_the_document_unchanged() {
    let mut doc = OfdDocument::new();
    let before = doc.to_bytes().unwrap();
    for ranges in [
        vec![[0., 11.]],
        vec![[3., 2.]],
        vec![[-1., 2.]],
        vec![[0., 4.], [3., 5.]],
        vec![[0., f64::NAN]],
        vec![[0., 21.]],
    ] {
        assert!(doc
            .add_box_layout_with_content_ranges(
                0,
                &[item(20.)],
                &[BoxContentRanges {
                    source_index: 0,
                    ranges
                },],
                style()
            )
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    let policy = BoxContentRanges {
        source_index: 0,
        ranges: vec![],
    };
    for items in [vec![], vec![BoxLayoutItem::PageBreak]] {
        assert!(doc
            .add_box_layout_with_content_ranges(0, &items, std::slice::from_ref(&policy), style())
            .is_err());
    }
    assert!(doc
        .add_box_layout_with_content_ranges(0, &[item(1.)], &[policy.clone(), policy], style())
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn measured_text_lines_can_be_drawn_in_their_unsplit_fragments() {
    let mut doc = OfdDocument::new();
    let text_style = ofdrw::FlowTextStyle {
        font_size: 2.,
        line_height: 3.,
        ..Default::default()
    };
    let metrics = doc
        .measure_flowed_spans(0, &[ofdrw::FlowSpan::new("A\nB\nC\nD")], 8., &text_style)
        .unwrap();
    let b = LayoutBox {
        size: [8., metrics.height],
        padding: [0.5; 4],
        ..Default::default()
    };
    let result = doc
        .add_box_layout_with_content_ranges(
            0,
            &[BoxLayoutItem::Box(b)],
            &[BoxContentRanges::from_text(0, &metrics)],
            style(),
        )
        .unwrap();
    let mut drawn = 0;
    for p in &result.placements {
        let content_offset = (p.source_offset - 0.5).max(0.);
        for line in &metrics.lines {
            if line.y >= content_offset && line.y + line.height <= content_offset + p.content_box[3]
            {
                doc.add_text_object(
                    p.page_index,
                    &line.text,
                    [
                        p.content_box[0] + line.x,
                        p.content_box[1] + line.y - content_offset,
                        p.content_box[2],
                        line.height,
                    ],
                    2.,
                    None,
                )
                .unwrap();
                drawn += 1;
            }
        }
    }
    assert_eq!(drawn, 4);
    assert_eq!(doc.page_text(0).unwrap(), ["A", "B", "C"]);
    assert_eq!(doc.page_text(1).unwrap(), ["D"]);
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(1).unwrap(), ["D"]);
}

#[test]
fn floating_columns_consume_independent_safe_source_heights() {
    let mut doc = OfdDocument::new();
    let left = LayoutBox {
        size: [6., 15.],
        clear: ofdrw::BoxClear::None,
        ..Default::default()
    };
    let right = LayoutBox {
        size: [6., 15.],
        clear: ofdrw::BoxClear::None,
        float: ofdrw::BoxFloat::Right,
        ..Default::default()
    };
    let result = doc
        .add_box_layout_with_content_ranges(
            0,
            &[BoxLayoutItem::Box(left), BoxLayoutItem::Box(right)],
            &[
                BoxContentRanges {
                    source_index: 0,
                    ranges: vec![[0., 5.], [5., 11.]],
                },
                BoxContentRanges {
                    source_index: 1,
                    ranges: vec![[0., 8.], [8., 15.]],
                },
            ],
            style(),
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(
        result
            .placements
            .iter()
            .map(|p| (
                p.source_index,
                p.page_index,
                p.source_offset,
                p.content_box[3]
            ))
            .collect::<Vec<_>>(),
        [
            (0, 0, 0., 5.),
            (1, 0, 0., 8.),
            (0, 1, 5., 10.),
            (1, 1, 8., 7.)
        ]
    );
    assert_eq!(result.placements[0].content_box[0], 2.);
    assert_eq!(result.placements[1].content_box[0], 12.);
}
