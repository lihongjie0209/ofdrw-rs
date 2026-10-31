use ofdrw::{DestinationInfo, DocumentActionTarget, OfdDocument};

fn source() -> (OfdDocument, String, String) {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(30., 20.).unwrap();
    doc.add_blank_page(30., 20.).unwrap();
    let annotation = doc
        .add_uri_link_annotation(0, "https://example.org/first", [2., 3., 10., 5.])
        .unwrap();
    let object = doc.annotation_objects(0, &annotation).unwrap()[0]
        .id
        .clone()
        .unwrap();
    (doc, annotation, object)
}

fn destination(doc: &OfdDocument) -> DestinationInfo {
    DestinationInfo {
        page_id: doc.pages().unwrap()[1].id.clone(),
        kind: "Fit".into(),
        left: None,
        top: None,
        right: None,
        bottom: None,
        zoom: None,
    }
}

#[test]
fn all_targets_keep_order_escaping_and_regions_after_reopening() {
    let (mut doc, annotation, object) = source();
    let destination = destination(&doc);
    let name = "bookmark & \"中文\"";
    doc.add_bookmark(0, name, destination.clone()).unwrap();
    doc.add_attachment("payload.txt", b"payload").unwrap();
    let attachment = doc.attachments().unwrap()[0].id.clone();
    let uri = "https://example.org/?a=1&b=\"中文\"";
    for target in [
        DocumentActionTarget::Uri(uri.into()),
        DocumentActionTarget::Page(destination.clone()),
        DocumentActionTarget::Bookmark(name.into()),
        DocumentActionTarget::Attachment {
            id: attachment.clone(),
            new_window: Some(false),
        },
    ] {
        doc.add_annotation_object_action(0, &annotation, &object, target)
            .unwrap();
    }
    doc.set_annotation_object_action_regions(
        0,
        &annotation,
        &object,
        2,
        &[[0., 1., 2., 3.], [4., 1., 2., 3.]],
    )
    .unwrap();
    let doc = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    let actions = doc
        .annotation_object_actions(0, &annotation, &object)
        .unwrap();
    assert_eq!(actions.len(), 5);
    assert!(actions.iter().all(|a| a.event == "CLICK"));
    assert_eq!(actions[1].attributes.get("URI").unwrap(), uri);
    assert_eq!(
        actions[2].destination.as_ref().unwrap().page_id,
        destination.page_id
    );
    assert_eq!(actions[2].region.as_ref().unwrap().areas.len(), 2);
    assert_eq!(actions[3].bookmark.as_deref(), Some(name));
    assert_eq!(actions[4].attributes.get("AttachID"), Some(&attachment));
    assert!(doc.validate_references().unwrap().is_empty());
}

#[test]
fn remove_last_action_then_append_and_clear_region() {
    let (mut doc, annotation, object) = source();
    doc.remove_annotation_object_action(0, &annotation, &object, 0)
        .unwrap();
    assert!(doc
        .annotation_object_actions(0, &annotation, &object)
        .unwrap()
        .is_empty());
    doc.add_annotation_object_action(
        0,
        &annotation,
        &object,
        DocumentActionTarget::Uri("https://example.org/next".into()),
    )
    .unwrap();
    doc.set_annotation_object_action_regions(0, &annotation, &object, 0, &[[0., 0., 2., 3.]])
        .unwrap();
    assert!(doc
        .annotation_object_actions(0, &annotation, &object)
        .unwrap()[0]
        .region
        .is_some());
    doc.set_annotation_object_action_regions(0, &annotation, &object, 0, &[])
        .unwrap();
    assert!(doc
        .annotation_object_actions(0, &annotation, &object)
        .unwrap()[0]
        .region
        .is_none());
}

