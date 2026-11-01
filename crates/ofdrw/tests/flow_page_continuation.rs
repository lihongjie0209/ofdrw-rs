use ofdrw::{FlowSpan, FlowStart, FlowTextStyle, OfdDocument, PageArea};

fn setup(width: f64) -> (OfdDocument, FlowTextStyle) {
    let mut document = OfdDocument::new();
    document.add_text_page("seed", width, 20.).unwrap();
    let font = document
        .add_embedded_font("DejaVu Serif", include_bytes!("fixtures/dejavu-serif.ttf"))
        .unwrap();
    let style = FlowTextStyle {
        page_size: [width, 20.],
        margins: [2.; 4],
        font_size: 2.,
        line_height: 3.,
        font_id: Some(font.to_string()),
        start: Some(FlowStart {
            page_index: 0,
            y: 10.,
        }),
        ..Default::default()
    };
    (document, style)
}

fn flow(
    document: &mut OfdDocument,
    text: &str,
    mut style: FlowTextStyle,
    mode: usize,
) -> Result<Vec<usize>, ofdrw::OfdError> {
    style.shape = mode >= 2;
    if mode.is_multiple_of(2) {
        document.add_flowed_text(0, text, style)
    } else {
        document.add_flowed_spans(0, &[FlowSpan::new(text)], style)
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
fn plain_styled_and_shaped_flow_reuse_the_start_page_then_reset_margins() {
    for mode in 0..4 {
        let (mut document, style) = setup(20.);
        assert_eq!(
            flow(&mut document, "A\nB\nC\nD", style, mode).unwrap(),
            [0, 1]
        );
        assert_eq!(document.pages().unwrap().len(), 2);
        assert_eq!(position(&document, 0, "A"), [2., 10.]);
        assert_eq!(position(&document, 0, "B"), [2., 13.]);
        assert_eq!(position(&document, 1, "C"), [2., 2.]);
        assert_eq!(position(&document, 1, "D"), [2., 5.]);
        assert_eq!(document.page_text(0).unwrap()[0], "seed");
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(position(&reopened, 0, "A"), [2., 10.]);
        assert!(reopened.validate_references().unwrap().is_empty());
        assert_eq!(
            document.page_png(0, 72.).unwrap(),
            reopened.page_png(0, 72.).unwrap()
        );
    }
}

#[test]
fn nonzero_canvas_origins_apply_only_to_the_existing_page() {
    for mode in 0..4 {
        let (mut document, style) = setup(20.);
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
        assert_eq!(flow(&mut document, "A\nB\nC", style, mode).unwrap(), [0, 1]);
        assert_eq!(position(&document, 0, "A"), [12., 30.]);
        assert_eq!(position(&document, 0, "B"), [12., 33.]);
        assert_eq!(position(&document, 1, "C"), [2., 2.]);
    }
}

#[test]
fn a_full_start_page_is_untouched_and_not_reported_as_used() {
    for mode in 0..4 {
        let (mut document, mut style) = setup(20.);
        let path = document.pages().unwrap()[0].path.clone();
        let before = document.entry(&path).unwrap().unwrap().to_vec();
        style.start.as_mut().unwrap().y = 18.;
        assert_eq!(flow(&mut document, "A", style, mode).unwrap(), [1]);
        assert_eq!(document.entry(&path).unwrap().unwrap(), before);
        assert_eq!(position(&document, 1, "A"), [2., 2.]);
    }
}

#[test]
fn invalid_start_geometry_and_cross_document_pages_leave_the_source_unchanged() {
    for mode in 0..4 {
        let (document, style) = setup(20.);
        for start in [
            FlowStart {
                page_index: 1,
                y: 10.,
            },
            FlowStart {
                page_index: 0,
                y: -1.,
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
            assert!(flow(&mut changed, "A", invalid, mode).is_err());
            assert_eq!(changed.to_bytes().unwrap(), before);
        }
        let mut changed = document.clone();
        let mut invalid = style.clone();
        invalid.page_size[0] += 1.;
        let before = changed.to_bytes().unwrap();
        assert!(flow(&mut changed, "A", invalid, mode).is_err());
        assert_eq!(changed.to_bytes().unwrap(), before);
        changed.append_document_body().unwrap();
        changed.append_blank_page(1, 20., 20.).unwrap();
        let before = changed.to_bytes().unwrap();
        let mut invalid = style.clone();
        invalid.start.as_mut().unwrap().page_index = 1;
        assert!(flow(&mut changed, "A", invalid, mode).is_err());
        assert_eq!(changed.to_bytes().unwrap(), before);
    }
}

#[test]
fn paragraph_gaps_do_not_offset_the_requested_first_position() {
    let (mut document, mut style) = setup(20.);
    style.paragraph_gap = 2.;
    assert_eq!(flow(&mut document, "A\nB", style, 0).unwrap(), [0]);
    assert_eq!(position(&document, 0, "A"), [2., 10.]);
    assert_eq!(position(&document, 0, "B"), [2., 15.]);
}

#[test]
fn kept_paragraphs_move_as_a_unit_when_the_remaining_page_is_too_short() {
    for mode in 0..4 {
        let (mut document, mut style) = setup(8.);
        style.keep_paragraphs_together = true;
        assert_eq!(flow(&mut document, "AAAAAAAA", style, mode).unwrap(), [1]);
        assert_eq!(document.page_text(0).unwrap(), ["seed"]);
    }
}

#[test]
fn continuation_uses_global_page_indexes_in_a_later_document_body() {
    for mode in 0usize..4 {
        let (mut document, mut style) = setup(20.);
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
        let later_path = document.pages().unwrap()[2].path.clone();
        let later_bytes = document.entry(&later_path).unwrap().unwrap().to_vec();
        style.font_id = Some(font.to_string());
        style.start = Some(FlowStart {
            page_index: 1,
            y: 10.,
        });
        style.shape = mode >= 2;
        let pages = if mode.is_multiple_of(2) {
            document.add_flowed_text(1, "A\nB\nC", style)
        } else {
            document.add_flowed_spans(1, &[FlowSpan::new("A\nB\nC")], style)
        }
        .unwrap();
        assert_eq!(pages, [1, 2]);
        assert_eq!(position(&document, 1, "A"), [2., 10.]);
        assert_eq!(position(&document, 2, "C"), [2., 2.]);
        assert_eq!(document.pages().unwrap()[3].document_index, 2);
        assert_eq!(document.entry(&later_path).unwrap().unwrap(), later_bytes);
        assert_eq!(document.page_text(0).unwrap(), ["seed"]);
        assert!(document.validate_references().unwrap().is_empty());
    }
}
