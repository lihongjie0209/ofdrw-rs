use ofdrw::{
    BoxClear, BoxFloat, BoxLayoutStyle, FlowContainer, FlowContainerChild, FlowSpan, FlowStart,
    FlowTextAlign, FlowTextStyle, LayoutBox, NestedTable, OfdDocument, TableCell, TableStyle,
    TableVerticalAlign,
};

fn geometry() -> BoxLayoutStyle {
    BoxLayoutStyle {
        page_size: [20., 14.],
        margins: [2.; 4],
        start: None,
    }
}
fn paragraph(text: &str, height: f64) -> FlowContainerChild {
    FlowContainerChild::Paragraph {
        spans: vec![FlowSpan::new(text)],
        style: Box::new(FlowTextStyle {
            font_size: 2.,
            line_height: height,
            ..Default::default()
        }),
    }
}
fn container(children: Vec<FlowContainerChild>, width: f64) -> FlowContainer {
    FlowContainer {
        layout: LayoutBox {
            size: [width, 0.],
            ..Default::default()
        },
        children,
        gap: 0.,
    }
}
fn floating(text: &str, width: f64, line_height: f64, float: BoxFloat) -> FlowContainerChild {
    let mut b = container(vec![paragraph(text, line_height)], width);
    b.layout.float = float;
    b.layout.clear = BoxClear::None;
    FlowContainerChild::Container(Box::new(b))
}
fn cell(row: usize, column: usize, text: &str) -> TableCell {
    TableCell {
        row,
        column,
        row_span: 1,
        column_span: 1,
        text: text.into(),
        background_color: None,
        text_color: None,
        border_width: None,
        border_color: None,
        padding: None,
        font_id: None,
        font_size: None,
        line_height: None,
        letter_spacing: None,
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
        align: FlowTextAlign::Left,
        vertical_align: TableVerticalAlign::Top,
        image: None,
        blocks: vec![],
    }
}
fn table(values: &[&str], row_height: f64) -> FlowContainerChild {
    FlowContainerChild::Table {
        table: Box::new(NestedTable {
            row_count: values.len(),
            column_widths: vec![10.],
            cells: values
                .iter()
                .enumerate()
                .map(|(row, text)| cell(row, 0, text))
                .collect(),
            row_height,
            padding: 0.,
            border_width: 0.2,
            border_color: [0; 3],
        }),
        style: Box::new(TableStyle {
            font_size: 2.,
            line_height: 3.,
            repeat_header: true,
            ..Default::default()
        }),
        align: FlowTextAlign::Right,
    }
}

#[test]
fn nested_floating_columns_break_independently_and_next_row_follows_the_tallest_column() {
    let mut doc = OfdDocument::new();
    let root = container(
        vec![
            floating("A\nB\nC\nD", 6., 3., BoxFloat::Left),
            floating("E\nF\nG", 6., 4., BoxFloat::Right),
            paragraph("Z", 3.),
        ],
        16.,
    );
    let result = doc.add_flow_containers(0, &[root], geometry()).unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(doc.page_text(0).unwrap(), ["A", "B", "C", "E", "F"]);
    assert_eq!(doc.page_text(1).unwrap(), ["D", "G", "Z"]);
    assert_eq!(result.placements[0].content_box[3], 10.);
    assert_eq!(result.placements[1].source_offset, 10.);
    assert_eq!(result.next.unwrap().y, 9.);
    let objects = doc.page_text_objects(1).unwrap();
    assert_eq!(objects[0].boundary.unwrap()[..2], [2., 2.]);
    assert_eq!(objects[1].boundary.unwrap()[..2], [12., 2.]);
    assert_eq!(objects[2].boundary.unwrap()[1], 6.);
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(1).unwrap(), ["D", "G", "Z"]);
    assert!(!reopened.page_png(0, 72.).unwrap().is_empty());
}

