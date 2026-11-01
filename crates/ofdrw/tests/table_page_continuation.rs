use ofdrw::{
    FlowStart, FlowTextAlign, OfdDocument, PageArea, TableCell, TableCellImage, TableStyle,
    TableVerticalAlign,
};

fn setup() -> (OfdDocument, TableStyle) {
    let mut document = OfdDocument::new();
    document.add_text_page("seed", 20., 20.).unwrap();
    let font = document
        .add_embedded_font("DejaVu Serif", include_bytes!("fixtures/dejavu-serif.ttf"))
        .unwrap();
    let style = TableStyle {
        start: Some(FlowStart {
            page_index: 0,
            y: 10.,
        }),
        page_size: [20.; 2],
        margins: [2.; 4],
        column_widths: vec![16.],
        row_height: 4.,
        font_size: 2.,
        line_height: 3.,
        padding: 0.5,
        font_id: Some(font.to_string()),
        repeat_header: true,
        ..Default::default()
    };
    (document, style)
}

fn rows(values: &[&str]) -> Vec<Vec<String>> {
    values
        .iter()
        .map(|value| vec![(*value).to_owned()])
        .collect()
}

fn cell(row: usize, span: usize, text: &str) -> TableCell {
    TableCell {
        row,
        column: 0,
        row_span: span,
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
        blocks: Vec::new(),
    }
}

fn position(document: &OfdDocument, page: usize, text: &str) -> [f64; 2] {
    let object = document
        .page_text_objects(page)
        .unwrap()
        .into_iter()
        .find(|object| object.codes.iter().any(|code| code.text == text))
        .unwrap();
    let [x, y, _, _] = object.boundary.unwrap();
    [x, y]
}

#[test]
fn table_reuses_available_space_and_repeats_its_header_after_overflow() {
    let (mut document, style) = setup();
    assert_eq!(
        document
            .add_table(0, &rows(&["H", "A", "B", "C"]), style)
            .unwrap(),
        [0, 1]
    );
    assert_eq!(document.page_text(0).unwrap(), ["seed", "H", "A"]);
    assert_eq!(document.page_text(1).unwrap(), ["H", "B", "C"]);
    assert_eq!(position(&document, 0, "H"), [2.5, 10.5]);
    assert_eq!(position(&document, 0, "A"), [2.5, 14.5]);
    assert_eq!(position(&document, 1, "H"), [2.5, 2.5]);
    assert_eq!(position(&document, 1, "B"), [2.5, 6.5]);
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert!(reopened.validate_references().unwrap().is_empty());
    assert_eq!(
        document.page_png(0, 72.).unwrap(),
        reopened.page_png(0, 72.).unwrap()
    );
}

#[test]
fn header_does_not_stay_alone_in_the_remaining_space() {
    let (mut document, mut style) = setup();
    style.start.as_mut().unwrap().y = 12.;
    let path = document.pages().unwrap()[0].path.clone();
    let before = document.entry(&path).unwrap().unwrap().to_vec();
    assert_eq!(
        document.add_table(0, &rows(&["H", "A"]), style).unwrap(),
        [1]
    );
    assert_eq!(document.entry(&path).unwrap().unwrap(), before);
    assert_eq!(document.page_text(0).unwrap(), ["seed"]);
    assert_eq!(document.page_text(1).unwrap(), ["H", "A"]);
}

#[test]
fn header_stays_with_the_first_complete_merged_row_group() {
    let (mut document, mut style) = setup();
    style.start.as_mut().unwrap().y = 8.;
    let cells = vec![cell(0, 1, "H"), cell(1, 2, "M"), cell(3, 1, "B")];
    assert_eq!(document.add_table_cells(0, 4, &cells, style).unwrap(), [1]);
    assert_eq!(document.page_text(0).unwrap(), ["seed"]);
    assert_eq!(document.page_text(1).unwrap(), ["H", "M", "B"]);
    assert!(document
        .page_paths(1)
        .unwrap()
        .iter()
        .any(|path| path.boundary == Some([2., 6., 16., 8.])));
    assert!(document.validate_references().unwrap().is_empty());
}

#[test]
fn canvas_origin_offsets_positions_without_shrinking_cell_widths() {
    let (mut document, style) = setup();
    document
        .set_page_area(
            0,
            PageArea {
                physical_box: [10., 20., 20., 20.],
                application_box: None,
                content_box: None,
                bleed_box: None,
            },
        )
        .unwrap();
    assert_eq!(
        document
            .add_table(0, &rows(&["H", "A", "B"]), style)
            .unwrap(),
        [0, 1]
    );
    assert_eq!(position(&document, 0, "H"), [12.5, 30.5]);
    assert_eq!(position(&document, 1, "H"), [2.5, 2.5]);
    assert!(document
        .page_paths(0)
        .unwrap()
        .iter()
        .any(|path| path.boundary == Some([12., 30., 16., 4.])));
}

