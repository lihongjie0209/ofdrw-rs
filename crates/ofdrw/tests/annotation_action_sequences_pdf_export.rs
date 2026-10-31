use ofdrw::{AnnotationGraphic, AnnotationPath, OfdDocument, PathStyle};

fn add_link(
    document: &mut OfdDocument,
    label: &str,
    overlap: bool,
    color: Option<[u8; 3]>,
) -> String {
    let id = document
        .add_uri_link_annotation(0, "https://example.org/first", [2., 3., 10., 5.])
        .unwrap();
    document.set_annotation_remark(0, &id, Some(label)).unwrap();
    if let Some(color) = color {
        document
            .append_annotation_objects(
                0,
                &id,
                &[AnnotationGraphic::Path(AnnotationPath {
                    commands: "M 0 0 L 10 0 L 10 5 L 0 5 C".into(),
                    boundary: [0., 0., 10., 5.],
                    style: PathStyle {
                        fill: true,
                        stroke: false,
                        fill_color: Some(color),
                        alpha: Some(128),
                        ..PathStyle::default()
                    },
                })],
            )
            .unwrap();
        document
            .set_annotation_parameter(0, &id, "PDFSourceSubtype", "Link")
            .unwrap();
    }
    let annotation = document
        .annotations()
        .unwrap()
        .into_iter()
        .find(|annotation| annotation.id == id)
        .unwrap();
    let path = annotation.source_path;
    let mut xml = String::from_utf8(document.entry(&path).unwrap().unwrap().to_vec()).unwrap();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let range = parsed
        .descendants()
        .find(|node| node.tag_name().name() == "Annot" && node.attribute("ID") == Some(id.as_str()))
        .unwrap()
        .range();
    let mut fragment = xml[range.clone()].to_owned();
    let first_region = "<ofd:Region><ofd:Area Start=\"2 3\"><ofd:Line Point1=\"8 3\"/><ofd:Line Point1=\"8 7\"/><ofd:Line Point1=\"2 7\"/><ofd:Close/></ofd:Area></ofd:Region>";
    let second_region = "<ofd:Region><ofd:Area Start=\"5 3\"><ofd:Line Point1=\"11 3\"/><ofd:Line Point1=\"11 7\"/><ofd:Line Point1=\"5 7\"/><ofd:Close/></ofd:Area></ofd:Region>";
    if overlap {
        fragment = fragment.replacen(
            "<ofd:Action Event=\"CLICK\">",
            &format!("<ofd:Action Event=\"CLICK\">{first_region}"),
            1,
        );
    }
    assert!(fragment.contains("</ofd:Actions>"));
    fragment = fragment.replacen("</ofd:Actions>", &format!("<ofd:Action Event=\"CLICK\">{}<ofd:URI URI=\"https://example.org/second\"/></ofd:Action></ofd:Actions>", if overlap { second_region } else { "" }), 1);
    xml.replace_range(range, &fragment);
    document.set_entry(&path, xml.into_bytes()).unwrap();
    id
}

fn source() -> OfdDocument {
    let mut document = OfdDocument::new();
    document.add_blank_page(30., 20.).unwrap();
    document
}

fn sample(document: &OfdDocument) -> [[u8; 4]; 2] {
    let image = image::load_from_memory(&document.page_png(0, 72.).unwrap())
        .unwrap()
        .to_rgba8();
    [image.get_pixel(17, 14).0, image.get_pixel(17, 21).0]
}

#[test]
fn missing_regions_use_each_owning_graphics_boundary() {
    let mut document = source();
    let id = add_link(&mut document, "owners", false, None);
    let owner2 = document
        .append_annotation_objects(
            0,
            &id,
            &[AnnotationGraphic::Path(AnnotationPath {
                commands: "M 0 0 L 3 5".into(),
                boundary: [7., 0., 3., 5.],
                style: PathStyle {
                    fill: false,
                    stroke: false,
                    ..PathStyle::default()
                },
            })],
        )
        .unwrap()
        .remove(0);
    let path = document.annotations().unwrap()[0].source_path.clone();
    let mut xml = String::from_utf8(document.entry(&path).unwrap().unwrap().to_vec()).unwrap();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let second_action = parsed
        .descendants()
        .find(|node| {
            node.tag_name().name() == "Action"
                && node
                    .descendants()
                    .any(|child| child.attribute("URI") == Some("https://example.org/second"))
        })
        .unwrap();
    let owner1 = second_action
        .ancestors()
        .find(|node| node.tag_name().name() == "PathObject")
        .unwrap();
    let owner2 = parsed
        .descendants()
        .find(|node| node.attribute("ID") == Some(owner2.as_str()))
        .unwrap();
    let replacement = xml[owner2.range()].replacen(
        "</ofd:PathObject>",
        &format!(
            "<ofd:Actions>{}</ofd:Actions></ofd:PathObject>",
            &xml[second_action.range()]
        ),
        1,
    );
    let mut edits = vec![
        (owner2.range(), replacement),
        (second_action.range(), String::new()),
        (
            owner1
                .attributes()
                .find(|attribute| attribute.name() == "Boundary")
                .unwrap()
                .range(),
            "Boundary=\"0 0 3 5\"".into(),
        ),
    ];
    edits.sort_by_key(|(range, _)| range.start);
    for (range, value) in edits.into_iter().rev() {
        xml.replace_range(range, &value);
    }
    document.set_entry(&path, xml.into_bytes()).unwrap();
    let imported = OfdDocument::from_pdf_bytes(&document.to_pdf().unwrap()).unwrap();
    let annotations = imported.annotations().unwrap();
    assert_eq!(annotations.len(), 2);
    for (annotation, x) in annotations.iter().zip([2., 9.]) {
        assert_eq!(annotation.actions.len(), 1);
        let bounds = annotation.appearance_box.unwrap();
        assert!((bounds[0] - x).abs() < 0.0001);
        assert!((bounds[2] - 3.).abs() < 0.0001);
    }
    assert_eq!(
        annotations[0].actions[0].attributes["URI"],
        "https://example.org/first"
    );
    assert_eq!(
        annotations[1].actions[0].attributes["URI"],
        "https://example.org/second"
    );
    assert!(imported.validate_references().unwrap().is_empty());
}