#[test]
fn centered_groups_and_clear_rules_match_box_row_semantics() {
    let mut doc = OfdDocument::new();
    let mut root = container(
        vec![
            floating("A", 4., 3., BoxFloat::Center),
            floating("B", 4., 3., BoxFloat::Center),
            paragraph("Z", 3.),
        ],
        16.,
    );
    root.gap = 1.;
    doc.add_flow_containers(0, &[root], geometry()).unwrap();
    let objects = doc.page_text_objects(0).unwrap();
    assert_eq!(objects[0].boundary.unwrap()[..2], [6., 2.]);
    assert_eq!(objects[1].boundary.unwrap()[..2], [10., 2.]);
    assert_eq!(objects[2].boundary.unwrap()[1], 6.);

    let mut doc = OfdDocument::new();
    let left = floating("L", 4., 3., BoxFloat::Left);
    let mut right = floating("R", 4., 3., BoxFloat::Right);
    if let FlowContainerChild::Container(b) = &mut right {
        b.layout.clear = BoxClear::Right;
    }
    doc.add_flow_containers(
        0,
        &[container(vec![left, right.clone(), right], 16.)],
        geometry(),
    )
    .unwrap();
    let objects = doc.page_text_objects(0).unwrap();
    assert_eq!(objects[0].boundary.unwrap()[1], 2.);
    assert_eq!(objects[1].boundary.unwrap()[1], 2.);
    assert_eq!(objects[2].boundary.unwrap()[1], 5.);
}

#[test]
fn repeated_headers_stay_with_body_groups_and_expand_actual_container_height() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_flow_containers(
            0,
            &[container(
                vec![
                    paragraph("A", 3.),
                    table(&["H", "R1", "R2", "R3"], 4.),
                    paragraph("Z", 3.),
                ],
                12.,
            )],
            geometry(),
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1, 2, 3, 4]);
    for (page, text) in [
        vec!["A"],
        vec!["H", "R1"],
        vec!["H", "R2"],
        vec!["H", "R3"],
        vec!["Z"],
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(doc.page_text(page).unwrap(), text);
    }
    assert_eq!(
        result
            .placements
            .iter()
            .map(|p| p.content_box[3])
            .sum::<f64>(),
        30.
    );
    assert_eq!(result.next.unwrap().y, 5.);
}

#[test]
fn complete_parent_measures_floating_row_height_before_deciding_to_move() {
    let mut doc = OfdDocument::new();
    doc.append_blank_page(0, 20., 14.).unwrap();
    let mut root = container(
        vec![
            floating("A", 4., 3., BoxFloat::Left),
            floating("B", 4., 3., BoxFloat::Right),
        ],
        16.,
    );
    root.layout.integrity = true;
    let mut style = geometry();
    style.start = Some(FlowStart {
        page_index: 0,
        y: 8.,
    });
    let result = doc.add_flow_containers(0, &[root], style).unwrap();
    assert_eq!(result.pages, [0]);
    assert_eq!(result.next.unwrap().y, 11.);
    assert_eq!(doc.page_text(0).unwrap(), ["A", "B"]);
}