#[test]
fn invalid_start_requests_leave_the_original_package_unchanged() {
    let (document, style) = setup();
    for start in [
        FlowStart {
            page_index: 1,
            y: 10.,
        },
        FlowStart {
            page_index: 0,
            y: 1.,
        },
        FlowStart {
            page_index: 0,
            y: 19.,
        },
        FlowStart {
            page_index: 0,
            y: f64::NAN,
        },
    ] {
        let mut changed = document.clone();
        let before = changed.to_bytes().unwrap();
        let mut invalid = style.clone();
        invalid.start = Some(start);
        assert!(changed.add_table(0, &rows(&["A"]), invalid).is_err());
        assert_eq!(changed.to_bytes().unwrap(), before);
    }
    let mut changed = document.clone();
    let mut invalid = style.clone();
    invalid.page_size[0] += 1.;
    let before = changed.to_bytes().unwrap();
    assert!(changed.add_table(0, &rows(&["A"]), invalid).is_err());
    assert_eq!(changed.to_bytes().unwrap(), before);
    changed.append_document_body().unwrap();
    changed.append_blank_page(1, 20., 20.).unwrap();
    let before = changed.to_bytes().unwrap();
    let mut invalid = style;
    invalid.start.as_mut().unwrap().page_index = 1;
    assert!(changed.add_table(0, &rows(&["A"]), invalid).is_err());
    assert_eq!(changed.to_bytes().unwrap(), before);
}

#[test]
fn omitting_start_keeps_the_existing_append_behavior() {
    let (mut document, mut style) = setup();
    style.start = None;
    assert_eq!(
        document.add_table(0, &rows(&["H", "A"]), style).unwrap(),
        [1]
    );
    assert_eq!(document.page_text(0).unwrap(), ["seed"]);
}

#[test]
fn image_only_tables_continue_without_font_resources() {
    let mut document = OfdDocument::new();
    document.add_blank_page(20., 20.).unwrap();
    document
        .set_entry(
            "Doc_0/PublicRes.xml",
            br#"<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res"/>"#.to_vec(),
        )
        .unwrap();
    assert!(document.fonts().unwrap().is_empty());
    let mut encoded = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let cells = (0..3)
        .map(|row| {
            let mut cell = cell(row, 1, "");
            cell.image = Some(TableCellImage {
                data: encoded.get_ref().clone(),
                format: "PNG".into(),
                width: 2.,
                height: 2.,
            });
            cell
        })
        .collect::<Vec<_>>();
    let style = TableStyle {
        start: Some(FlowStart {
            page_index: 0,
            y: 10.,
        }),
        page_size: [20.; 2],
        margins: [2.; 4],
        column_widths: vec![16.],
        row_height: 4.,
        font_size: 2.,
        line_height: 3.,
        padding: 0.5,
        ..Default::default()
    };
    assert_eq!(
        document.add_table_cells(0, 3, &cells, style).unwrap(),
        [0, 1]
    );
    assert!(document.fonts().unwrap().is_empty());
    assert_eq!(document.page_images(0).unwrap().len(), 2);
    assert_eq!(document.page_images(1).unwrap().len(), 1);
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert!(reopened.validate_references().unwrap().is_empty());
    assert_eq!(reopened.page_images(0).unwrap().len(), 2);
}

#[test]
fn continuation_in_a_later_body_preserves_other_document_pages() {
    let (mut document, mut style) = setup();
    document.append_document_body().unwrap();
    document.append_blank_page(1, 20., 20.).unwrap();
    let font = document
        .add_embedded_font_to_document(
            1,
            "DejaVu Serif",
            include_bytes!("fixtures/dejavu-serif.ttf"),
        )
        .unwrap();
    document.append_document_body().unwrap();
    document.append_blank_page(2, 20., 20.).unwrap();
    let later = document.pages().unwrap()[2].path.clone();
    let before = document.entry(&later).unwrap().unwrap().to_vec();
    style.start = Some(FlowStart {
        page_index: 1,
        y: 10.,
    });
    style.font_id = Some(font.to_string());
    assert_eq!(
        document
            .add_table(1, &rows(&["H", "A", "B"]), style)
            .unwrap(),
        [1, 2]
    );
    assert_eq!(document.pages().unwrap()[3].document_index, 2);
    assert_eq!(document.entry(&later).unwrap().unwrap(), before);
    assert!(document.validate_references().unwrap().is_empty());
}
