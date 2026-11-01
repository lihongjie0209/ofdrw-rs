use ofdrw::{
    BoxLayer, BoxLayoutItem, BoxLayoutStyle, FlowContainer, FlowContainerChild, FlowSpan,
    FlowStart, FlowTextAlign, FlowTextStyle, LayoutBox, NestedTable, OfdDocument, PageArea,
    PathStyle, TableCell, TableCellImage, TableStyle, TableVerticalAlign, TextBox,
};
use std::io::Cursor;

fn geometry() -> BoxLayoutStyle {
    BoxLayoutStyle {
        page_size: [20., 20.],
        margins: [2.; 4],
        start: None,
    }
}
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
fn root(children: Vec<FlowContainerChild>) -> FlowContainer {
    FlowContainer {
        layout: LayoutBox {
            size: [16., 0.],
            layer: Some(BoxLayer::Foreground),
            ..Default::default()
        },
        children,
        gap: 0.,
    }
}
fn image() -> TableCellImage {
    let mut data = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 2, image::Rgb([255, 0, 0])))
        .write_to(&mut data, image::ImageFormat::Png)
        .unwrap();
    TableCellImage {
        data: data.into_inner(),
        format: "PNG".into(),
        width: 2.,
        height: 2.,
    }
}
fn table() -> NestedTable {
    NestedTable {
        row_count: 6,
        column_widths: vec![12.],
        row_height: 3.,
        padding: 0.,
        border_width: 0.2,
        border_color: [0; 3],
        cells: (0..6)
            .map(|row| TableCell {
                row,
                column: 0,
                row_span: 1,
                column_span: 1,
                text: if row == 0 {
                    "H".into()
                } else {
                    row.to_string()
                },
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
                blocks: vec![],
            })
            .collect(),
    }
}

