use ofdrw::{
    BoxClear, BoxFloat, BoxLayer, BoxLayoutStyle, FixedContainer, FlowContainer,
    FlowContainerChild, FlowSpan, FlowStart, FlowTextAlign, FlowTextStyle, LayoutBox, NestedTable,
    OfdDocument, PageArea, PathStyle, TableCell, TableCellImage, TableStyle, TableVerticalAlign,
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
            size: [16., 0.],
            ..Default::default()
        },
        children,
        gap: 0.,
    }
}
fn fixed(container: FlowContainer, position: [f64; 2]) -> FixedContainer {
    FixedContainer {
        position,
        container,
    }
}
fn document() -> OfdDocument {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(20., 20.).unwrap();
    doc
}
fn point(doc: &OfdDocument, page: usize, text: &str) -> [f64; 2] {
    let item = doc
        .page_text_objects(page)
        .unwrap()
        .into_iter()
        .find(|object| {
            object
                .codes
                .iter()
                .map(|code| code.text.as_str())
                .collect::<String>()
                == text
        })
        .unwrap();
    let [x, y, _, _] = item.boundary.unwrap();
    [x, y]
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
                    format!("R{row}")
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
fn automatic_height_matches_existing_flow_geometry_and_plain_or_shaped_painting() {
    for shape in [false, true] {
        let mut positioned = OfdDocument::new();
        let mut flowed = OfdDocument::new();
        let font = positioned
            .add_embedded_font("DejaVu Sans", include_bytes!("fixtures/dejavu-sans.ttf"))
            .unwrap();
        assert_eq!(
            font,
            flowed
                .add_embedded_font("DejaVu Sans", include_bytes!("fixtures/dejavu-sans.ttf"))
                .unwrap()
        );
        positioned.add_blank_page(30., 100.).unwrap();
        let text = FlowContainerChild::Paragraph {
            spans: vec![FlowSpan {
                underline: true,
                fill_color: Some([120, 20, 30]),
                ..FlowSpan::new("AfiB\nA سلام\nCafe\u{301}")
            }],
            style: Box::new(FlowTextStyle {
                font_id: Some(font.to_string()),
                font_size: 2.,
                line_height: 3.,
                shape,
                align: FlowTextAlign::Right,
                first_line_indent: 0.5,
                rest_line_indent: 0.25,
                ..Default::default()
            }),
        };
        let mut root = container(vec![
            text,
            FlowContainerChild::Spacer { height: 1. },
            paragraph("Z"),
        ]);
        root.layout.size[0] = 12.;
        root.layout.padding = [0.5; 4];
        root.layout.border = [0.2; 4];
        root.layout.background_color = Some([230, 240, 250]);
        root.gap = 0.5;
        let expected = flowed
            .add_flow_containers(
                0,
                std::slice::from_ref(&root),
                BoxLayoutStyle {
                    page_size: [30., 100.],
                    margins: [2.; 4],
                    start: None,
                },
            )
            .unwrap();
        let actual = positioned
            .add_fixed_containers(0, &[fixed(root, [2., 2.])])
            .unwrap();
        assert_eq!(actual.len(), expected.placements.len());
        for (actual, expected) in actual.iter().zip(&expected.placements) {
            assert_eq!(
                (
                    actual.page_index,
                    actual.source_index,
                    actual.first,
                    actual.last
                ),
                (
                    expected.page_index,
                    expected.source_index,
                    expected.first,
                    expected.last
                )
            );
            let actual_values = [
                actual.allocation,
                actual.border_box,
                actual.padding_box,
                actual.content_box,
                actual.borders,
            ];
            let expected_values = [
                expected.allocation,
                expected.border_box,
                expected.padding_box,
                expected.content_box,
                expected.borders,
            ];
            for (actual, expected) in actual_values
                .into_iter()
                .flatten()
                .zip(expected_values.into_iter().flatten())
            {
                assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
            }
        }
        assert_eq!(
            positioned.page_png(0, 72.).unwrap(),
            flowed.page_png(0, 72.).unwrap()
        );
        let reopened = OfdDocument::from_bytes(&positioned.to_bytes().unwrap()).unwrap();
        assert_eq!(
            positioned.page_text(0).unwrap(),
            reopened.page_text(0).unwrap()
        );
    }
}

#[test]
fn declared_height_preserves_decoration_while_all_text_lines_can_overflow() {
    let mut doc = document();
    let mut root = container(vec![paragraph("A\nB\nC")]);
    root.layout.size = [12., 2.];
    root.layout.border = [0.2; 4];
    root.layout.background_color = Some([255, 0, 0]);
    let placed = doc
        .add_fixed_containers(0, &[fixed(root, [2., 2.])])
        .unwrap();
    assert_eq!(placed[0].content_box, [2.2, 2.2, 12., 2.]);
    assert_eq!(point(&doc, 0, "A"), [2.2, 2.2]);
    assert_eq!(point(&doc, 0, "C"), [2.2, 8.2]);
    assert_eq!(doc.pages().unwrap().len(), 1);
    assert_eq!(placed[0].source_offset, 0.);
    assert!(placed[0].first && placed[0].last);
}

fn column(text: &str, width: f64, height: f64, float: BoxFloat) -> FlowContainerChild {
    let mut b = container(vec![paragraph(text)]);
    b.layout.size = [width, height];
    b.layout.float = float;
    b.layout.clear = BoxClear::None;
    FlowContainerChild::Container(Box::new(b))
}
#[test]
fn nested_floating_rows_use_declared_heights_and_keep_overflowing_content() {
    let mut doc = document();
    let mut root = container(vec![
        column("L1\nL2", 6., 2., BoxFloat::Left),
        column("R", 6., 4., BoxFloat::Right),
        paragraph("Z"),
    ]);
    root.gap = 1.;
    let placed = doc
        .add_fixed_containers(0, &[fixed(root, [2., 2.])])
        .unwrap();
    assert_eq!(point(&doc, 0, "L1"), [2., 2.]);
    assert_eq!(point(&doc, 0, "L2"), [2., 5.]);
    assert_eq!(point(&doc, 0, "R"), [12., 2.]);
    assert_eq!(point(&doc, 0, "Z"), [2., 7.]);
    assert_eq!(placed[0].content_box[3], 8.);
}

#[test]
fn line_breaks_flush_rows_and_empty_breaks_do_not_add_gap() {
    let make = |extra: bool| {
        let mut children = vec![
            column("L", 6., 2., BoxFloat::Left),
            FlowContainerChild::LineBreak,
            column("R", 6., 4., BoxFloat::Right),
        ];
        if extra {
            children.insert(0, FlowContainerChild::LineBreak);
            children.insert(2, FlowContainerChild::LineBreak);
            children.push(FlowContainerChild::LineBreak);
            children.push(FlowContainerChild::LineBreak);
        }
        let mut root = container(children);
        root.gap = 1.;
        root
    };
    let mut doc = document();
    let actual = doc
        .add_fixed_containers(0, &[fixed(make(true), [2., 2.])])
        .unwrap();
    let mut reference = document();
    let expected = reference
        .add_fixed_containers(0, &[fixed(make(false), [2., 2.])])
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(point(&doc, 0, "R"), [12., 5.]);
    assert_eq!(
        doc.page_png(0, 72.).unwrap(),
        reference.page_png(0, 72.).unwrap()
    );
}

#[test]
fn fixed_coordinates_and_widths_are_preserved_on_nonzero_and_smaller_canvases() {
    let mut doc = document();
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
    let mut root = container(vec![paragraph("A")]);
    root.layout.size[0] = 40.;
    let placed = doc
        .add_fixed_containers(
            0,
            &[
                fixed(root, [12., 22.]),
                fixed(container(vec![paragraph("B")]), [-4., -3.]),
            ],
        )
        .unwrap();
    assert_eq!(placed[0].content_box, [12., 22., 40., 3.]);
    assert_eq!(point(&doc, 0, "A"), [12., 22.]);
    assert_eq!(point(&doc, 0, "B"), [-4., -3.]);
    assert_eq!(doc.pages().unwrap().len(), 1);
    assert_eq!(
        doc.pages().unwrap()[0].physical_box,
        Some([10., 20., 20., 20.])
    );
}

#[test]
fn later_root_decoration_can_cover_an_earlier_roots_content() {
    let mut doc = document();
    let mut a = container(vec![paragraph("A")]);
    a.layout.size = [12., 4.];
    a.layout.background_color = Some([255, 0, 0]);
    let mut b = container(vec![]);
    b.layout.size = [12., 4.];
    b.layout.background_color = Some([0, 255, 0]);
    doc.add_fixed_containers(0, &[fixed(a, [2., 2.]), fixed(b, [2., 2.])])
        .unwrap();
    let mut reference = document();
    reference
        .add_path_object(
            0,
            "M 0 0 L 12 0 L 12 4 L 0 4 C",
            [2., 2., 12., 4.],
            PathStyle {
                fill: true,
                stroke: false,
                fill_color: Some([255, 0, 0]),
                ..Default::default()
            },
        )
        .unwrap();
    reference
        .add_flowed_spans(
            0,
            &[FlowSpan::new("A")],
            FlowTextStyle {
                page_size: [20., 20.],
                margins: [2.; 4],
                font_size: 2.,
                line_height: 3.,
                start: Some(FlowStart {
                    page_index: 0,
                    y: 2.,
                }),
                ..Default::default()
            },
        )
        .unwrap();
    reference
        .add_path_object(
            0,
            "M 0 0 L 12 0 L 12 4 L 0 4 C",
            [2., 2., 12., 4.],
            PathStyle {
                fill: true,
                stroke: false,
                fill_color: Some([0, 255, 0]),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        doc.page_png(0, 72.).unwrap(),
        reference.page_png(0, 72.).unwrap()
    );
    assert_eq!(doc.page_text(0).unwrap(), ["A"]);
}

#[test]
fn images_and_full_tables_remain_on_the_selected_page_without_repeated_headers() {
    let mut doc = document();
    let root = container(vec![
        paragraph("A"),
        FlowContainerChild::Image {
            image: image(),
            align: FlowTextAlign::Right,
        },
        FlowContainerChild::Table {
            table: Box::new(table()),
            style: Box::new(TableStyle {
                font_size: 2.,
                line_height: 2.5,
                repeat_header: true,
                ..Default::default()
            }),
            align: FlowTextAlign::Left,
        },
        paragraph("Z"),
    ]);
    let placed = doc
        .add_fixed_containers(0, &[fixed(root, [2., 2.])])
        .unwrap();
    assert_eq!(placed[0].content_box[3], 26.);
    assert_eq!(doc.pages().unwrap().len(), 1);
    assert_eq!(
        doc.page_text(0).unwrap(),
        ["A", "H", "R1", "R2", "R3", "R4", "R5", "Z"]
    );
    assert_eq!(
        doc.page_images(0).unwrap()[0].boundary,
        Some([16., 5., 2., 2.])
    );
    assert!(doc.validate_references().unwrap().is_empty());
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(0).unwrap(), doc.page_text(0).unwrap());
}

#[test]
fn nested_offsets_and_layer_inheritance_restore_default_body_writing() {
    let mut doc = document();
    let mut nested = container(vec![paragraph("B")]);
    nested.layout.size[0] = 8.;
    nested.layout.offset = [-0.5, 1.];
    nested.layout.layer = Some(BoxLayer::Body);
    nested.layout.background_color = Some([255, 0, 0]);
    let mut root = container(vec![FlowContainerChild::Container(Box::new(nested))]);
    root.layout.margin = [1.; 4];
    root.layout.border = [0.2; 4];
    root.layout.padding = [0.5; 4];
    root.layout.offset = [1., 2.];
    root.layout.layer = Some(BoxLayer::Foreground);
    let placed = doc
        .add_fixed_containers(0, &[fixed(root, [5., 7.])])
        .unwrap();
    assert_eq!(placed[0].allocation[..2], [5., 7.]);
    let p = point(&doc, 0, "B");
    assert!((p[0] - 7.2).abs() < 1e-9);
    assert!((p[1] - 11.7).abs() < 1e-9);
    let objects = doc.page_objects(0).unwrap();
    assert!(objects
        .iter()
        .any(|object| object.layer_type == "Foreground"));
    assert!(objects
        .iter()
        .filter(|object| object.kind == "TextObject")
        .all(|object| object.layer_type == "Body"));
    doc.add_text_object(0, "Later", [2., 2., 6., 3.], 2., None)
        .unwrap();
    assert!(doc
        .page_objects(0)
        .unwrap()
        .iter()
        .filter(|object| object.kind == "TextObject")
        .all(|object| object.layer_type == "Body"));
}

#[test]
fn invalid_positions_heights_and_late_image_errors_roll_back_every_root() {
    let mut doc = document();
    let before = doc.to_bytes().unwrap();
    for height in [-1., f64::NAN, f64::INFINITY] {
        let mut root = container(vec![paragraph("A")]);
        root.layout.size[1] = height;
        assert!(doc
            .add_fixed_containers(0, &[fixed(root, [2., 2.])])
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    for position in [[f64::NAN, 0.], [0., f64::INFINITY], [f64::MAX, f64::MAX]] {
        let mut root = container(vec![paragraph("A")]);
        root.layout.offset = [f64::MAX; 2];
        assert!(doc
            .add_fixed_containers(0, &[fixed(root, position)])
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    let valid = container(vec![FlowContainerChild::Image {
        image: image(),
        align: FlowTextAlign::Left,
    }]);
    let invalid = container(vec![FlowContainerChild::Image {
        image: TableCellImage {
            data: b"bad".to_vec(),
            format: "PNG".into(),
            width: 2.,
            height: 2.,
        },
        align: FlowTextAlign::Left,
    }]);
    assert!(doc
        .add_fixed_containers(0, &[fixed(valid, [2., 2.]), fixed(invalid, [5., 5.])])
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
    assert!(doc.add_fixed_containers(1, &[]).is_err());
    assert!(doc.add_fixed_containers(0, &[]).unwrap().is_empty());
}

#[test]
fn placeholders_keep_declared_geometry_and_structural_limits_are_checked() {
    let mut doc = document();
    let mut root = container(vec![paragraph("Hidden")]);
    root.layout.size[1] = 9.;
    root.layout.placeholder = true;
    let placed = doc
        .add_fixed_containers(0, &[fixed(root, [2., 2.])])
        .unwrap();
    assert_eq!(placed[0].content_box[3], 9.);
    assert!(doc.page_objects(0).unwrap().is_empty());
    let before = doc.to_bytes().unwrap();
    let source = fixed(container(vec![]), [2., 2.]);
    assert!(doc.add_fixed_containers(0, &vec![source; 10_001]).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
    let mut nested = container(vec![paragraph("A")]);
    for _ in 0..7 {
        nested = container(vec![FlowContainerChild::Container(Box::new(nested))]);
    }
    doc.add_fixed_containers(0, &[fixed(nested.clone(), [2., 2.])])
        .unwrap();
    let before = doc.to_bytes().unwrap();
    let too_deep = container(vec![FlowContainerChild::Container(Box::new(nested))]);
    assert!(doc
        .add_fixed_containers(0, &[fixed(too_deep, [2., 2.])])
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn global_page_selection_uses_the_later_body_font_and_preserves_other_pages() {
    let mut doc = document();
    doc.append_document_body().unwrap();
    doc.append_blank_page(1, 20., 20.).unwrap();
    let font = doc
        .add_embedded_font_to_document(1, "DejaVu Sans", include_bytes!("fixtures/dejavu-sans.ttf"))
        .unwrap();
    doc.append_document_body().unwrap();
    doc.append_blank_page(2, 20., 20.).unwrap();
    let original = doc
        .pages()
        .unwrap()
        .iter()
        .map(|page| doc.entry(&page.path).unwrap().unwrap().to_vec())
        .collect::<Vec<_>>();
    let root = container(vec![FlowContainerChild::Paragraph {
        spans: vec![FlowSpan::new("A")],
        style: Box::new(FlowTextStyle {
            font_id: Some(font.to_string()),
            font_size: 2.,
            line_height: 3.,
            shape: true,
            ..Default::default()
        }),
    }]);
    let result = doc
        .add_fixed_containers(1, &[fixed(root, [3., 4.])])
        .unwrap();
    assert_eq!(result[0].page_index, 1);
    assert_eq!(doc.pages().unwrap().len(), 3);
    assert_eq!(doc.page_text(1).unwrap(), ["A"]);
    for page in [0, 2] {
        assert_eq!(
            doc.entry(&doc.pages().unwrap()[page].path)
                .unwrap()
                .unwrap(),
            original[page]
        );
    }
    assert!(doc.validate_references().unwrap().is_empty());
}

#[test]
fn integrity_protected_pages_reject_fixed_container_writes() {
    let mut doc = document();
    doc.protect_integrity_sm2(
        include_bytes!("fixtures/user-private-key.bin"),
        include_bytes!("fixtures/user-cert.der"),
    )
    .unwrap();
    let before = doc.to_bytes().unwrap();
    assert!(doc
        .add_fixed_containers(0, &[fixed(container(vec![paragraph("A")]), [2., 2.])])
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn page_area_fillers_are_ignored_on_fixed_pages_without_flushing_floating_rows() {
    let make = |fillers: bool| {
        let children = vec![
            column("L", 6., 2., BoxFloat::Left),
            column("R", 6., 4., BoxFloat::Right),
            paragraph("Z"),
        ];
        let children = if fillers {
            let mut result = vec![FlowContainerChild::PageAreaFill];
            for child in children {
                result.push(child);
                result.push(FlowContainerChild::PageAreaFill);
            }
            result
        } else {
            children
        };
        let mut root = container(children);
        root.gap = 1.;
        root
    };
    let mut doc = document();
    let placed = doc
        .add_fixed_containers(0, &[fixed(make(true), [2., 2.])])
        .unwrap();
    let mut reference = document();
    let expected = reference
        .add_fixed_containers(0, &[fixed(make(false), [2., 2.])])
        .unwrap();
    assert_eq!(placed, expected);
    assert_eq!(point(&doc, 0, "R"), [12., 2.]);
    assert_eq!(point(&doc, 0, "Z"), [2., 7.]);
    assert_eq!(doc.pages().unwrap().len(), 1);
    assert_eq!(
        doc.page_png(0, 72.).unwrap(),
        reference.page_png(0, 72.).unwrap()
    );
    let before = doc.to_bytes().unwrap();
    doc.add_fixed_containers(
        0,
        &[fixed(
            container(vec![FlowContainerChild::PageAreaFill]),
            [2., 2.],
        )],
    )
    .unwrap();
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn fixed_box_geometry_matches_the_independent_java_virtual_page_reference() {
    let fixture = include_str!("fixtures/fixed-containers-java.tsv");
    assert!(fixture.contains("84e9eb260ba6fd351ecccf08a01a40305d4af635"));
    for name in ["outside", "negative", "tall", "placeholder", "fractional"] {
        let (position, size) = match name {
            "outside" => ([12., 22.], [40., 3.]),
            "negative" => ([-4., -3.], [8., 2.]),
            "tall" => ([3., 4.], [5., 50.]),
            "placeholder" => ([5., 6.], [4., 9.]),
            _ => ([1.25, 2.5], [6.5, 3.25]),
        };
        let mut root = container(vec![]);
        root.layout.size = size;
        root.layout.background_color = Some([240, 230, 220]);
        if name == "outside" {
            root.layout.margin = [1.; 4];
            root.layout.padding = [0.5; 4];
            root.layout.border = [0.25; 4];
        }
        if name == "tall" {
            root.layout.integrity = true;
        }
        if name == "placeholder" {
            root.layout.placeholder = true;
        }
        if name == "fractional" {
            root.layout.margin = [0.25, 0.5, 0.75, 1.];
            root.layout.border = [0.25, 0.5, 0.75, 1.];
            root.layout.padding = [0.25, 0.5, 0.75, 1.];
        }
        let rows = fixture
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .filter(|fields| fields[1] == name)
            .collect::<Vec<_>>();
        let parse = |value: &str| {
            value
                .split_whitespace()
                .map(|value| value.parse::<f64>().unwrap())
                .collect::<Vec<_>>()
        };
        let allocation = parse(rows.iter().find(|fields| fields[0] == "A").unwrap()[2]);
        let backgrounds = rows
            .iter()
            .filter(|fields| fields[0] == "B")
            .map(|fields| parse(fields[2]))
            .collect::<Vec<_>>();
        let mut doc = document();
        let placed = doc
            .add_fixed_containers(0, &[fixed(root, position)])
            .unwrap();
        assert_eq!(placed[0].allocation.as_slice(), allocation, "{name}");
        let paths = doc.page_paths(0).unwrap();
        if backgrounds.is_empty() {
            assert!(paths.is_empty(), "{name}");
        } else {
            assert_eq!(
                paths[0].boundary.unwrap().as_slice(),
                backgrounds[0],
                "{name}"
            );
        }
        assert_eq!(doc.pages().unwrap().len(), 1);
    }
}

#[test]
fn cumulative_nested_offset_overflow_rolls_back_already_painted_roots_and_resources() {
    let mut doc = document();
    let before = doc.to_bytes().unwrap();
    let valid = container(vec![FlowContainerChild::Image {
        image: image(),
        align: FlowTextAlign::Left,
    }]);
    let mut child = container(vec![paragraph("A")]);
    child.layout.offset = [f64::MAX, 0.];
    let mut root = container(vec![FlowContainerChild::Container(Box::new(child))]);
    root.layout.offset = [f64::MAX, 0.];
    root.layout.layer = Some(BoxLayer::Foreground);
    assert!(doc
        .add_fixed_containers(0, &[fixed(valid, [2., 2.]), fixed(root, [0., 0.])])
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
    doc.add_text_object(0, "Body", [2., 2., 6., 3.], 2., None)
        .unwrap();
    assert!(doc
        .page_objects(0)
        .unwrap()
        .iter()
        .all(|object| object.layer_type == "Body"));
}