#[test]
fn invalid_targets_indices_and_rectangles_do_not_mutate_the_package() {
    let (mut doc, annotation, object) = source();
    let before = doc.to_bytes().unwrap();
    let mut missing_page = destination(&doc);
    missing_page.page_id = "99999".into();
    for target in [
        DocumentActionTarget::Uri(" ".into()),
        DocumentActionTarget::Bookmark("missing".into()),
        DocumentActionTarget::Attachment {
            id: "99999".into(),
            new_window: None,
        },
        DocumentActionTarget::Page(missing_page),
    ] {
        assert!(doc
            .add_annotation_object_action(0, &annotation, &object, target)
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    for rect in [
        [0., 0., 0., 1.],
        [0., 0., 1., -1.],
        [f64::NAN, 0., 1., 1.],
        [f64::MAX, 0., f64::MAX, 1.],
    ] {
        assert!(doc
            .set_annotation_object_action_regions(0, &annotation, &object, 0, &[rect])
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    for (page, annotation, object) in [
        (0, "", object.as_str()),
        (0, annotation.as_str(), ""),
        (0, "99999", object.as_str()),
        (0, annotation.as_str(), "99999"),
        (99, annotation.as_str(), object.as_str()),
    ] {
        assert!(doc
            .annotation_object_actions(page, annotation, object)
            .is_err());
        assert!(doc
            .add_annotation_object_action(
                page,
                annotation,
                object,
                DocumentActionTarget::Uri("x".into())
            )
            .is_err());
    }
    assert!(doc
        .remove_annotation_object_action(0, &annotation, &object, 99)
        .is_err());
    assert!(doc
        .set_annotation_object_action_regions(0, &annotation, &object, 99, &[])
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn nested_page_blocks_and_alternate_namespace_prefix_are_preserved() {
    let (mut doc, annotation, object) = source();
    let path = doc.annotations().unwrap()[0].source_path.clone();
    let xml = String::from_utf8(doc.entry(&path).unwrap().unwrap().to_vec()).unwrap();
    let xml = xml
        .replace("ofd:", "o:")
        .replace("xmlns:ofd=", "xmlns:o=")
        .replace("<o:PathObject", "<o:PageBlock><o:PathObject")
        .replace("</o:PathObject>", "</o:PathObject></o:PageBlock>");
    doc.set_entry(&path, xml.into_bytes()).unwrap();
    doc.add_annotation_object_action(
        0,
        &annotation,
        &object,
        DocumentActionTarget::Uri("https://example.org/new".into()),
    )
    .unwrap();
    doc.set_annotation_object_action_regions(0, &annotation, &object, 1, &[[0., 0., 3., 2.]])
        .unwrap();
    doc.remove_annotation_object_action(0, &annotation, &object, 0)
        .unwrap();
    let actions = doc
        .annotation_object_actions(0, &annotation, &object)
        .unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].attributes["URI"], "https://example.org/new");
    let xml = std::str::from_utf8(doc.entry(&path).unwrap().unwrap()).unwrap();
    assert!(xml.contains("<o:PageBlock>"));
    assert!(xml.contains("<o:Region>"));
    assert!(!xml.contains("<ofd:"));
}

#[test]
fn signed_annotations_allow_reads_but_reject_every_action_edit() {
    let (mut doc, annotation, object) = source();
    doc.add_digital_signature(&[1; 32], "2030-01-01 00:00:00")
        .unwrap();
    let before = doc.to_bytes().unwrap();
    assert_eq!(
        doc.annotation_object_actions(0, &annotation, &object)
            .unwrap()
            .len(),
        1
    );
    assert!(doc
        .add_annotation_object_action(
            0,
            &annotation,
            &object,
            DocumentActionTarget::Uri("x".into())
        )
        .is_err());
    assert!(doc
        .remove_annotation_object_action(0, &annotation, &object, 0)
        .is_err());
    assert!(doc
        .set_annotation_object_action_regions(0, &annotation, &object, 0, &[])
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
    assert!(doc
        .check_signature_references(0)
        .unwrap()
        .iter()
        .all(|reference| reference.valid));
}

#[test]
fn annotation_targets_cannot_cross_document_bodies() {
    let (mut doc, annotation, object) = source();
    let mut other = OfdDocument::new();
    for _ in 0..8 {
        other.add_blank_page(30., 20.).unwrap();
    }
    let mut destination = destination(&other);
    destination.page_id = other.pages().unwrap()[7].id.clone();
    other
        .add_bookmark(0, "foreign", destination.clone())
        .unwrap();
    other.add_attachment("foreign.txt", b"foreign").unwrap();
    let body = doc.append_document_body_from(&other, 0).unwrap();
    let attachment = doc
        .attachments()
        .unwrap()
        .into_iter()
        .find(|a| a.document_index == body)
        .unwrap()
        .id;
    // Page identifiers are local to a body; choose an ID absent from the owner body.
    assert!(!doc
        .pages()
        .unwrap()
        .iter()
        .any(|p| p.document_index == 0 && p.id == destination.page_id));
    let before = doc.to_bytes().unwrap();
    for target in [
        DocumentActionTarget::Page(destination),
        DocumentActionTarget::Bookmark("foreign".into()),
        DocumentActionTarget::Attachment {
            id: attachment,
            new_window: None,
        },
    ] {
        assert!(doc
            .add_annotation_object_action(0, &annotation, &object, target)
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
}
