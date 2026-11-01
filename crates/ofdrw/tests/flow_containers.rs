use ofdrw::{
    BoxLayoutStyle, FlowContainer, FlowContainerChild, FlowSpan, FlowTextAlign, FlowTextStyle,
    LayoutBox, OfdDocument, TableCellImage,
};
use std::io::Cursor;

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
fn container(children: Vec<FlowContainerChild>) -> FlowContainer {
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
fn image() -> TableCellImage {
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 2, image::Rgb([255, 0, 0])))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    TableCellImage {
        data: bytes.into_inner(),
        format: "PNG".into(),
        width: 4.,
        height: 4.,
    }
}

#[test]
fn mixed_nested_content_keeps_lines_and_images_intact_across_pages() {
    let mut doc = OfdDocument::new();
    let mut nested = container(vec![paragraph("B\nC\nD")]);
    nested.layout.padding = [0.5; 4];
    nested.layout.border = [0.25; 4];
    nested.layout.background_color = Some([220, 230, 240]);
    let root = container(vec![
        paragraph("A"),
        FlowContainerChild::Container(Box::new(nested)),
        FlowContainerChild::Image {
            image: image(),
            align: FlowTextAlign::Right,
        },
        paragraph("E"),
    ]);
    let result = doc.add_flow_containers(0, &[root], geometry()).unwrap();
    assert_eq!(result.pages, [0, 1, 2]);
    assert_eq!(doc.page_text(0).unwrap(), ["A", "B", "C"]);
    assert_eq!(doc.page_text(1).unwrap(), ["D"]);
    assert_eq!(doc.page_text(2).unwrap(), ["E"]);
    assert_eq!(
        doc.page_images(1).unwrap()[0].boundary.unwrap()[..3],
        [10., 5.75, 4.]
    );
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(2).unwrap(), ["E"]);
    assert!(!reopened.page_png(1, 72.).unwrap().is_empty());
}

#[test]
fn complete_nested_boxes_move_to_a_new_page() {
    let mut doc = OfdDocument::new();
    let mut nested = container(vec![paragraph("B\nC")]);
    nested.layout.integrity = true;
    let root = container(vec![
        paragraph("A\nA"),
        FlowContainerChild::Container(Box::new(nested)),
    ]);
    let result = doc.add_flow_containers(0, &[root], geometry()).unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(doc.page_text(0).unwrap(), ["A", "A"]);
    assert_eq!(doc.page_text(1).unwrap(), ["B", "C"]);
}

#[test]
fn container_gaps_spacers_and_nested_alignment_use_content_coordinates() {
    let mut doc = OfdDocument::new();
    let mut nested = container(vec![paragraph("B")]);
    nested.layout.size[0] = 4.;
    nested.layout.float = ofdrw::BoxFloat::Right;
    let mut root = container(vec![
        paragraph("A"),
        FlowContainerChild::Spacer { height: 1. },
        FlowContainerChild::Container(Box::new(nested)),
    ]);
    root.gap = 0.5;
    let result = doc.add_flow_containers(0, &[root], geometry()).unwrap();
    assert_eq!(result.pages, [0]);
    let objects = doc.page_text_objects(0).unwrap();
    assert_eq!(objects[0].boundary.unwrap()[..2], [2., 2.]);
    assert_eq!(objects[1].boundary.unwrap()[..2], [10., 7.]);
    assert_eq!(result.next.unwrap().y, 10.);
}

