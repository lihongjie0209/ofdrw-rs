use ofdrw::{
    BoxLayoutItem, BoxLayoutStyle, FlowContainer, FlowContainerChild, FlowSpan, FlowTextStyle,
    LayoutBox, OfdDocument, TextBox,
};

fn geometry() -> BoxLayoutStyle {
    BoxLayoutStyle {
        page_size: [30., 20.],
        margins: [2.; 4],
        start: None,
    }
}
fn layout(dash: Vec<f64>) -> LayoutBox {
    LayoutBox {
        size: [10., 5.],
        border: [1.; 4],
        border_color: [10, 20, 30],
        border_dash: Some(dash),
        alpha: Some(128),
        ..Default::default()
    }
}
fn xml(doc: &OfdDocument, page: usize) -> String {
    String::from_utf8(
        doc.entry(&doc.pages().unwrap()[page].path)
            .unwrap()
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

#[test]
fn java_dash_arguments_normalize_without_changing_allocated_geometry() {
    for (input, pattern, offset) in [
        (vec![3.], "3 3", None),
        (vec![3., 2.], "3 2", None),
        (vec![1., 3., 2.], "3 2", Some("1")),
        (vec![1., 3., 2., 4., 1.], "3 2 4 1", Some("1")),
        (vec![-1., 3., 2.], "3 2", None),
        (vec![0.0000001, 3., 2.], "3 2", None),
    ] {
        let mut doc = OfdDocument::new();
        let source = layout(input);
        let result = doc
            .add_box_layout(0, &[BoxLayoutItem::Box(source.clone())], geometry())
            .unwrap();
        let mut solid = source;
        solid.border_dash = None;
        let mut reference = OfdDocument::new();
        assert_eq!(
            result,
            reference
                .add_box_layout(0, &[BoxLayoutItem::Box(solid)], geometry())
                .unwrap()
        );
        assert_eq!(doc.page_paths(0).unwrap().len(), 1);
        let encoded = xml(&doc, 0);
        assert!(encoded.contains(&format!("DashPattern=\"{pattern}\"")));
        if let Some(offset) = offset {
            assert!(encoded.contains(&format!("DashOffset=\"{offset}\"")));
        } else {
            assert!(!encoded.contains("DashOffset="));
        }
        assert!(encoded.contains("Alpha=\"128\""));
        assert!(encoded.contains("Value=\"10 20 30\""));
        assert!(doc.page_svg(0).unwrap().contains("stroke-dasharray"));
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        assert_eq!(doc.page_paths(0).unwrap(), reopened.page_paths(0).unwrap());
        assert_ne!(
            doc.page_png(0, 72.).unwrap(),
            reference.page_png(0, 72.).unwrap()
        );
    }
}

#[test]
fn unequal_edges_use_java_centerline_and_corner_geometry() {
    let mut doc = OfdDocument::new();
    let mut b = layout(vec![1., 3., 2.]);
    b.border = [1., 2., 3., 4.];
    doc.add_box_layout(0, &[BoxLayoutItem::Box(b)], geometry())
        .unwrap();
    let paths = doc.page_paths(0).unwrap();
    assert_eq!(
        paths
            .iter()
            .map(|path| path.boundary.unwrap())
            .collect::<Vec<_>>(),
        [
            [2., 2., 16., 1.],
            [2., 8., 16., 3.],
            [2., 3., 4., 5.],
            [16., 3., 2., 5.],
        ]
    );
    assert_eq!(
        paths
            .iter()
            .map(|path| path.abbreviated_data.as_deref().unwrap())
            .collect::<Vec<_>>(),
        [
            "M 0 0.5 L 16 0.5",
            "M 0 1.5 L 16 1.5",
            "M 2 0 L 2 5",
            "M 1 0 L 1 5",
        ]
    );
    assert!(paths.iter().all(|path| path.stroke && !path.fill));
    assert_eq!(xml(&doc, 0).matches("DashPattern=\"3 2\"").count(), 4);
}

#[test]
fn page_cuts_do_not_draw_new_horizontal_edges_in_middle_fragments() {
    let mut doc = OfdDocument::new();
    let mut b = layout(vec![2.]);
    b.size[1] = 40.;
    let result = doc
        .add_box_layout(0, &[BoxLayoutItem::Box(b)], geometry())
        .unwrap();
    assert_eq!(result.pages, [0, 1, 2]);
    for (page, count) in [(0, 3), (1, 2), (2, 3)] {
        let paths = doc.page_paths(page).unwrap();
        assert_eq!(paths.len(), count);
        assert_eq!(
            xml(&doc, page).matches("DashPattern=\"2 2\"").count(),
            count
        );
    }
    assert_eq!(result.placements[1].borders, [0., 1., 0., 1.]);
}

#[test]
fn empty_dash_is_solid_and_placeholders_do_not_paint() {
    for placeholder in [false, true] {
        let mut b = layout(Vec::new());
        b.placeholder = placeholder;
        let mut doc = OfdDocument::new();
        doc.add_box_layout(0, &[BoxLayoutItem::Box(b.clone())], geometry())
            .unwrap();
        b.border_dash = None;
        let mut reference = OfdDocument::new();
        reference
            .add_box_layout(0, &[BoxLayoutItem::Box(b)], geometry())
            .unwrap();
        assert_eq!(doc.page_paths(0).unwrap(), reference.page_paths(0).unwrap());
        assert!(!xml(&doc, 0).contains("DashPattern="));
        if placeholder {
            assert!(doc.page_paths(0).unwrap().is_empty());
        }
    }
}

#[test]
fn invalid_dash_arguments_roll_back_boxes_text_boxes_and_nested_containers() {
    for dash in [
        vec![0.],
        vec![-1.],
        vec![1., 0.],
        vec![f64::NAN],
        vec![f64::INFINITY, 1., 2.],
        vec![f64::MAX, f64::MAX],
        vec![f64::MAX],
        vec![1.; 258],
    ] {
        let mut doc = OfdDocument::new();
        let before = doc.to_bytes().unwrap();
        let mut b = layout(dash);
        b.placeholder = true;
        assert!(doc
            .add_box_layout(
                0,
                &[
                    BoxLayoutItem::Box(layout(vec![1.])),
                    BoxLayoutItem::Box(b.clone())
                ],
                geometry()
            )
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
        assert!(doc
            .add_text_boxes(
                0,
                &[TextBox {
                    layout: b.clone(),
                    spans: vec![FlowSpan::new("A")],
                    style: FlowTextStyle::default()
                }],
                geometry()
            )
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
        let nested = FlowContainer {
            layout: b,
            children: vec![],
            gap: 0.,
        };
        let parent = FlowContainer {
            layout: LayoutBox {
                size: [20., 0.],
                ..Default::default()
            },
            children: vec![FlowContainerChild::Container(Box::new(nested))],
            gap: 0.,
        };
        assert!(doc.add_flow_containers(0, &[parent], geometry()).is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
}

#[test]
fn full_length_dash_sequence_and_nested_text_content_are_supported() {
    let mut doc = OfdDocument::new();
    let mut b = layout(vec![1.; 257]);
    b.size[1] = 0.;
    let root = FlowContainer {
        layout: LayoutBox {
            size: [20., 0.],
            ..Default::default()
        },
        children: vec![
            FlowContainerChild::Container(Box::new(FlowContainer {
                layout: b,
                children: vec![FlowContainerChild::Paragraph {
                    spans: vec![FlowSpan::new("A")],
                    style: Box::new(FlowTextStyle {
                        font_size: 2.,
                        line_height: 3.,
                        ..Default::default()
                    }),
                }],
                gap: 0.,
            })),
            FlowContainerChild::LineBreak,
        ],
        gap: 0.,
    };
    doc.add_flow_containers(0, &[root], geometry()).unwrap();
    assert_eq!(doc.page_text(0).unwrap(), ["A"]);
    assert_eq!(doc.page_paths(0).unwrap().len(), 1);
    assert!(xml(&doc, 0).contains("DashOffset=\"1\""));
}

#[test]
fn all_border_paths_match_independently_generated_java_geometry() {
    let fixture = include_str!("fixtures/box-border-dashes-java.tsv");
    assert!(fixture.contains("84e9eb260ba6fd351ecccf08a01a40305d4af635"));
    let tokens = |value: &str| {
        value
            .split_whitespace()
            .map(|token| {
                token
                    .parse::<f64>()
                    .map_or_else(|_| token.to_string(), |number| number.to_string())
            })
            .collect::<Vec<_>>()
    };
    for (name, dash) in [
        ("one", vec![3.]),
        ("pairs", vec![3., 2.]),
        ("offset", vec![1., 3., 2.]),
        ("long", vec![1., 3., 2., 4., 1.]),
        ("unequal", vec![1., 3., 2.]),
        ("negative_offset", vec![-1., 3., 2.]),
    ] {
        let mut b = layout(dash);
        if name == "unequal" {
            b.border = [1., 2., 3., 4.];
        }
        let mut doc = OfdDocument::new();
        doc.add_box_layout(0, &[BoxLayoutItem::Box(b)], geometry())
            .unwrap();
        let paths = doc.page_paths(0).unwrap();
        let reference = fixture
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .filter(|fields| fields[0] == name)
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), reference.len(), "{name}");
        for (path, expected) in paths.iter().zip(reference) {
            let boundary = expected[1]
                .split_whitespace()
                .map(|value| value.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(path.boundary.unwrap().as_slice(), boundary, "{name}");
            assert_eq!(
                tokens(path.abbreviated_data.as_deref().unwrap()),
                tokens(expected[5]),
                "{name}"
            );
        }
    }
}

#[test]
fn dashed_box_borders_survive_vector_pdf_export_and_reimport() {
    let mut doc = OfdDocument::new();
    let mut b = layout(vec![1., 3., 2.]);
    b.alpha = None;
    doc.add_box_layout(0, &[BoxLayoutItem::Box(b)], geometry())
        .unwrap();
    let pdf = doc.to_pdf().unwrap();
    let imported = OfdDocument::from_pdf_bytes(&pdf).unwrap();
    assert_eq!(imported.pages().unwrap().len(), 1);
    let page_xml = xml(&imported, 0);
    assert!(page_xml.contains("DashPattern="), "{page_xml}");
    let svg = imported.page_svg(0).unwrap();
    assert!(svg.contains("stroke-dasharray"), "{svg}");
    assert!(!imported.page_png(0, 30.).unwrap().is_empty());
}
