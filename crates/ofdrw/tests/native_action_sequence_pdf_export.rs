use ofdrw::{DestinationInfo, OfdDocument, OfdError, PathStyle};

fn source(regions: bool, template: bool) -> (OfdDocument, String, usize) {
    let mut document = OfdDocument::new();
    for _ in 0..3 {
        document.add_blank_page(30., 20.).unwrap();
    }
    let object = document
        .add_path_object(
            0,
            "M 0 0 L 10 0 L 10 5 C",
            [2., 3., 10., 5.],
            PathStyle::default(),
        )
        .unwrap();
    let destination = DestinationInfo {
        page_id: document.pages().unwrap()[2].id.clone(),
        kind: "Fit".into(),
        left: None,
        top: None,
        right: None,
        bottom: None,
        zoom: None,
    };
    document
        .add_bookmark(
            0,
            "last",
            DestinationInfo {
                page_id: document.pages().unwrap()[1].id.clone(),
                ..destination.clone()
            },
        )
        .unwrap();
    document
        .add_page_object_uri_action(0, &object, "https://example.org/first")
        .unwrap();
    document
        .add_page_object_goto_action(0, &object, destination)
        .unwrap();
    document
        .add_page_object_bookmark_action(0, &object, "last")
        .unwrap();
    document
        .add_page_object_uri_action(0, &object, "https://example.org/first")
        .unwrap();
    if regions {
        for index in 0..4 {
            document
                .set_page_object_action_regions(
                    0,
                    &object,
                    index,
                    &[[3., 4., 4., 2.], [8., 4., 2., 2.]],
                )
                .unwrap();
        }
    }
    if template {
        document
            .promote_page_to_template(0, &[1], "Background")
            .unwrap();
    }
    (document, object, usize::from(template))
}

fn check(regions: bool, template: bool) {
    let (document, _, page_index) = source(regions, template);
    let document = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    let bytes = document.to_pdf().unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let pages = pdf.get_pages();
    let page = pdf.get_dictionary(pages[&(page_index as u32 + 1)]).unwrap();
    let annotations = page.get(b"Annots").unwrap().as_array().unwrap();
    assert_eq!(annotations.len(), 1);
    let annotation = pdf
        .dereference(&annotations[0])
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    if regions {
        assert_eq!(
            annotation
                .get(b"QuadPoints")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            16
        );
    }
    let action = annotation.get(b"A").unwrap().as_dict().unwrap();
    assert_eq!(
        action.get(b"URI").unwrap().as_str().unwrap(),
        b"https://example.org/first"
    );
    let next = action.get(b"Next").unwrap().as_array().unwrap();
    assert_eq!(next.len(), 3);
    for (item, page_number) in next[..2].iter().zip([3, 2]) {
        let destination = item
            .as_dict()
            .unwrap()
            .get(b"D")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(destination[0].as_reference().unwrap(), pages[&page_number]);
        assert_eq!(destination[1].as_name().unwrap(), b"Fit");
    }
    assert_eq!(
        next[2]
            .as_dict()
            .unwrap()
            .get(b"URI")
            .unwrap()
            .as_str()
            .unwrap(),
        b"https://example.org/first"
    );
    let imported = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    let annotations = imported.annotations().unwrap();
    assert_eq!(annotations.len(), if template { 2 } else { 1 });
    assert_eq!(
        annotations[0]
            .actions
            .iter()
            .map(|action| action.kind.as_str())
            .collect::<Vec<_>>(),
        ["URI", "Goto", "Goto", "URI"]
    );
    assert!(imported.validate_references().unwrap().is_empty());
}

#[test]
fn native_page_sequences_keep_order_bookmarks_and_duplicate_actions() {
    for regions in [false, true] {
        check(regions, false);
    }
}

#[test]
fn template_sequences_export_on_the_using_page() {
    for regions in [false, true] {
        check(regions, true);
    }
}