#[test]
fn invalid_nested_content_rolls_back_the_entire_operation() {
    let mut doc = OfdDocument::new();
    let before = doc.to_bytes().unwrap();
    let mut invalid_image = image();
    invalid_image.data.clear();
    let mut too_tall = image();
    too_tall.height = 11.;
    for child in [
        FlowContainerChild::Image {
            image: invalid_image,
            align: FlowTextAlign::Left,
        },
        FlowContainerChild::Image {
            image: too_tall,
            align: FlowTextAlign::Left,
        },
        FlowContainerChild::Spacer { height: f64::NAN },
    ] {
        assert!(doc
            .add_flow_containers(0, &[container(vec![paragraph("A"), child])], geometry())
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    let mut nested = container(vec![paragraph("B")]);
    nested.layout.clear = ofdrw::BoxClear::None;
    assert!(doc
        .add_flow_containers(
            0,
            &[container(vec![FlowContainerChild::Container(Box::new(
                nested
            ))])],
            geometry()
        )
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn nesting_and_node_limits_fail_without_changing_source() {
    let mut doc = OfdDocument::new();
    let before = doc.to_bytes().unwrap();
    let mut nested = container(vec![paragraph("A")]);
    for _ in 0..8 {
        nested = container(vec![FlowContainerChild::Container(Box::new(nested))]);
    }
    assert!(doc.add_flow_containers(0, &[nested], geometry()).is_err());
    let huge = container(vec![FlowContainerChild::Spacer { height: 0. }; 10_001]);
    assert!(doc.add_flow_containers(0, &[huge], geometry()).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn shaped_nested_paragraphs_preserve_their_text_and_font_after_round_trip() {
    let mut doc = OfdDocument::new();
    let font = doc
        .add_embedded_font("DejaVu Sans", include_bytes!("fixtures/dejavu-sans.ttf"))
        .unwrap();
    let child = FlowContainerChild::Paragraph {
        spans: vec![FlowSpan::new("مرحبا e\u{301}\nMore text")],
        style: Box::new(FlowTextStyle {
            font_id: Some(font.to_string()),
            shape: true,
            font_size: 2.,
            line_height: 3.,
            ..Default::default()
        }),
    };
    let root = container(vec![FlowContainerChild::Container(Box::new(container(
        vec![child],
    )))]);
    doc.add_flow_containers(0, &[root], geometry()).unwrap();
    let text = doc.page_text(0).unwrap();
    assert!(text.concat().contains("مرحبا"));
    assert!(doc.validate_references().unwrap().is_empty());
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(0).unwrap(), text);
    assert!(!reopened.page_png(0, 72.).unwrap().is_empty());
}

#[test]
fn root_floating_columns_and_nested_placeholders_keep_their_own_geometry() {
    let mut doc = OfdDocument::new();
    let mut left = container(vec![paragraph("A\nB\nC\nD")]);
    left.layout.size[0] = 6.;
    left.layout.clear = ofdrw::BoxClear::None;
    let mut hidden = container(vec![paragraph("X")]);
    hidden.layout.placeholder = true;
    let mut right = container(vec![
        FlowContainerChild::Container(Box::new(hidden)),
        paragraph("E"),
    ]);
    right.layout.size[0] = 6.;
    right.layout.clear = ofdrw::BoxClear::None;
    right.layout.float = ofdrw::BoxFloat::Right;
    let result = doc
        .add_flow_containers(0, &[left, right], geometry())
        .unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(doc.page_text(0).unwrap(), ["A", "B", "C", "E"]);
    let objects = doc.page_text_objects(0).unwrap();
    assert_eq!(objects.last().unwrap().boundary.unwrap()[0], 12.);
    assert_eq!(objects.last().unwrap().boundary.unwrap()[1], 5.);
}

#[test]
fn overflowing_nested_drawing_coordinates_roll_back_even_without_decorations() {
    let mut doc = OfdDocument::new();
    let before = doc.to_bytes().unwrap();
    let mut nested = container(vec![paragraph("A")]);
    nested.layout.offset = [f64::MAX, 0.];
    let mut root = container(vec![FlowContainerChild::Container(Box::new(nested))]);
    root.layout.offset = [f64::MAX, 0.];
    assert!(doc.add_flow_containers(0, &[root], geometry()).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn fractional_nested_edges_and_gaps_preserve_every_line_once() {
    for index in 1..40 {
        let mut doc = OfdDocument::new();
        let edge = f64::from(index) / 100.;
        let mut nested = container(vec![
            paragraph("A\nB\nC\nD"),
            FlowContainerChild::Image {
                image: image(),
                align: FlowTextAlign::Center,
            },
        ]);
        nested.layout.margin = [edge; 4];
        nested.layout.padding = [edge / 3.; 4];
        nested.layout.border = [edge / 7.; 4];
        let mut middle = container(vec![
            FlowContainerChild::Container(Box::new(nested)),
            paragraph("E"),
        ]);
        middle.gap = edge / 11.;
        middle.layout.padding = [edge / 5.; 4];
        let root = container(vec![
            FlowContainerChild::Container(Box::new(middle)),
            paragraph("F"),
        ]);
        let result = doc.add_flow_containers(0, &[root], geometry()).unwrap();
        assert_eq!(
            result
                .pages
                .iter()
                .map(|page| doc.page_images(*page).unwrap().len())
                .sum::<usize>(),
            1
        );
        let text: Vec<String> = result
            .pages
            .iter()
            .flat_map(|page| doc.page_text(*page).unwrap())
            .collect();
        assert_eq!(text, ["A", "B", "C", "D", "E", "F"], "edge {edge}");
    }
}
