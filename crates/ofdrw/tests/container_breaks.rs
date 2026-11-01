use ofdrw::{
    BoxClear, BoxFloat, BoxLayoutStyle, FlowContainer, FlowContainerChild, FlowSpan, FlowStart,
    FlowTextStyle, LayoutBox, OfdDocument,
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
fn container(children: Vec<FlowContainerChild>) -> FlowContainer {
    FlowContainer {
        layout: LayoutBox {
            size: [16., 0.],
            ..Default::default()
        },
        children,
        gap: 0.,
    }
}
fn column(text: &str, float: BoxFloat) -> FlowContainerChild {
    let mut b = container(vec![paragraph(text, 3.)]);
    b.layout.size[0] = 4.;
    b.layout.float = float;
    b.layout.clear = BoxClear::None;
    FlowContainerChild::Container(Box::new(b))
}

#[test]
fn row_breaks_flush_nonempty_rows_and_ignore_empty_leading_repeated_and_trailing_rows() {
    let mut reference = OfdDocument::new();
    let mut doc = OfdDocument::new();
    let mut base = container(vec![
        column("A", BoxFloat::Left),
        FlowContainerChild::LineBreak,
        column("B", BoxFloat::Right),
    ]);
    base.gap = 1.5;
    let expected = reference
        .add_flow_containers(0, &[base], geometry())
        .unwrap();
    let mut extra = container(vec![
        FlowContainerChild::LineBreak,
        FlowContainerChild::LineBreak,
        column("A", BoxFloat::Left),
        FlowContainerChild::LineBreak,
        FlowContainerChild::LineBreak,
        column("B", BoxFloat::Right),
        FlowContainerChild::LineBreak,
        FlowContainerChild::LineBreak,
    ]);
    extra.gap = 1.5;
    let result = doc.add_flow_containers(0, &[extra], geometry()).unwrap();
    assert_eq!(result, expected);
    let objects = doc.page_text_objects(0).unwrap();
    assert_eq!(objects[0].boundary.unwrap()[..2], [2., 2.]);
    assert_eq!(objects[1].boundary.unwrap()[..2], [14., 6.5]);
    assert_eq!(result.next.unwrap().y, 9.5);
    assert_eq!(
        doc.page_png(0, 72.).unwrap(),
        reference.page_png(0, 72.).unwrap()
    );
}

#[test]
fn page_fills_preserve_explicit_empty_pages_and_reset_row_gaps() {
    let mut doc = OfdDocument::new();
    let mut root = container(vec![
        paragraph("A", 3.),
        FlowContainerChild::PageAreaFill,
        FlowContainerChild::PageAreaFill,
        paragraph("B", 3.),
        FlowContainerChild::PageAreaFill,
        FlowContainerChild::PageAreaFill,
    ]);
    root.gap = 4.;
    let result = doc.add_flow_containers(0, &[root], geometry()).unwrap();
    assert_eq!(result.pages, [0, 1, 2, 3]);
    assert_eq!(doc.page_text(0).unwrap(), ["A"]);
    assert!(doc.page_objects(1).unwrap().is_empty());
    assert_eq!(doc.page_text(2).unwrap(), ["B"]);
    assert!(doc.page_objects(3).unwrap().is_empty());
    assert_eq!(
        doc.page_text_objects(2).unwrap()[0].boundary.unwrap()[1],
        2.
    );
    assert_eq!(result.next.unwrap().y, 12.);
    assert_eq!(
        result
            .placements
            .iter()
            .map(|p| p.content_box[3])
            .sum::<f64>(),
        40.
    );
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.pages().unwrap().len(), 4);
    assert_eq!(reopened.page_text(2).unwrap(), ["B"]);
}

#[test]
fn leading_and_trailing_fills_match_java_page_area_filler_semantics() {
    for (children, expected) in [
        (
            vec![FlowContainerChild::PageAreaFill, paragraph("A", 3.)],
            vec![vec![], vec!["A"]],
        ),
        (
            vec![paragraph("A", 3.), FlowContainerChild::PageAreaFill],
            vec![vec!["A"]],
        ),
        (
            vec![paragraph("A", 10.), FlowContainerChild::PageAreaFill],
            vec![vec!["A"], vec![]],
        ),
    ] {
        let mut doc = OfdDocument::new();
        let result = doc
            .add_flow_containers(0, &[container(children)], geometry())
            .unwrap();
        assert_eq!(result.pages.len(), expected.len());
        for (page, text) in expected.into_iter().enumerate() {
            assert_eq!(doc.page_text(page).unwrap(), text);
        }
    }
}