#[test]
fn overlapping_rectangles_execute_only_actions_active_at_each_location() {
    let (mut document, object, _) = source(false, false);
    document
        .set_page_object_action_regions(0, &object, 1, &[[5., 3., 6., 4.]])
        .unwrap();
    let bytes = document.to_pdf().unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
    let annotations = page.get(b"Annots").unwrap().as_array().unwrap();
    for (x, y, expected) in [(3., 4., 3), (6., 4., 4), (11.5, 4., 3), (6., 7.5, 3)] {
        let x = x * 72. / 25.4;
        let y = (20. - y) * 72. / 25.4;
        let active = annotations
            .iter()
            .filter_map(|value| {
                let annotation = pdf.dereference(value).unwrap().1.as_dict().unwrap();
                let rect = annotation.get(b"Rect").unwrap().as_array().unwrap();
                let rect = rect
                    .iter()
                    .map(|value| f64::from(value.as_float().unwrap()))
                    .collect::<Vec<_>>();
                (x > rect[0] && x < rect[2] && y > rect[1] && y < rect[3]).then_some(annotation)
            })
            .collect::<Vec<_>>();
        assert_eq!(active.len(), 1, "click {x} {y}");
        let action = active[0].get(b"A").unwrap().as_dict().unwrap();
        assert_eq!(
            1 + action.get(b"Next").unwrap().as_array().unwrap().len(),
            expected
        );
    }
    let imported = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    assert!(imported.validate_references().unwrap().is_empty());
}

#[test]
fn different_nonrectangular_regions_are_reported_without_editing_the_source() {
    let (mut document, object, _) = source(false, false);
    document
        .set_page_object_action_regions(0, &object, 1, &[[3., 4., 2., 2.]])
        .unwrap();
    let path = document.pages().unwrap()[0].path.clone();
    let xml = String::from_utf8(document.entry(&path).unwrap().unwrap().to_vec()).unwrap();
    assert!(xml.contains("Point1=\"5 4\""));
    document
        .set_entry(
            &path,
            xml.replacen("Point1=\"5 4\"", "Point1=\"4.5 4\"", 1)
                .into_bytes(),
        )
        .unwrap();
    let before = document.to_bytes().unwrap();
    assert!(
        matches!(document.to_pdf(), Err(OfdError::Invalid(message)) if message.contains("must be rectangular"))
    );
    assert_eq!(document.to_bytes().unwrap(), before);
}

#[test]
fn partition_rectangle_and_cell_counts_are_bounded() {
    for (count, expected) in [(1_001, "1000 rectangles"), (51, "10000 cells")] {
        let (mut document, object, _) = source(false, false);
        let rectangles = (0..count)
            .map(|index| {
                let offset = if count == 51 { index as f64 * 2. } else { 3. };
                [offset, offset, 1., 1.]
            })
            .collect::<Vec<_>>();
        document
            .set_page_object_action_regions(0, &object, 0, &rectangles)
            .unwrap();
        assert!(
            matches!(document.to_pdf(), Err(OfdError::Limit(message)) if message.contains(expected))
        );
    }
}

