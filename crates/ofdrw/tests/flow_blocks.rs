use ofdrw::{
    FlowBlock, FlowDocumentStyle, FlowParagraphStyle, FlowSpan, FlowStart, FlowTextAlign,
    FlowTextStyle, NestedTable, OfdDocument, PageArea, TableCell, TableCellImage,
    TableVerticalAlign,
};

fn setup() -> (OfdDocument, FlowDocumentStyle) {
    let mut document = OfdDocument::new();
    let font = document
        .add_embedded_font("DejaVu Serif", include_bytes!("fixtures/dejavu-serif.ttf"))
        .unwrap();
    let style = FlowDocumentStyle {
        text: FlowTextStyle {
            page_size: [20.; 2],
            margins: [2.; 4],
            font_size: 2.,
            line_height: 3.,
            font_id: Some(font.to_string()),
            ..Default::default()
        },
        block_gap: 1.,
    };
    (document, style)
}

fn image(width: f64, height: f64, align: FlowTextAlign) -> FlowBlock {
    let mut data = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 2, image::Rgb([200, 40, 20])))
        .write_to(&mut data, image::ImageFormat::Png)
        .unwrap();
    FlowBlock::Image {
        image: TableCellImage {
            data: data.into_inner(),
            format: "PNG".into(),
            width,
            height,
        },
        align,
    }
}