#[test]
fn native_link_annotations_keep_all_supported_click_actions() {
    for overlap in [false, true] {
        let mut document = source();
        add_link(&mut document, "native", overlap, None);
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        let imported = OfdDocument::from_pdf_bytes(&reopened.to_pdf().unwrap()).unwrap();
        let annotations = imported.annotations().unwrap();
        let mut counts = annotations
            .iter()
            .map(|annotation| annotation.actions.len())
            .collect::<Vec<_>>();
        counts.sort();
        assert_eq!(counts, if overlap { vec![1, 1, 2] } else { vec![2] });
        for annotation in &annotations {
            if annotation.actions.len() == 2 {
                assert_eq!(
                    annotation.actions[0].attributes["URI"],
                    "https://example.org/first"
                );
                assert_eq!(
                    annotation.actions[1].attributes["URI"],
                    "https://example.org/second"
                );
            }
        }
        assert!(imported.validate_references().unwrap().is_empty());
    }
}

#[test]
fn edited_imported_sequences_can_use_different_rectangular_regions() {
    let mut document = source();
    let id = add_link(&mut document, "edited", true, None);
    document
        .set_annotation_parameter(0, &id, "PDFActionSequence", "true")
        .unwrap();
    let imported = OfdDocument::from_pdf_bytes(&document.to_pdf().unwrap()).unwrap();
    assert_eq!(imported.annotations().unwrap().len(), 3);
    assert!(imported
        .annotations()
        .unwrap()
        .iter()
        .any(|annotation| annotation.actions.len() == 2));
}

#[test]
fn split_links_paint_a_transparent_appearance_once_and_keep_annotation_order() {
    let mut document = source();
    add_link(&mut document, "first", true, Some([255, 0, 0]));
    add_link(&mut document, "second", true, Some([0, 255, 0]));
    let expected = sample(&document);
    let bytes = document.to_pdf().unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
    let annotations = page.get(b"Annots").unwrap().as_array().unwrap();
    assert_eq!(annotations.len(), 6);
    let mut nonempty = 0;
    for (index, value) in annotations.iter().enumerate() {
        let annotation = pdf.dereference(value).unwrap().1.as_dict().unwrap();
        assert_eq!(
            lopdf::decode_text_string(annotation.get(b"Contents").unwrap()).unwrap(),
            if index < 3 { "first" } else { "second" }
        );
        let appearance = annotation
            .get(b"AP")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"N")
            .unwrap();
        if !pdf
            .dereference(appearance)
            .unwrap()
            .1
            .as_stream()
            .unwrap()
            .content
            .is_empty()
        {
            nonempty += 1;
        }
    }
    assert_eq!(nonempty, 2);
    let imported = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    assert_eq!(sample(&imported), expected);
    let again = OfdDocument::from_pdf_bytes(&imported.to_pdf().unwrap()).unwrap();
    assert_eq!(sample(&again), expected);
    assert!(again.validate_references().unwrap().is_empty());
}

#[test]
fn appearance_frame_does_not_enlarge_a_rectangle_click_region() {
    let mut document = source();
    let id = add_link(&mut document, "appearance", true, Some([0, 0, 255]));
    // The first partition's artwork occupies the full appearance frame, but its
    // click quadrilateral must still cover only that partition.
    document.set_annotation_read_only(0, &id, true).unwrap();
    let pdf = lopdf::Document::load_mem(&document.to_pdf().unwrap()).unwrap();
    let page = pdf.get_dictionary(pdf.get_pages()[&1]).unwrap();
    let value = &page.get(b"Annots").unwrap().as_array().unwrap()[0];
    let annotation = pdf.dereference(value).unwrap().1.as_dict().unwrap();
    let rect = annotation.get(b"Rect").unwrap().as_array().unwrap();
    let quads = annotation.get(b"QuadPoints").unwrap().as_array().unwrap();
    assert_eq!(quads.len(), 8);
    let rect_width = rect[2].as_float().unwrap() - rect[0].as_float().unwrap();
    let xs = quads
        .chunks_exact(2)
        .map(|point| point[0].as_float().unwrap())
        .collect::<Vec<_>>();
    let quad_width = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max)
        - xs.iter().copied().fold(f32::INFINITY, f32::min);
    assert!(quad_width < rect_width / 2.);
}
