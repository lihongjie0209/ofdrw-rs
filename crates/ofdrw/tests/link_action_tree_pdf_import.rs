use ofdrw::{OfdDocument, OfdError};

const SOURCE: &[u8] = include_bytes!("fixtures/link-action-tree.pdf");

fn check(document: &OfdDocument) {
    let annotations = document.annotations().unwrap();
    assert_eq!(annotations.len(), 1);
    let annotation = &annotations[0];
    assert_eq!(annotation.parameters["PDFActionSequence"], "true");
    let actions = &annotation.actions;
    assert_eq!(
        actions
            .iter()
            .map(|action| action.kind.as_str())
            .collect::<Vec<_>>(),
        ["URI", "Goto", "URI", "URI", "URI", "URI"]
    );
    for (index, suffix) in [
        (0, "docs/start"),
        (2, "docs/shared"),
        (3, "last"),
        (4, "docs/tail"),
        (5, "docs/shared"),
    ] {
        assert_eq!(
            actions[index].attributes["URI"],
            format!("https://example.org/{suffix}")
        );
    }
    assert_eq!(
        actions[1].destination.as_ref().unwrap().page_id,
        document.pages().unwrap()[1].id
    );
    assert_eq!(actions[1].destination.as_ref().unwrap().kind, "Fit");
    assert!(actions[0].region.is_some());
    assert!(actions
        .iter()
        .all(|action| action.region == actions[0].region));
    assert!(document.validate_references().unwrap().is_empty());
}

#[test]
fn mixed_action_tree_retains_order_shared_actions_and_click_regions() {
    let document = OfdDocument::from_pdf_bytes(SOURCE).unwrap();
    check(&document);
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    check(&reopened);
    let bytes = reopened.to_pdf().unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
    let annotations = page.get(b"Annots").unwrap().as_array().unwrap();
    assert_eq!(annotations.len(), 1);
    let annotation = pdf
        .dereference(&annotations[0])
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let quads = annotation.get(b"QuadPoints").unwrap().as_array().unwrap();
    assert_eq!(quads.len(), 8);
    for expected in [[12., 22.], [70., 22.], [78., 38.], [18., 38.]] {
        assert!(quads
            .chunks_exact(2)
            .any(
                |point| (f64::from(point[0].as_float().unwrap()) - expected[0]).abs() < 0.001
                    && (f64::from(point[1].as_float().unwrap()) - expected[1]).abs() < 0.001
            ));
    }
    let action = annotation.get(b"A").unwrap().as_dict().unwrap();
    assert_eq!(action.get(b"S").unwrap().as_name().unwrap(), b"URI");
    let next = action.get(b"Next").unwrap().as_array().unwrap();
    assert_eq!(next.len(), 5);
    assert_eq!(
        next[0]
            .as_dict()
            .unwrap()
            .get(b"S")
            .unwrap()
            .as_name()
            .unwrap(),
        b"GoTo"
    );
    let exported = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    check(&exported);
}

#[test]
fn goto_root_retains_its_following_uri_actions() {
    let mut pdf = lopdf::Document::load_mem(SOURCE).unwrap();
    pdf.get_object_mut((6, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("A", (8, 0));
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    let document = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    for document in [
        &document,
        &OfdDocument::from_pdf_bytes(&document.to_pdf().unwrap()).unwrap(),
    ] {
        let actions = &document.annotations().unwrap()[0].actions;
        assert_eq!(
            actions
                .iter()
                .map(|action| action.kind.as_str())
                .collect::<Vec<_>>(),
            ["Goto", "URI", "URI"]
        );
        assert_eq!(
            actions[1].attributes["URI"],
            "https://example.org/docs/shared"
        );
        assert_eq!(actions[2].attributes["URI"], "https://example.org/last");
    }
}

#[test]
fn action_tree_also_survives_a_link_normal_appearance() {
    use lopdf::{dictionary, Object, Stream};
    let mut pdf = lopdf::Document::load_mem(SOURCE).unwrap();
    let appearance = pdf.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 70.into(), 20.into()],
            "Resources" => dictionary! {},
        },
        b"0 0 1 rg 0 0 70 20 re f".to_vec(),
    ));
    pdf.get_object_mut((6, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("AP", dictionary! {"N" => Object::Reference(appearance)});
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    let document = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    check(&document);
    let image = image::load_from_memory(&document.page_png(0, 72.).unwrap())
        .unwrap()
        .to_rgb8();
    assert!(image.pixels().any(|pixel| pixel.0 == [0, 0, 255]));
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    check(&reopened);
    let exported = OfdDocument::from_pdf_bytes(&reopened.to_pdf().unwrap()).unwrap();
    check(&exported);
}

#[test]
fn unsupported_child_and_cycle_return_errors_for_the_complete_import() {
    for cycle in [false, true] {
        let mut pdf = lopdf::Document::load_mem(SOURCE).unwrap();
        let child = pdf.get_object_mut((9, 0)).unwrap().as_dict_mut().unwrap();
        if cycle {
            child.set("Next", (7, 0));
        } else {
            child.set("S", lopdf::Object::Name(b"JavaScript".to_vec()));
        }
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        let expected = if cycle {
            "cycle"
        } else {
            "action type is not supported"
        };
        assert!(matches!(OfdDocument::from_pdf_bytes(&bytes),
            Err(OfdError::Invalid(message)) if message.contains(expected)));
    }
}