fn table(values: &[&str], repeat_header: bool) -> FlowBlock {
    FlowBlock::Table {
        table: NestedTable {
            row_count: values.len(),
            column_widths: vec![16.],
            row_height: 4.,
            padding: 0.,
            border_width: 0.2,
            border_color: [0; 3],
            cells: values
                .iter()
                .enumerate()
                .map(|(row, text)| TableCell {
                    row,
                    column: 0,
                    row_span: 1,
                    column_span: 1,
                    text: (*text).into(),
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
                })
                .collect(),
        },
        repeat_header,
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
fn paragraphs_images_and_tables_share_one_cursor_across_pages() {
    let (mut document, style) = setup();
    let result = document
        .add_flow_blocks(
            0,
            &[
                FlowBlock::paragraph("A"),
                image(6., 4., FlowTextAlign::Center),
                table(&["R1", "R2"], false),
                FlowBlock::paragraph("B"),
            ],
            style.clone(),
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(
        result.next,
        Some(FlowStart {
            page_index: 1,
            y: 10.
        })
    );
    assert_eq!(position(&document, 0, "A"), [2., 2.]);
    assert_eq!(
        document.page_images(0).unwrap()[0].boundary,
        Some([7., 6., 6., 4.])
    );
    assert_eq!(position(&document, 0, "R1"), [2., 11.]);
    assert_eq!(position(&document, 1, "R2"), [2., 2.]);
    assert_eq!(position(&document, 1, "B"), [2., 7.]);
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert!(reopened.validate_references().unwrap().is_empty());
    assert_eq!(
        reopened.page_png(0, 72.).unwrap(),
        document.page_png(0, 72.).unwrap()
    );
    let mut continued = style;
    continued.text.start = result.next;
    document
        .add_flow_blocks(0, &[FlowBlock::paragraph("C")], continued)
        .unwrap();
    assert_eq!(position(&document, 1, "C"), [2., 10.]);
}

#[test]
fn blank_lines_and_underline_do_not_confuse_the_continuation_position() {
    for shape in [false, true] {
        let (mut document, mut style) = setup();
        style.text.shape = shape;
        let mut span = FlowSpan::new("A\n\nB");
        span.underline = true;
        let result = document
            .add_flow_blocks(
                0,
                &[
                    FlowBlock::Paragraph {
                        spans: vec![span],
                        style: None,
                    },
                    FlowBlock::paragraph("C"),
                ],
                style,
            )
            .unwrap();
        assert_eq!(position(&document, 0, "B"), [2., 8.]);
        assert_eq!(position(&document, 0, "C"), [2., 12.]);
        assert_eq!(result.next.unwrap().y, 15.);
    }
}

#[test]
fn paragraph_overrides_preserve_leading_and_do_not_leak_to_the_next_block() {
    let (mut document, style) = setup();
    let result = document
        .add_flow_blocks(
            0,
            &[
                FlowBlock::Paragraph {
                    spans: vec![FlowSpan::new("A")],
                    style: Some(Box::new(FlowParagraphStyle {
                        font_size: Some(4.),
                        align: Some(FlowTextAlign::Right),
                        ..Default::default()
                    })),
                },
                FlowBlock::paragraph("B"),
            ],
            style,
        )
        .unwrap();
    assert_eq!(position(&document, 0, "B"), [2., 8.]);
    assert_eq!(result.next.unwrap().y, 11.);
    let objects = document.page_text_objects(0).unwrap();
    assert_eq!(objects[0].size, Some(4.));
    assert_eq!(objects[1].size, Some(2.));
    assert!(objects[0].boundary.unwrap()[0] > 2.);
}

#[test]
fn disabling_shaping_for_one_paragraph_clears_its_inherited_language_only() {
    let (mut document, mut style) = setup();
    style.text.shape = true;
    style.text.language = Some("en".into());
    document
        .add_flow_blocks(
            0,
            &[
                FlowBlock::Paragraph {
                    spans: vec![FlowSpan::new("fi")],
                    style: Some(Box::new(FlowParagraphStyle {
                        shape: Some(false),
                        ..Default::default()
                    })),
                },
                FlowBlock::paragraph("fi"),
            ],
            style,
        )
        .unwrap();
    let objects = document.page_text_objects(0).unwrap();
    assert!(objects[0].cg_transforms.is_empty());
    assert_eq!(objects[1].cg_transforms[0].code_count, 2);
    assert_eq!(objects[1].cg_transforms[0].glyph_count, 1);
}

#[test]
fn repeated_header_moves_with_the_first_body_row_and_returns_last_row_end() {
    let (mut document, mut style) = setup();
    document.add_text_page("seed", 20., 20.).unwrap();
    style.text.start = Some(FlowStart {
        page_index: 0,
        y: 13.,
    });
    let result = document
        .add_flow_blocks(
            0,
            &[table(&["H", "A", "B"], true), FlowBlock::paragraph("C")],
            style,
        )
        .unwrap();
    assert_eq!(result.pages, [1]);
    assert_eq!(document.page_text(0).unwrap(), ["seed"]);
    assert_eq!(position(&document, 1, "H"), [2., 2.]);
    assert_eq!(position(&document, 1, "C"), [2., 15.]);
    assert_eq!(
        result.next,
        Some(FlowStart {
            page_index: 1,
            y: 18.
        })
    );
}

#[test]
fn image_moves_intact_to_a_new_page_and_discards_interblock_gap() {
    let (mut document, mut style) = setup();
    document.append_blank_page(0, 20., 20.).unwrap();
    style.text.start = Some(FlowStart {
        page_index: 0,
        y: 14.,
    });
    let result = document
        .add_flow_blocks(
            0,
            &[
                FlowBlock::paragraph("A"),
                image(4., 5., FlowTextAlign::Right),
            ],
            style,
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(
        document.page_images(1).unwrap()[0].boundary,
        Some([14., 2., 4., 5.])
    );
    assert_eq!(result.next.unwrap().y, 7.);
}

#[test]
fn nonzero_canvas_origin_is_applied_to_content_and_removed_from_next_position() {
    let (mut document, mut style) = setup();
    document.append_blank_page(0, 20., 20.).unwrap();
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
    style.text.start = Some(FlowStart {
        page_index: 0,
        y: 4.,
    });
    let result = document
        .add_flow_blocks(
            0,
            &[
                image(4., 2., FlowTextAlign::Left),
                table(&["T"], false),
                FlowBlock::paragraph("A"),
            ],
            style,
        )
        .unwrap();
    assert_eq!(
        document.page_images(0).unwrap()[0].boundary,
        Some([12., 24., 4., 2.])
    );
    assert_eq!(position(&document, 0, "T"), [12., 27.]);
    assert_eq!(position(&document, 0, "A"), [12., 32.]);
    assert_eq!(result.next.unwrap().y, 15.);
}

#[test]
fn spacer_crosses_pages_without_reusing_a_full_start_page() {
    let (mut document, mut style) = setup();
    document.append_blank_page(0, 20., 20.).unwrap();
    style.text.start = Some(FlowStart {
        page_index: 0,
        y: 18.,
    });
    let result = document
        .add_flow_blocks(
            0,
            &[FlowBlock::Spacer { height: 19. }, FlowBlock::paragraph("A")],
            style,
        )
        .unwrap();
    assert_eq!(result.pages, [1, 2]);
    assert_eq!(position(&document, 2, "A"), [2., 5.]);
    assert_eq!(result.next.unwrap().y, 8.);
}

#[test]
fn page_breaks_preserve_explicit_blank_pages_and_reset_block_gap() {
    let (mut document, style) = setup();
    let result = document
        .add_flow_blocks(
            0,
            &[
                FlowBlock::PageBreak,
                FlowBlock::paragraph("A"),
                FlowBlock::PageBreak,
                FlowBlock::PageBreak,
                FlowBlock::paragraph("B"),
                FlowBlock::PageBreak,
            ],
            style,
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1, 2, 3]);
    assert_eq!(position(&document, 0, "A"), [2., 2.]);
    assert!(document.page_text(1).unwrap().is_empty());
    assert_eq!(position(&document, 2, "B"), [2., 2.]);
    assert_eq!(
        result.next,
        Some(FlowStart {
            page_index: 3,
            y: 2.
        })
    );
}

#[test]
fn later_image_failure_rolls_back_prior_text_resources_and_pages() {
    let (mut document, style) = setup();
    let before = document.to_bytes().unwrap();
    let mut bad_image = image(4., 4., FlowTextAlign::Left);
    if let FlowBlock::Image { image, .. } = &mut bad_image {
        image.data.clear();
    }
    assert!(document
        .add_flow_blocks(0, &[FlowBlock::paragraph("A"), bad_image], style)
        .is_err());
    assert_eq!(document.to_bytes().unwrap(), before);
}

#[test]
fn later_invalid_table_rolls_back_an_already_placed_image() {
    let (mut document, style) = setup();
    let before = document.to_bytes().unwrap();
    let mut bad_table = table(&["A"], false);
    if let FlowBlock::Table { table, .. } = &mut bad_table {
        table.cells.clear();
    }
    assert!(document
        .add_flow_blocks(0, &[image(4., 4., FlowTextAlign::Left), bad_table], style)
        .is_err());
    assert_eq!(document.to_bytes().unwrap(), before);
}

#[test]
fn invalid_geometry_oversized_images_and_unbounded_spacers_fail_atomically() {
    for block in [
        image(17., 1., FlowTextAlign::Left),
        image(1., 17., FlowTextAlign::Left),
        image(f64::NAN, 1., FlowTextAlign::Left),
        FlowBlock::Spacer { height: -1. },
        FlowBlock::Spacer { height: 0. },
        FlowBlock::Spacer {
            height: f64::INFINITY,
        },
        FlowBlock::Spacer { height: 1e308 },
        FlowBlock::paragraph(""),
    ] {
        let (mut document, style) = setup();
        let before = document.to_bytes().unwrap();
        assert!(document
            .add_flow_blocks(0, &[FlowBlock::paragraph("A"), block], style)
            .is_err());
        assert_eq!(document.to_bytes().unwrap(), before);
    }
    for gap in [-1., f64::NAN, f64::INFINITY] {
        let (mut document, mut style) = setup();
        style.block_gap = gap;
        assert!(document
            .add_flow_blocks(0, &[FlowBlock::paragraph("A")], style)
            .is_err());
        assert!(document.pages().unwrap().is_empty());
    }
}

#[test]
fn empty_flow_returns_the_selected_cursor_without_creating_a_page() {
    let (mut document, mut style) = setup();
    let before = document.to_bytes().unwrap();
    let result = document.add_flow_blocks(0, &[], style.clone()).unwrap();
    assert!(result.pages.is_empty());
    assert!(result.next.is_none());
    assert_eq!(document.to_bytes().unwrap(), before);
    document.append_blank_page(0, 20., 20.).unwrap();
    style.text.start = Some(FlowStart {
        page_index: 0,
        y: 12.,
    });
    assert_eq!(
        document
            .add_flow_blocks(0, &[], style.clone())
            .unwrap()
            .next,
        style.text.start
    );
    assert!(document.add_flow_blocks(1, &[], style).is_err());
}

#[test]
fn an_image_only_flow_does_not_require_valid_text_typography_or_a_font() {
    let mut document = OfdDocument::new();
    document
        .set_entry(
            "Doc_0/PublicRes.xml",
            br#"<ofd:Res xmlns:ofd="http://www.ofdspec.org/2016" BaseLoc="Res"/>"#.to_vec(),
        )
        .unwrap();
    let style = FlowDocumentStyle {
        text: FlowTextStyle {
            page_size: [20.; 2],
            margins: [2.; 4],
            font_id: Some("missing".into()),
            font_size: 99.,
            line_height: 100.,
            ..Default::default()
        },
        block_gap: 0.,
    };
    let result = document
        .add_flow_blocks(0, &[image(4., 5., FlowTextAlign::Left)], style)
        .unwrap();
    assert_eq!(result.next.unwrap().y, 7.);
    assert!(document.fonts().unwrap().is_empty());
    assert!(document.validate_references().unwrap().is_empty());
}

#[test]
fn a_later_body_uses_global_page_indexes_and_preserves_following_bodies() {
    let (mut document, mut style) = setup();
    document.append_blank_page(0, 20., 20.).unwrap();
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
    let following = document.pages().unwrap()[2].path.clone();
    let bytes = document.entry(&following).unwrap().unwrap().to_vec();
    style.text.font_id = Some(font.to_string());
    style.text.start = Some(FlowStart {
        page_index: 1,
        y: 14.,
    });
    let result = document
        .add_flow_blocks(
            1,
            &[
                FlowBlock::paragraph("A"),
                image(4., 5., FlowTextAlign::Left),
            ],
            style,
        )
        .unwrap();
    assert_eq!(result.pages, [1, 2]);
    assert_eq!(
        result.next,
        Some(FlowStart {
            page_index: 2,
            y: 7.
        })
    );
    assert_eq!(document.pages().unwrap()[3].document_index, 2);
    assert_eq!(document.entry(&following).unwrap().unwrap(), bytes);
    assert!(document.validate_references().unwrap().is_empty());
}