#[test]
fn invalid_header_pair_and_header_spans_roll_back_the_whole_call() {
    let mut doc = OfdDocument::new();
    let before = doc.to_bytes().unwrap();
    assert!(doc
        .add_flow_containers(
            0,
            &[container(
                vec![paragraph("A", 3.), table(&["H", "R1"], 6.)],
                12.
            )],
            geometry()
        )
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
    let mut merged = cell(0, 0, "H");
    merged.row_span = 2;
    let bad = FlowContainerChild::Table {
        table: Box::new(NestedTable {
            row_count: 2,
            column_widths: vec![10.],
            cells: vec![merged],
            row_height: 4.,
            padding: 0.,
            border_width: 0.2,
            border_color: [0; 3],
        }),
        style: Box::new(TableStyle {
            font_size: 2.,
            line_height: 3.,
            repeat_header: true,
            ..Default::default()
        }),
        align: FlowTextAlign::Left,
    };
    assert!(doc
        .add_flow_containers(0, &[container(vec![bad], 12.)], geometry())
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn header_lookahead_includes_the_first_complete_merged_body_group() {
    let mut header = cell(0, 0, "H");
    header.column_span = 2;
    let mut merged = cell(1, 0, "M");
    merged.row_span = 2;
    let mut last = cell(3, 0, "R3");
    last.column_span = 2;
    let grid = NestedTable {
        row_count: 4,
        column_widths: vec![5., 5.],
        cells: vec![header, merged, cell(1, 1, "R1"), cell(2, 1, "R2"), last],
        row_height: 4.,
        padding: 0.,
        border_width: 0.2,
        border_color: [0; 3],
    };
    let table = FlowContainerChild::Table {
        table: Box::new(grid),
        style: Box::new(TableStyle {
            font_size: 2.,
            line_height: 3.,
            repeat_header: true,
            ..Default::default()
        }),
        align: FlowTextAlign::Left,
    };
    let mut geometry = geometry();
    geometry.page_size[1] = 16.;
    let mut doc = OfdDocument::new();
    doc.add_flow_containers(
        0,
        &[container(
            vec![paragraph("A", 3.), table, paragraph("Z", 3.)],
            12.,
        )],
        geometry,
    )
    .unwrap();
    assert_eq!(doc.page_text(0).unwrap(), ["A"]);
    assert_eq!(doc.page_text(1).unwrap(), ["H", "M", "R1", "R2"]);
    assert_eq!(doc.page_text(2).unwrap(), ["H", "R3", "Z"]);
}

#[test]
fn existing_canvas_origins_apply_only_to_the_selected_page() {
    let mut doc = OfdDocument::new();
    doc.append_blank_page(0, 20., 14.).unwrap();
    doc.set_page_area(
        0,
        ofdrw::PageArea {
            physical_box: [30., 40., 20., 14.],
            application_box: None,
            content_box: None,
            bleed_box: None,
        },
    )
    .unwrap();
    doc.append_document_body().unwrap();
    doc.append_blank_page(1, 20., 14.).unwrap();
    let path = doc.pages().unwrap()[1].path.clone();
    let following = doc.entry(&path).unwrap().unwrap().to_vec();
    let root = container(
        vec![
            floating("A\nB\nC\nD", 6., 3., BoxFloat::Left),
            floating("E", 6., 3., BoxFloat::Right),
        ],
        16.,
    );
    let mut style = geometry();
    style.start = Some(FlowStart {
        page_index: 0,
        y: 2.,
    });
    let result = doc.add_flow_containers(0, &[root], style).unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(
        doc.page_text_objects(0).unwrap()[0].boundary.unwrap()[..2],
        [32., 42.]
    );
    assert_eq!(
        doc.page_text_objects(1).unwrap()[0].boundary.unwrap()[..2],
        [2., 2.]
    );
    assert_eq!(doc.entry(&path).unwrap().unwrap(), following);
    assert_eq!(doc.pages().unwrap()[2].document_index, 1);
}

#[test]
fn floating_shaped_text_images_and_repeated_tables_share_page_geometry() {
    let mut doc = OfdDocument::new();
    let mut left = container(vec![table(&["H", "R1", "R2", "R3"], 4.)], 7.);
    if let FlowContainerChild::Table { table, .. } = &mut left.children[0] {
        table.column_widths = vec![6.];
    }
    left.layout.clear = BoxClear::None;
    let font = doc
        .add_embedded_font("DejaVu Sans", include_bytes!("fixtures/dejavu-sans.ttf"))
        .unwrap();
    let shaped = FlowTextStyle {
        font_id: Some(font.to_string()),
        font_size: 2.,
        line_height: 3.,
        shape: true,
        ..Default::default()
    };
    let text = FlowContainerChild::Paragraph {
        spans: vec![FlowSpan::new("A\nB\nC\nD\ne\u{301}")],
        style: Box::new(shaped.clone()),
    };
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 2, image::Rgb([220, 20, 30])))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let picture = FlowContainerChild::Image {
        image: ofdrw::TableCellImage {
            data: bytes.into_inner(),
            format: "PNG".into(),
            width: 2.,
            height: 2.,
        },
        align: FlowTextAlign::Right,
    };
    let mut right = container(vec![text, picture], 7.);
    right.layout.float = BoxFloat::Right;
    right.layout.clear = BoxClear::None;
    let mut page = geometry();
    page.page_size[1] = 16.;
    let result = doc
        .add_flow_containers(
            0,
            &[container(
                vec![
                    FlowContainerChild::Container(Box::new(left)),
                    FlowContainerChild::Container(Box::new(right)),
                    paragraph("Z", 3.),
                ],
                16.,
            )],
            page,
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(
        doc.page_text(0).unwrap(),
        ["H", "R1", "R2", "A", "B", "C", "D"]
    );
    assert_eq!(doc.page_text(1).unwrap(), ["H", "R3", "e\u{301}", "Z"]);
    assert_eq!(
        doc.page_images(1).unwrap()[0].boundary,
        Some([16., 5., 2., 2.])
    );
    assert!(doc.validate_references().unwrap().is_empty());
}

#[test]
fn fractional_floating_decorations_keep_every_line_once_and_fail_atomically_on_overflow() {
    for index in 1..20 {
        let mut doc = OfdDocument::new();
        let mut left = match floating("A\nB\nC\nD", 5., 3., BoxFloat::Left) {
            FlowContainerChild::Container(b) => b,
            _ => unreachable!(),
        };
        let mut right = match floating("E\nF\nG", 5., 4., BoxFloat::Right) {
            FlowContainerChild::Container(b) => b,
            _ => unreachable!(),
        };
        let edge = f64::from(index) / 100.;
        left.layout.padding = [edge; 4];
        left.layout.border = [edge / 3.; 4];
        right.layout.margin = [edge / 7.; 4];
        right.layout.background_color = Some([230, 240, 220]);
        let root = container(
            vec![
                FlowContainerChild::Container(left),
                FlowContainerChild::Container(right),
            ],
            16.,
        );
        let result = doc.add_flow_containers(0, &[root], geometry()).unwrap();
        let all = result
            .pages
            .iter()
            .flat_map(|page| doc.page_text(*page).unwrap())
            .collect::<Vec<_>>();
        for expected in ["A", "B", "C", "D", "E", "F", "G"] {
            assert_eq!(
                all.iter().filter(|text| text.as_str() == expected).count(),
                1
            );
        }
    }
    let mut doc = OfdDocument::new();
    let before = doc.to_bytes().unwrap();
    let root = container(
        vec![
            floating("A", 6., 3., BoxFloat::Left),
            floating("B", 6., 11., BoxFloat::Right),
        ],
        16.,
    );
    assert!(doc.add_flow_containers(0, &[root], geometry()).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
    let root = container(
        vec![
            floating("A", 6., 3., BoxFloat::Left),
            FlowContainerChild::Spacer { height: 1e30 },
        ],
        16.,
    );
    assert!(doc.add_flow_containers(0, &[root], geometry()).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn planned_vertical_drawing_matches_the_existing_static_path() {
    let mut doc = OfdDocument::new();
    let mut nested = container(vec![paragraph("B\nC", 3.)], 10.);
    nested.layout.padding = [0.5; 4];
    nested.layout.border = [0.25; 4];
    nested.layout.background_color = Some([220, 230, 240]);
    nested.layout.offset = [0.5, 0.25];
    let mut table = table(&["R1", "R2", "R3"], 4.);
    if let FlowContainerChild::Table { style, .. } = &mut table {
        style.repeat_header = false;
    }
    let mut root = container(
        vec![
            paragraph("A\n\nB", 3.),
            FlowContainerChild::Container(Box::new(nested)),
            table,
        ],
        12.,
    );
    root.layout.padding = [0.5; 4];
    root.layout.border = [0.25; 4];
    root.layout.background_color = Some([240, 220, 200]);
    root.layout.alpha = Some(120);
    root.layout.offset = [0.5, 0.25];
    root.gap = 0.25;
    if let FlowContainerChild::Paragraph { style, .. } = &mut root.children[0] {
        style.paragraph_gap = 0.25;
    }
    let mut reference = doc.clone();
    let expected = reference
        .add_flow_containers(0, std::slice::from_ref(&root), geometry())
        .unwrap();
    if let FlowContainerChild::Container(nested) = &mut root.children[1] {
        nested.layout.clear = BoxClear::None;
    }
    let actual = doc.add_flow_containers(0, &[root], geometry()).unwrap();
    assert_eq!(actual, expected);
    for page in actual.pages {
        assert_eq!(
            doc.page_png(page, 72.).unwrap(),
            reference.page_png(page, 72.).unwrap()
        );
    }
}

#[test]
fn planned_placeholders_reserve_rows_without_painting_their_children_or_borders() {
    let mut hidden = match floating("X\nY", 6., 3., BoxFloat::Left) {
        FlowContainerChild::Container(b) => b,
        _ => unreachable!(),
    };
    hidden.layout.placeholder = true;
    hidden.layout.border = [1.; 4];
    hidden.layout.background_color = Some([255, 0, 0]);
    let root = container(
        vec![
            FlowContainerChild::Container(hidden),
            floating("B", 6., 3., BoxFloat::Right),
            paragraph("Z", 3.),
        ],
        16.,
    );
    let mut doc = OfdDocument::new();
    let result = doc.add_flow_containers(0, &[root], geometry()).unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(doc.page_text(0).unwrap(), ["B"]);
    assert_eq!(doc.page_objects(0).unwrap().len(), 1);
    assert_eq!(doc.page_text(1).unwrap(), ["Z"]);
    assert_eq!(result.next.unwrap().y, 5.);
}

#[test]
fn nested_paint_failure_rolls_back_staged_pages_and_objects() {
    let mut doc = OfdDocument::new();
    let before = doc.to_bytes().unwrap();
    let mut nested = match floating("B", 6., 3., BoxFloat::Left) {
        FlowContainerChild::Container(b) => b,
        _ => unreachable!(),
    };
    nested.layout.offset = [f64::MAX, 0.];
    let mut root = container(
        vec![paragraph("seed", 3.), FlowContainerChild::Container(nested)],
        16.,
    );
    root.layout.offset = [f64::MAX, 0.];
    assert!(doc.add_flow_containers(0, &[root], geometry()).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
    assert!(doc.pages().unwrap().is_empty());
}

#[test]
fn kept_paragraphs_move_together_inside_independently_continuing_columns() {
    let mut doc = OfdDocument::new();
    doc.append_blank_page(0, 20., 14.).unwrap();
    let typography = FlowTextStyle {
        font_size: 2.,
        line_height: 3.,
        keep_paragraphs_together: true,
        ..Default::default()
    };
    let spans = vec![FlowSpan::new("AAAA AAAA")];
    let measured = doc
        .measure_flowed_spans(0, &spans, 6., &typography)
        .unwrap();
    assert!(measured.height > 4. && measured.height <= 10.);
    let mut left = container(
        vec![FlowContainerChild::Paragraph {
            spans,
            style: Box::new(typography),
        }],
        6.,
    );
    left.layout.clear = BoxClear::None;
    let root = container(
        vec![
            FlowContainerChild::Container(Box::new(left)),
            floating("B", 6., 3., BoxFloat::Right),
        ],
        16.,
    );
    let mut style = geometry();
    style.start = Some(FlowStart {
        page_index: 0,
        y: 8.,
    });
    let result = doc.add_flow_containers(0, &[root], style).unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(doc.page_text(0).unwrap(), ["B"]);
    assert_eq!(
        doc.page_text(1)
            .unwrap()
            .concat()
            .chars()
            .filter(|ch| *ch == 'A')
            .count(),
        8
    );
}

#[test]
fn empty_floating_columns_do_not_allocate_a_partially_filled_start_page() {
    let mut doc = OfdDocument::new();
    doc.append_blank_page(0, 20., 14.).unwrap();
    let mut empty = container(Vec::new(), 4.);
    empty.layout.clear = BoxClear::None;
    let mut root = container(
        vec![
            FlowContainerChild::Container(Box::new(empty)),
            floating("B", 6., 6., BoxFloat::Right),
        ],
        16.,
    );
    root.layout.background_color = Some([220, 230, 240]);
    let mut style = geometry();
    style.start = Some(FlowStart {
        page_index: 0,
        y: 8.,
    });
    let result = doc.add_flow_containers(0, &[root], style).unwrap();
    assert_eq!(result.pages, [1]);
    assert_eq!(result.placements[0].allocation[3], 6.);
    assert!(doc.page_objects(0).unwrap().is_empty());
    assert_eq!(doc.page_text(1).unwrap(), ["B"]);
}