#[test]
fn overlapping_areas_of_one_action_execute_that_action_once() {
    let (mut document, object, _) = source(false, false);
    document
        .set_page_object_action_regions(0, &object, 0, &[[3., 4., 4., 2.], [5., 4., 4., 2.]])
        .unwrap();
    let pdf = lopdf::Document::load_mem(&document.to_pdf().unwrap()).unwrap();
    let page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
    let x = 6. * 72. / 25.4;
    let y = 15. * 72. / 25.4;
    let active = page
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|value| {
            let annotation = pdf.dereference(value).unwrap().1.as_dict().unwrap();
            let rect = annotation.get(b"Rect").unwrap().as_array().unwrap();
            let rect = rect
                .iter()
                .map(|value| f64::from(value.as_float().unwrap()))
                .collect::<Vec<_>>();
            (x > rect[0] && x < rect[2] && y > rect[1] && y < rect[3]).then_some(annotation)
        })
        .collect::<Vec<_>>();
    assert_eq!(active.len(), 1);
    assert_eq!(
        active[0]
            .get(b"A")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Next")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn disjoint_attachment_and_uri_regions_remain_independent_links() {
    let mut document = OfdDocument::new();
    document.add_blank_page(30., 20.).unwrap();
    let object = document
        .add_path_object(0, "M 0 0 L 10 10", [0., 0., 10., 10.], PathStyle::default())
        .unwrap();
    document.add_attachment("link.txt", b"payload").unwrap();
    let attachment = document.attachments().unwrap()[0].id.clone();
    document
        .add_page_object_attachment_action(0, &object, &attachment, None)
        .unwrap();
    document
        .add_page_object_uri_action(0, &object, "https://example.org/")
        .unwrap();
    document
        .set_page_object_action_regions(0, &object, 0, &[[0., 0., 4., 4.]])
        .unwrap();
    document
        .set_page_object_action_regions(0, &object, 1, &[[6., 0., 4., 4.]])
        .unwrap();
    let pdf = lopdf::Document::load_mem(&document.to_pdf().unwrap()).unwrap();
    let page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
    let mut kinds = page
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            pdf.dereference(value)
                .unwrap()
                .1
                .as_dict()
                .unwrap()
                .get(b"Subtype")
                .unwrap()
                .as_name()
                .unwrap()
        })
        .collect::<Vec<_>>();
    kinds.sort();
    assert_eq!(kinds, [b"FileAttachment".as_slice(), b"Link".as_slice()]);
}

#[test]
fn partition_expansion_has_action_and_payload_budgets() {
    for long_uri in [false, true] {
        let (mut document, object, _) = source(false, false);
        let rectangles = (0..10)
            .map(|index| [2. + index as f64 * 0.9, 3. + index as f64 * 0.4, 0.3, 0.2])
            .collect::<Vec<_>>();
        document
            .set_page_object_action_regions(0, &object, 1, &rectangles)
            .unwrap();
        let uri = format!(
            "https://example.org/{}",
            "a".repeat(if long_uri { 60_000 } else { 1 })
        );
        for _ in 0..if long_uri { 1 } else { 96 } {
            document
                .add_page_object_uri_action(0, &object, &uri)
                .unwrap();
        }
        let expected = if long_uri {
            "1000000 target bytes"
        } else {
            "10000 output actions"
        };
        assert!(
            matches!(document.to_pdf(), Err(OfdError::Limit(message)) if message.contains(expected))
        );
    }
}

#[test]
fn native_sequence_count_and_payload_are_bounded() {
    for long_uri in [false, true] {
        let (mut document, object, _) = source(false, false);
        let uri = format!(
            "https://example.org/{}",
            "a".repeat(if long_uri { 600_000 } else { 1 })
        );
        for _ in 0..if long_uri { 2 } else { 253 } {
            document
                .add_page_object_uri_action(0, &object, &uri)
                .unwrap();
        }
        let expected = if long_uri {
            "1000000 target bytes"
        } else {
            "256 actions"
        };
        assert!(
            matches!(document.to_pdf(), Err(OfdError::Limit(message)) if message.contains(expected))
        );
    }
}

#[test]
fn attachment_activation_cannot_be_miswritten_as_a_next_action() {
    for attachment_first in [false, true] {
        let mut document = OfdDocument::new();
        document.add_blank_page(30., 20.).unwrap();
        let object = document
            .add_path_object(0, "M 0 0 L 10 10", [0., 0., 10., 10.], PathStyle::default())
            .unwrap();
        document.add_attachment("link.txt", b"payload").unwrap();
        let attachment = document.attachments().unwrap()[0].id.clone();
        for index in 0..2 {
            if (index == 0) == attachment_first {
                document
                    .add_page_object_attachment_action(0, &object, &attachment, None)
                    .unwrap();
            } else {
                document
                    .add_page_object_uri_action(0, &object, "https://example.org/")
                    .unwrap();
            }
        }
        assert!(
            matches!(document.to_pdf(), Err(OfdError::Invalid(message)) if message.contains("attachment activation"))
        );
    }
}
