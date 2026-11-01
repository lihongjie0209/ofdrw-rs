use ofdrw::{
    BoxLayoutStyle, FlowContainer, FlowContainerChild, FlowSpan, FlowTextAlign, FlowTextStyle,
    LayoutBox, NestedTable, OfdDocument, TableCell, TableStyle, TableVerticalAlign,
};

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
fn table(values: &[&str]) -> NestedTable {
    NestedTable {
        row_count: values.len(),
        column_widths: vec![10.],
        cells: values
            .iter()
            .enumerate()
            .map(|(row, text)| cell(row, 0, text))
            .collect(),
        row_height: 4.,
        padding: 0.,
        border_width: 0.2,
        border_color: [0; 3],
    }
}
fn typography() -> TableStyle {
    TableStyle {
        font_size: 2.,
        line_height: 3.,
        ..Default::default()
    }
}
fn paragraph(text: &str) -> FlowContainerChild {
    FlowContainerChild::Paragraph {
        spans: vec![FlowSpan::new(text)],
        style: Box::new(FlowTextStyle {
            font_size: 2.,
            line_height: 3.,
            ..Default::default()
        }),
    }
}
fn child(table: NestedTable) -> FlowContainerChild {
    FlowContainerChild::Table {
        table: Box::new(table),
        style: Box::new(typography()),
        align: FlowTextAlign::Right,
    }
}
fn root(children: Vec<FlowContainerChild>) -> FlowContainer {
    FlowContainer {
        layout: LayoutBox {
            size: [12., 0.],
            ..Default::default()
        },
        children,
        gap: 0.,
    }
}
fn geometry() -> BoxLayoutStyle {
    BoxLayoutStyle {
        page_size: [20., 14.],
        margins: [2.; 4],
        start: None,
    }
}

#[test]
fn table_children_split_between_rows_and_continue_to_following_paragraphs() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_flow_containers(
            0,
            &[root(vec![
                paragraph("A"),
                child(table(&["R1", "R2", "R3", "R4"])),
                paragraph("Z"),
            ])],
            geometry(),
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1, 2]);
    assert_eq!(doc.page_text(0).unwrap(), ["A", "R1"]);
    assert_eq!(doc.page_text(1).unwrap(), ["R2", "R3"]);
    assert_eq!(doc.page_text(2).unwrap(), ["R4", "Z"]);
    assert_eq!(
        doc.page_text_objects(1).unwrap()[0].boundary.unwrap()[..2],
        [4., 2.]
    );
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(2).unwrap(), ["R4", "Z"]);
    assert!(!reopened.page_png(1, 72.).unwrap().is_empty());
}

#[test]
fn row_spans_protect_complete_merged_groups() {
    let mut doc = OfdDocument::new();
    let mut merged = cell(0, 0, "M");
    merged.row_span = 2;
    let mut last = cell(2, 0, "R3");
    last.column_span = 2;
    let table = NestedTable {
        row_count: 3,
        column_widths: vec![5., 5.],
        cells: vec![merged, cell(0, 1, "R1"), cell(1, 1, "R2"), last],
        row_height: 4.,
        padding: 0.,
        border_width: 0.2,
        border_color: [0; 3],
    };
    let result = doc
        .add_flow_containers(
            0,
            &[root(vec![paragraph("A"), child(table), paragraph("Z")])],
            geometry(),
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1, 2]);
    assert_eq!(doc.page_text(0).unwrap(), ["A"]);
    assert_eq!(doc.page_text(1).unwrap(), ["M", "R1", "R2"]);
    assert_eq!(doc.page_text(2).unwrap(), ["R3", "Z"]);
}