#[test]
fn measured_boxes_obey_ofd_layer_order_instead_of_insertion_order() {
    let colors = [
        (BoxLayer::Foreground, [0, 255, 0]),
        (BoxLayer::Background, [255, 0, 0]),
        (BoxLayer::Body, [0, 0, 255]),
    ];
    let items = colors
        .iter()
        .enumerate()
        .map(|(index, (layer, color))| {
            BoxLayoutItem::Box(LayoutBox {
                size: [4., 4.],
                layer: Some(*layer),
                background_color: Some(*color),
                offset: [0., -4. * index as f64],
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    let mut doc = OfdDocument::new();
    doc.add_box_layout(0, &items, geometry()).unwrap();
    assert_eq!(
        doc.page_layers(0)
            .unwrap()
            .iter()
            .map(|layer| layer.layer_type.as_str())
            .collect::<Vec<_>>(),
        ["Background", "Body", "Foreground"]
    );
    let mut reference = OfdDocument::new();
    reference.add_blank_page(20., 20.).unwrap();
    for (layer, color) in colors {
        let layer_id = reference
            .add_page_layer(
                0,
                match layer {
                    BoxLayer::Background => "Background",
                    BoxLayer::Body => "Body",
                    BoxLayer::Foreground => "Foreground",
                },
                None,
            )
            .unwrap();
        let id = reference
            .add_path_object(
                0,
                "M 0 0 L 4 0 L 4 4 L 0 4 C",
                [2., 2., 4., 4.],
                PathStyle {
                    fill: true,
                    stroke: false,
                    fill_color: Some(color),
                    ..Default::default()
                },
            )
            .unwrap();
        reference
            .move_page_object_to_layer(0, &id, &layer_id)
            .unwrap();
    }
    assert_eq!(
        doc.page_png(0, 72.).unwrap(),
        reference.page_png(0, 72.).unwrap()
    );
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(
        reopened.page_layers(0).unwrap(),
        doc.page_layers(0).unwrap()
    );
    assert!(doc.validate_references().unwrap().is_empty());
}

#[test]
fn text_box_decoration_and_plain_or_shaped_text_share_the_selected_layer_across_pages() {
    for shape in [false, true] {
        let mut doc = OfdDocument::new();
        let font = doc
            .add_embedded_font("DejaVu Sans", include_bytes!("fixtures/dejavu-sans.ttf"))
            .unwrap();
        let text = TextBox {
            layout: LayoutBox {
                size: [12., 0.],
                border: [0.2; 4],
                border_dash: Some(vec![1., 2.]),
                layer: Some(BoxLayer::Foreground),
                ..Default::default()
            },
            spans: vec![FlowSpan::new("A\nB\nC\nD\nE\nF")],
            style: FlowTextStyle {
                font_id: Some(font.to_string()),
                font_size: 2.,
                line_height: 4.,
                shape,
                ..Default::default()
            },
        };
        let result = doc.add_text_boxes(0, &[text], geometry()).unwrap();
        assert_eq!(result.pages, [0, 1]);
        for page in 0..2 {
            assert!(doc
                .page_objects(page)
                .unwrap()
                .iter()
                .all(|object| object.layer_type == "Foreground"));
            assert_eq!(
                doc.page_layers(page)
                    .unwrap()
                    .iter()
                    .filter(|layer| layer.object_count > 0)
                    .count(),
                1
            );
        }
        doc.add_text_object(0, "Body", [2., 2., 6., 3.], 2., None)
            .unwrap();
        assert_eq!(
            doc.page_layers(0)
                .unwrap()
                .iter()
                .map(|layer| layer.layer_type.as_str())
                .collect::<Vec<_>>(),
            ["Body", "Foreground"]
        );
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        assert_eq!(
            reopened.page_objects(1).unwrap(),
            doc.page_objects(1).unwrap()
        );
    }
}

#[test]
fn nested_layer_overrides_restore_parent_for_images_tables_and_following_text() {
    for dynamic in [false, true] {
        for repeat_header in [false, true] {
            let mut doc = OfdDocument::new();
            let nested = FlowContainer {
                layout: LayoutBox {
                    size: [12., 0.],
                    border: [0.2; 4],
                    layer: Some(BoxLayer::Background),
                    ..Default::default()
                },
                children: vec![paragraph("B")],
                gap: 0.,
            };
            let mut container = root(vec![
                paragraph("A"),
                FlowContainerChild::Container(Box::new(nested)),
                FlowContainerChild::Image {
                    image: image(),
                    align: FlowTextAlign::Left,
                },
                FlowContainerChild::Table {
                    table: Box::new(table()),
                    style: Box::new(TableStyle {
                        font_size: 2.,
                        line_height: 2.5,
                        repeat_header,
                        ..Default::default()
                    }),
                    align: FlowTextAlign::Left,
                },
                paragraph("Z"),
            ]);
            if dynamic {
                container.children.push(FlowContainerChild::LineBreak);
            }
            let result = doc
                .add_flow_containers(0, &[container], geometry())
                .unwrap();
            assert!(result.pages.len() >= 2);
            for page in result.pages {
                let objects = doc.page_objects(page).unwrap();
                assert!(objects
                    .iter()
                    .filter(|object| object.kind == "ImageObject")
                    .all(|object| object.layer_type == "Foreground"));
                for text in doc.page_text_objects(page).unwrap() {
                    let value = text
                        .codes
                        .iter()
                        .map(|code| code.text.as_str())
                        .collect::<String>();
                    let layer = objects.iter().find(|object| object.id == text.id).unwrap();
                    assert_eq!(
                        layer.layer_type,
                        if value == "B" {
                            "Background"
                        } else {
                            "Foreground"
                        }
                    );
                }
            }
            assert!(doc.validate_references().unwrap().is_empty());
        }
    }
}

#[test]
fn existing_named_layer_is_reused_with_canvas_origin_and_later_default_writes_stay_in_body() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(20., 20.).unwrap();
    doc.set_page_area(
        0,
        PageArea {
            physical_box: [10., 20., 20., 20.],
            application_box: None,
            content_box: None,
            bleed_box: None,
        },
    )
    .unwrap();
    let layer = doc.add_page_layer(0, "Background", None).unwrap();
    doc.set_page_layer_name(0, &layer, Some("existing"))
        .unwrap();
    let mut style = geometry();
    style.start = Some(FlowStart {
        page_index: 0,
        y: 2.,
    });
    doc.add_box_layout(
        0,
        &[BoxLayoutItem::Box(LayoutBox {
            size: [4., 4.],
            background_color: Some([255, 0, 0]),
            layer: Some(BoxLayer::Background),
            ..Default::default()
        })],
        style,
    )
    .unwrap();
    let layers = doc.page_layers(0).unwrap();
    assert_eq!(
        layers
            .iter()
            .filter(|layer| layer.layer_type == "Background")
            .count(),
        1
    );
    assert_eq!(layers[0].id.as_deref(), Some(layer.as_str()));
    assert_eq!(layers[0].name.as_deref(), Some("existing"));
    assert_eq!(
        doc.page_paths(0).unwrap()[0].boundary,
        Some([12., 22., 4., 4.])
    );
    doc.add_text_object(0, "Body", [12., 22., 4., 3.], 2., None)
        .unwrap();
    assert_eq!(
        doc.page_objects(0).unwrap().last().unwrap().layer_type,
        "Body"
    );
}

#[test]
fn placeholders_and_late_paint_errors_do_not_leak_layer_selection_or_mutate_source() {
    let mut doc = OfdDocument::new();
    let before = doc.to_bytes().unwrap();
    let bad = root(vec![
        paragraph("A"),
        FlowContainerChild::Image {
            image: TableCellImage {
                data: b"bad".to_vec(),
                format: "PNG".into(),
                width: 2.,
                height: 2.,
            },
            align: FlowTextAlign::Left,
        },
    ]);
    assert!(doc.add_flow_containers(0, &[bad], geometry()).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
    let mut placeholder = root(vec![paragraph("Hidden")]);
    placeholder.layout.placeholder = true;
    doc.add_flow_containers(0, &[placeholder], geometry())
        .unwrap();
    assert!(doc
        .page_layers(0)
        .unwrap()
        .iter()
        .all(|layer| layer.object_count == 0 && layer.layer_type == "Body"));
    doc.add_text_object(0, "Body", [2., 2., 6., 3.], 2., None)
        .unwrap();
    assert!(doc
        .page_objects(0)
        .unwrap()
        .iter()
        .all(|object| object.layer_type == "Body"));
}

#[test]
fn omitted_nested_layer_inherits_and_explicit_body_overrides_then_restores_foreground() {
    for dynamic in [false, true] {
        let mut body = root(vec![paragraph("B")]);
        body.layout.size[0] = 8.;
        body.layout.layer = Some(BoxLayer::Body);
        body.layout.background_color = Some([255, 0, 0]);
        let inherited = FlowContainer {
            layout: LayoutBox {
                size: [12., 0.],
                border: [0.2; 4],
                ..Default::default()
            },
            children: vec![
                paragraph("N"),
                FlowContainerChild::Container(Box::new(body)),
                paragraph("M"),
            ],
            gap: 0.,
        };
        let mut container = root(vec![
            paragraph("A"),
            FlowContainerChild::Container(Box::new(inherited)),
            paragraph("Z"),
        ]);
        if dynamic {
            container.children.push(FlowContainerChild::LineBreak);
        }
        let mut doc = OfdDocument::new();
        doc.add_flow_containers(0, &[container], geometry())
            .unwrap();
        let objects = doc.page_objects(0).unwrap();
        assert_eq!(
            objects
                .iter()
                .filter(|object| object.layer_type == "Body")
                .count(),
            2
        );
        assert_eq!(
            objects
                .iter()
                .filter(|object| object.layer_type == "Foreground")
                .count(),
            8
        );
        for text in doc.page_text_objects(0).unwrap() {
            let value = text
                .codes
                .iter()
                .map(|code| code.text.as_str())
                .collect::<String>();
            let object = objects.iter().find(|object| object.id == text.id).unwrap();
            assert_eq!(
                object.layer_type,
                if value == "B" { "Body" } else { "Foreground" }
            );
        }
    }
}