#[test]
fn a_fill_inside_one_floating_column_does_not_move_the_other_column() {
    let mut left = container(vec![
        paragraph("A", 3.),
        FlowContainerChild::PageAreaFill,
        paragraph("B", 3.),
    ]);
    left.layout.size[0] = 6.;
    left.layout.clear = BoxClear::None;
    let mut right = container(vec![paragraph("C\nD", 4.)]);
    right.layout.size[0] = 6.;
    right.layout.float = BoxFloat::Right;
    right.layout.clear = BoxClear::None;
    let root = container(vec![
        FlowContainerChild::Container(Box::new(left)),
        FlowContainerChild::Container(Box::new(right)),
        paragraph("Z", 3.),
    ]);
    let mut doc = OfdDocument::new();
    let result = doc.add_flow_containers(0, &[root], geometry()).unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(doc.page_text(0).unwrap(), ["A", "C", "D"]);
    assert_eq!(doc.page_text(1).unwrap(), ["B", "Z"]);
    assert_eq!(result.next.unwrap().y, 8.);
}

#[test]
fn fills_respect_complete_container_constraints_and_errors_are_atomic() {
    let mut doc = OfdDocument::new();
    let before = doc.to_bytes().unwrap();
    let mut split = container(vec![
        paragraph("A", 3.),
        FlowContainerChild::PageAreaFill,
        paragraph("B", 3.),
    ]);
    split.layout.integrity = true;
    assert!(doc.add_flow_containers(0, &[split], geometry()).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
    doc.append_blank_page(0, 20., 14.).unwrap();
    let mut complete = container(vec![paragraph("A", 3.), FlowContainerChild::PageAreaFill]);
    complete.layout.integrity = true;
    complete.gap = 100.;
    let mut style = geometry();
    style.start = Some(FlowStart {
        page_index: 0,
        y: 8.,
    });
    let result = doc.add_flow_containers(0, &[complete], style).unwrap();
    assert_eq!(result.pages, [0]);
    assert_eq!(result.placements[0].content_box[3], 4.);
    assert_eq!(result.next.unwrap().y, 12.);
}

#[test]
fn empty_breaks_do_not_inflate_height_or_add_gaps_and_still_obey_node_limits() {
    let mut doc = OfdDocument::new();
    let mut reference = OfdDocument::new();
    let mut empty = container(Vec::new());
    empty.gap = f64::MAX;
    let expected = reference
        .add_flow_containers(0, std::slice::from_ref(&empty), geometry())
        .unwrap();
    empty.children = vec![FlowContainerChild::LineBreak; 100];
    assert_eq!(
        doc.add_flow_containers(0, &[empty], geometry()).unwrap(),
        expected
    );
    let before = doc.to_bytes().unwrap();
    let huge = container(vec![FlowContainerChild::LineBreak; 10_001]);
    assert!(doc.add_flow_containers(0, &[huge], geometry()).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn controls_match_the_independently_generated_java_page_and_position_fixture() {
    fn element(
        label: &str,
        width: f64,
        height: f64,
        float: BoxFloat,
        clear: BoxClear,
    ) -> FlowContainerChild {
        let mut b = container(vec![paragraph(label, height)]);
        b.layout.size[0] = width;
        b.layout.float = float;
        b.layout.clear = clear;
        FlowContainerChild::Container(Box::new(b))
    }
    let a = || element("A", 4., 3., BoxFloat::Left, BoxClear::None);
    let b = || element("B", 5., 3., BoxFloat::Right, BoxClear::None);
    let c = || element("C", 6., 3., BoxFloat::Left, BoxClear::Both);
    use FlowContainerChild::{LineBreak as Br, PageAreaFill as Fill};
    for (name, children) in [
        ("row_break", vec![a(), Br, b()]),
        ("empty_breaks", vec![Br, Br, a(), Br, Br, b(), Br, Br]),
        ("fills", vec![a(), Fill, Fill, c(), Fill, Fill]),
        ("leading_fill", vec![Fill, a()]),
        ("trailing_fill", vec![a(), Fill]),
        (
            "full_fill",
            vec![element("A", 4., 10., BoxFloat::Left, BoxClear::Both), Fill],
        ),
        ("fills_and_br", vec![a(), b(), Br, Fill, c()]),
    ] {
        let mut doc = OfdDocument::new();
        let result = doc
            .add_flow_containers(0, &[container(children)], geometry())
            .unwrap();
        let mut pages = Vec::new();
        let mut expected = Vec::new();
        for line in include_str!("fixtures/container-controls-java.tsv")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.get(1).copied() != Some(name) {
                continue;
            }
            let page = fields[2].parse::<usize>().unwrap();
            if fields[0] == "P" {
                pages.push(page);
            } else {
                expected.push((
                    page,
                    fields[3].to_owned(),
                    fields[4].parse::<f64>().unwrap(),
                    fields[5].parse::<f64>().unwrap(),
                ));
            }
        }
        assert_eq!(result.pages, pages, "{name}");
        let actual = result
            .pages
            .iter()
            .flat_map(|page| {
                doc.page_text_objects(*page)
                    .unwrap()
                    .into_iter()
                    .map(|object| {
                        let text = object
                            .codes
                            .iter()
                            .map(|code| code.text.as_str())
                            .collect::<String>();
                        let bounds = object.boundary.unwrap();
                        (*page, text, bounds[0], bounds[1])
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "{name}");
    }
}