#[test]
fn container_table_raster_matches_existing_table_drawing() {
    let mut doc = OfdDocument::new();
    let mut reference = doc.clone();
    let mut table = table(&["AV", "Text"]);
    table.cells[0].background_color = Some([220, 240, 230]);
    table.cells[0].bold = true;
    table.cells[0].underline = true;
    let geometry = BoxLayoutStyle {
        page_size: [20., 30.],
        margins: [2.; 4],
        start: None,
    };
    let mut style = typography();
    style.page_size = geometry.page_size;
    style.margins = [2., 6., 2., 4.];
    style.column_widths = table.column_widths.clone();
    style.row_height = table.row_height;
    style.padding = table.padding;
    style.border_width = table.border_width;
    reference
        .add_table_cells(0, table.row_count, &table.cells, style)
        .unwrap();
    doc.add_flow_containers(0, &[root(vec![child(table)])], geometry)
        .unwrap();
    assert_eq!(doc.page_text(0).unwrap(), reference.page_text(0).unwrap());
    assert_eq!(
        doc.page_png(0, 72.).unwrap(),
        reference.page_png(0, 72.).unwrap()
    );
}

#[test]
fn invalid_table_typography_grid_and_oversized_groups_roll_back() {
    let mut doc = OfdDocument::new();
    let before = doc.to_bytes().unwrap();
    let mut too_tall = table(&["A"]);
    too_tall.row_height = 11.;
    let mut gap = table(&["A", "B"]);
    gap.cells.pop();
    for bad in [too_tall, gap] {
        assert!(doc
            .add_flow_containers(0, &[root(vec![paragraph("A"), child(bad)])], geometry())
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    for bad_style in [
        TableStyle {
            font_id: Some("missing".into()),
            ..typography()
        },
        TableStyle {
            line_height: f64::NAN,
            ..typography()
        },
    ] {
        let bad = FlowContainerChild::Table {
            table: Box::new(table(&["A"])),
            style: Box::new(bad_style),
            align: FlowTextAlign::Left,
        };
        assert!(doc
            .add_flow_containers(0, &[root(vec![bad])], geometry())
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
}

#[test]
fn fractional_nested_table_geometry_preserves_every_row_once() {
    for index in 1..20 {
        let mut doc = OfdDocument::new();
        let mut table = table(&["A", "B", "C", "D"]);
        table.row_height = 3.3;
        table.padding = 0.11;
        let mut typography = typography();
        typography.font_size = 1.1;
        typography.line_height = 2.2;
        let table_child = FlowContainerChild::Table {
            table: Box::new(table),
            style: Box::new(typography),
            align: FlowTextAlign::Center,
        };
        let mut nested = root(vec![table_child]);
        nested.layout.padding = [f64::from(index) / 100.; 4];
        let result = doc
            .add_flow_containers(
                0,
                &[root(vec![FlowContainerChild::Container(Box::new(nested))])],
                geometry(),
            )
            .unwrap();
        let text = result
            .pages
            .iter()
            .flat_map(|page| doc.page_text(*page).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(text, ["A", "B", "C", "D"]);
    }
}

#[test]
fn table_children_reuse_image_and_nested_table_cell_rendering() {
    let mut image_bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 2, image::Rgb([200, 20, 30])))
        .write_to(&mut image_bytes, image::ImageFormat::Png)
        .unwrap();
    let mut picture_table = table(&[""]);
    picture_table.cells[0].image = Some(ofdrw::TableCellImage {
        data: image_bytes.into_inner(),
        format: "PNG".into(),
        width: 2.,
        height: 2.,
    });
    let mut doc = OfdDocument::new();
    let image_only = FlowContainerChild::Table {
        table: Box::new(picture_table),
        style: Box::new(TableStyle {
            font_id: Some("unused-missing-font".into()),
            ..typography()
        }),
        align: FlowTextAlign::Left,
    };
    doc.add_flow_containers(0, &[root(vec![image_only])], geometry())
        .unwrap();
    assert_eq!(doc.page_images(0).unwrap().len(), 1);
    assert!(doc.page_text(0).unwrap().is_empty());

    let mut nested = table(&["R1", "R2"]);
    nested.row_height = 3.;
    nested.column_widths = vec![8.];
    let mut outer = table(&[""]);
    outer.row_height = 0.;
    outer.cells[0].blocks = vec![ofdrw::TableCellBlock::Table(Box::new(nested))];
    let mut doc = OfdDocument::new();
    let result = doc
        .add_flow_containers(
            0,
            &[root(vec![paragraph("A"), child(outer), paragraph("Z")])],
            geometry(),
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(doc.page_text(0).unwrap(), ["A", "R1", "R2"]);
    assert_eq!(doc.page_text(1).unwrap(), ["Z"]);
}
