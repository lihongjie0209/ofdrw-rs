use ofdrw::{
    BoxClear, BoxFloat, BoxLayoutItem, BoxLayoutStyle, FlowStart, LayoutBox, OfdDocument, PageArea,
};

fn style() -> BoxLayoutStyle {
    BoxLayoutStyle {
        page_size: [20.; 2],
        margins: [2.; 4],
        start: None,
    }
}

fn shared(width: f64, height: f64, float: BoxFloat, clear: BoxClear) -> LayoutBox {
    LayoutBox {
        size: [width, height],
        float,
        clear,
        ..Default::default()
    }
}

fn boxed(b: LayoutBox) -> BoxLayoutItem {
    BoxLayoutItem::Box(b)
}

#[test]
fn left_and_right_boxes_share_a_row_whose_height_is_the_maximum() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_box_layout(
            0,
            &[
                boxed(shared(4., 3., BoxFloat::Left, BoxClear::None)),
                boxed(shared(5., 7., BoxFloat::Right, BoxClear::None)),
                boxed(shared(3., 2., BoxFloat::Left, BoxClear::None)),
                BoxLayoutItem::LineBreak,
                boxed(LayoutBox {
                    size: [4., 1.],
                    ..Default::default()
                }),
            ],
            style(),
        )
        .unwrap();
    let allocations: Vec<_> = result.placements.iter().map(|p| p.allocation).collect();
    assert_eq!(
        allocations,
        [
            [2., 2., 4., 3.],
            [13., 2., 5., 7.],
            [6., 2., 3., 2.],
            [2., 9., 4., 1.]
        ]
    );
    assert_eq!(result.pages, [0]);
    assert_eq!(
        result.next,
        Some(FlowStart {
            page_index: 0,
            y: 10.
        })
    );
}

#[test]
fn center_groups_stay_separate_from_side_floats_and_honor_clear_edges() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_box_layout(
            0,
            &[
                boxed(shared(4., 3., BoxFloat::Center, BoxClear::None)),
                boxed(shared(3., 4., BoxFloat::Center, BoxClear::Right)),
                boxed(shared(2., 2., BoxFloat::Center, BoxClear::None)),
                boxed(shared(2., 1., BoxFloat::Left, BoxClear::None)),
            ],
            style(),
        )
        .unwrap();
    let allocations: Vec<_> = result.placements.iter().map(|p| p.allocation).collect();
    assert_eq!(
        allocations,
        [
            [6.5, 2., 4., 3.],
            [10.5, 2., 3., 4.],
            [9., 6., 2., 2.],
            [2., 8., 2., 1.]
        ]
    );
}

#[test]
fn matching_clear_restarts_a_row_while_opposite_clear_is_exclusive() {
    for (float, same, opposite) in [
        (BoxFloat::Left, BoxClear::Left, BoxClear::Right),
        (BoxFloat::Right, BoxClear::Right, BoxClear::Left),
    ] {
        let mut doc = OfdDocument::new();
        let result = doc
            .add_box_layout(
                0,
                &[
                    boxed(shared(4., 3., float, BoxClear::None)),
                    boxed(shared(4., 4., float, same)),
                    boxed(shared(4., 2., float, opposite)),
                    boxed(shared(4., 1., float, BoxClear::None)),
                ],
                style(),
            )
            .unwrap();
        let ys: Vec<_> = result.placements.iter().map(|p| p.allocation[1]).collect();
        assert_eq!(ys, [2., 5., 9., 11.]);
    }
}

#[test]
fn overflowing_row_width_starts_a_new_row_and_content_width_is_clamped() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_box_layout(
            0,
            &[
                boxed(shared(9., 3., BoxFloat::Left, BoxClear::None)),
                boxed(shared(8., 4., BoxFloat::Right, BoxClear::None)),
                boxed(LayoutBox {
                    size: [100., 1.],
                    margin: [1.; 4],
                    border: [1.; 4],
                    padding: [1.; 4],
                    ..Default::default()
                }),
            ],
            style(),
        )
        .unwrap();
    assert_eq!(result.placements[1].allocation, [10., 5., 8., 4.]);
    assert_eq!(result.placements[2].allocation, [2., 9., 16., 7.]);
    assert_eq!(result.placements[2].content_box, [5., 12., 10., 1.]);
}

#[test]
fn margin_border_padding_and_relative_offset_have_distinct_rectangles() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_box_layout(
            0,
            &[
                boxed(LayoutBox {
                    size: [4., 5.],
                    margin: [1.; 4],
                    border: [1.; 4],
                    padding: [1.; 4],
                    offset: [2., -1.],
                    ..Default::default()
                }),
                boxed(LayoutBox {
                    size: [4., 1.],
                    ..Default::default()
                }),
            ],
            style(),
        )
        .unwrap();
    let p = &result.placements[0];
    assert_eq!(p.allocation, [2., 2., 10., 11.]);
    assert_eq!(p.border_box, [5., 2., 8., 9.]);
    assert_eq!(p.padding_box, [6., 3., 6., 7.]);
    assert_eq!(p.content_box, [7., 4., 4., 5.]);
    assert_eq!(result.placements[1].allocation[1], 13.);
}

#[test]
fn split_boxes_keep_columns_and_do_not_repeat_completed_boxes() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_box_layout(
            0,
            &[
                boxed(shared(4., 2., BoxFloat::Left, BoxClear::None)),
                boxed(shared(5., 22., BoxFloat::Right, BoxClear::None)),
            ],
            style(),
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1]);
    assert_eq!(result.placements.len(), 3);
    assert_eq!(result.placements[2].allocation, [13., 2., 5., 6.]);
    assert_eq!(result.placements[2].source_offset, 16.);
    assert!(!result.placements[2].first);
    assert!(result.placements[2].last);
    assert_eq!(result.next.unwrap().y, 8.);
}

#[test]
fn split_decoration_is_consumed_once_including_bottom_padding_and_border() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_box_layout(
            0,
            &[boxed(LayoutBox {
                size: [4., 15.],
                margin: [1.; 4],
                border: [1.; 4],
                padding: [1.; 4],
                ..Default::default()
            })],
            style(),
        )
        .unwrap();
    assert_eq!(result.placements.len(), 2);
    assert_eq!(result.placements[0].content_box, [5., 5., 4., 13.]);
    assert_eq!(result.placements[0].borders, [1., 1., 0., 1.]);
    assert_eq!(result.placements[1].allocation, [2., 2., 10., 5.]);
    assert_eq!(result.placements[1].content_box, [5., 2., 4., 2.]);
    assert_eq!(result.placements[1].borders, [0., 1., 1., 1.]);
    assert_eq!(result.next.unwrap().y, 7.);
}

#[test]
fn a_complete_box_can_wait_while_a_neighbor_splits() {
    let mut doc = OfdDocument::new();
    doc.append_blank_page(0, 20., 20.).unwrap();
    let mut style = style();
    style.start = Some(FlowStart {
        page_index: 0,
        y: 14.,
    });
    let result = doc
        .add_box_layout(
            0,
            &[
                boxed(LayoutBox {
                    integrity: true,
                    ..shared(4., 5., BoxFloat::Left, BoxClear::None)
                }),
                boxed(shared(4., 20., BoxFloat::Left, BoxClear::None)),
            ],
            style,
        )
        .unwrap();
    let sources: Vec<_> = result
        .placements
        .iter()
        .map(|p| (p.page_index, p.source_index))
        .collect();
    assert_eq!(sources, [(0, 1), (1, 0), (1, 1)]);
    assert_eq!(result.placements[0].allocation, [6., 14., 4., 4.]);
    assert_eq!(result.placements[1].allocation, [2., 2., 4., 5.]);
    assert_eq!(result.placements[2].allocation, [6., 2., 4., 16.]);
}

#[test]
fn a_complete_row_moves_together_without_touching_the_full_start_page() {
    let mut doc = OfdDocument::new();
    doc.append_blank_page(0, 20., 20.).unwrap();
    let mut style = style();
    style.start = Some(FlowStart {
        page_index: 0,
        y: 17.,
    });
    let result = doc
        .add_box_layout(
            0,
            &[
                boxed(LayoutBox {
                    integrity: true,
                    ..shared(4., 4., BoxFloat::Left, BoxClear::None)
                }),
                boxed(LayoutBox {
                    integrity: true,
                    ..shared(4., 5., BoxFloat::Right, BoxClear::None)
                }),
            ],
            style,
        )
        .unwrap();
    assert_eq!(result.pages, [1]);
    assert_eq!(result.placements[0].allocation, [2., 2., 4., 4.]);
    assert_eq!(
        result.next,
        Some(FlowStart {
            page_index: 1,
            y: 7.
        })
    );
}

#[test]
fn margin_only_fragments_and_placeholders_reserve_space_without_drawing() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_box_layout(
            0,
            &[
                boxed(LayoutBox {
                    size: [4., 2.],
                    margin: [20., 0., 0., 0.],
                    background_color: Some([255, 0, 0]),
                    ..Default::default()
                }),
                boxed(LayoutBox {
                    size: [4., 3.],
                    placeholder: true,
                    border: [1.; 4],
                    background_color: Some([0, 255, 0]),
                    ..Default::default()
                }),
            ],
            style(),
        )
        .unwrap();
    assert_eq!(result.placements[0].allocation, [2., 2., 4., 16.]);
    assert_eq!(result.placements[0].border_box[3], 0.);
    assert!(doc.page_paths(0).unwrap().is_empty());
    assert_eq!(doc.page_paths(1).unwrap().len(), 1);
    assert_eq!(result.next.unwrap().y, 13.);
}

#[test]
fn nonzero_existing_canvas_origins_reset_on_new_pages() {
    let mut doc = OfdDocument::new();
    doc.append_blank_page(0, 20., 20.).unwrap();
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
    let mut style = style();
    style.start = Some(FlowStart {
        page_index: 0,
        y: 14.,
    });
    let result = doc
        .add_box_layout(
            0,
            &[boxed(shared(4., 8., BoxFloat::Right, BoxClear::None))],
            style,
        )
        .unwrap();
    assert_eq!(result.placements[0].allocation, [24., 34., 4., 4.]);
    assert_eq!(result.placements[1].allocation, [14., 2., 4., 4.]);
    assert_eq!(
        result.next,
        Some(FlowStart {
            page_index: 1,
            y: 6.
        })
    );
}

#[test]
fn explicit_breaks_and_empty_input_have_predictable_page_effects() {
    let mut doc = OfdDocument::new();
    assert!(doc.add_box_layout(0, &[], style()).unwrap().next.is_none());
    assert!(doc
        .add_box_layout(0, &[BoxLayoutItem::LineBreak], style())
        .unwrap()
        .pages
        .is_empty());
    let result = doc
        .add_box_layout(
            0,
            &[
                BoxLayoutItem::PageBreak,
                BoxLayoutItem::LineBreak,
                boxed(LayoutBox {
                    size: [4., 2.],
                    ..Default::default()
                }),
                BoxLayoutItem::PageBreak,
                BoxLayoutItem::PageBreak,
            ],
            style(),
        )
        .unwrap();
    assert_eq!(result.pages, [0, 1, 2]);
    assert_eq!(result.placements[0].page_index, 0);
    assert_eq!(
        result.next,
        Some(FlowStart {
            page_index: 2,
            y: 2.
        })
    );
}

#[test]
fn invalid_later_boxes_and_invalid_starts_leave_the_source_unchanged() {
    let mut doc = OfdDocument::new();
    let before = doc.to_bytes().unwrap();
    for invalid in [
        LayoutBox {
            size: [-1., 2.],
            ..Default::default()
        },
        LayoutBox {
            size: [1., f64::NAN],
            ..Default::default()
        },
        LayoutBox {
            size: [1., 17.],
            integrity: true,
            ..Default::default()
        },
        LayoutBox {
            size: [1., 1e308],
            ..Default::default()
        },
        LayoutBox {
            size: [1., 1.],
            padding: [0., 20., 0., 0.],
            ..Default::default()
        },
        LayoutBox {
            size: [1., 1.],
            offset: [f64::INFINITY, 0.],
            ..Default::default()
        },
    ] {
        assert!(doc
            .add_box_layout(
                0,
                &[
                    boxed(LayoutBox {
                        size: [4., 2.],
                        ..Default::default()
                    }),
                    boxed(invalid)
                ],
                style()
            )
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    let mut invalid = style();
    invalid.start = Some(FlowStart {
        page_index: 0,
        y: 2.,
    });
    assert!(doc.add_box_layout(0, &[], invalid).is_err());
    assert!(doc.add_box_layout(1, &[], style()).is_err());
    assert!(doc
        .add_box_layout(0, &vec![BoxLayoutItem::LineBreak; 10_001], style())
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn background_and_border_alpha_do_not_accumulate_at_corners_and_survive_roundtrip() {
    let mut doc = OfdDocument::new();
    doc.add_box_layout(
        0,
        &[boxed(LayoutBox {
            size: [4.; 2],
            border: [1.; 4],
            background_color: Some([255, 0, 0]),
            border_color: [0, 0, 255],
            alpha: Some(128),
            ..Default::default()
        })],
        style(),
    )
    .unwrap();
    let png = doc.page_png(0, 254.).unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    let corner = image.get_pixel(25, 25);
    let side = image.get_pixel(25, 40);
    let inside = image.get_pixel(40, 40);
    assert_eq!(corner, side);
    assert!(corner[2] > corner[0]);
    assert!(inside[0] > inside[2]);
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(png, reopened.page_png(0, 254.).unwrap());
    assert!(reopened.validate_references().unwrap().is_empty());
}

#[test]
fn floating_rows_and_decoration_fragments_match_independently_generated_java_coordinates() {
    let reference = include_str!("fixtures/box-layout-java.tsv");
    for (name, boxes) in [
        (
            "left_right",
            vec![
                shared(4., 3., BoxFloat::Left, BoxClear::None),
                shared(5., 7., BoxFloat::Right, BoxClear::None),
                shared(3., 2., BoxFloat::Left, BoxClear::None),
            ],
        ),
        (
            "center",
            vec![
                shared(4., 3., BoxFloat::Center, BoxClear::None),
                shared(3., 4., BoxFloat::Center, BoxClear::None),
            ],
        ),
        (
            "clear",
            vec![
                shared(4., 3., BoxFloat::Left, BoxClear::None),
                shared(5., 4., BoxFloat::Left, BoxClear::Left),
                shared(3., 2., BoxFloat::Right, BoxClear::None),
            ],
        ),
        (
            "split",
            vec![LayoutBox {
                size: [4., 22.],
                ..Default::default()
            }],
        ),
        (
            "decoration",
            vec![LayoutBox {
                size: [4., 5.],
                margin: [1.; 4],
                border: [1.; 4],
                padding: [1.; 4],
                ..Default::default()
            }],
        ),
        (
            "split_decoration",
            vec![LayoutBox {
                size: [4., 15.],
                margin: [1.; 4],
                border: [1.; 4],
                padding: [1.; 4],
                ..Default::default()
            }],
        ),
    ] {
        let mut doc = OfdDocument::new();
        let items: Vec<_> = boxes.into_iter().map(boxed).collect();
        let result = doc.add_box_layout(0, &items, style()).unwrap();
        let expected: Vec<_> = reference
            .lines()
            .filter(|line| line.starts_with(&format!("{name}\t")))
            .collect();
        assert_eq!(result.placements.len(), expected.len(), "{name}");
        for (placement, line) in result.placements.iter().zip(expected) {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(
                placement.page_index,
                fields[1].parse::<usize>().unwrap(),
                "{name}"
            );
            assert_eq!(
                placement.source_index,
                fields[2].parse::<usize>().unwrap(),
                "{name}"
            );
            for (actual, expected) in placement.allocation.iter().zip(&fields[3..]) {
                assert!(
                    (actual - expected.parse::<f64>().unwrap()).abs() < 1e-8,
                    "{name}: {line}, got {placement:?}"
                );
            }
        }
    }
}

#[test]
fn every_vertical_cut_preserves_total_content_height_and_outer_edge_widths() {
    for cut in 1..10 {
        let mut doc = OfdDocument::new();
        doc.append_blank_page(0, 20., 20.).unwrap();
        let mut style = style();
        style.start = Some(FlowStart {
            page_index: 0,
            y: 18. - f64::from(cut),
        });
        let result = doc
            .add_box_layout(
                0,
                &[boxed(LayoutBox {
                    size: [4.; 2],
                    margin: [1.; 4],
                    padding: [1.; 4],
                    border: [1.; 4],
                    background_color: Some([255, 0, 0]),
                    ..Default::default()
                })],
                style,
            )
            .unwrap();
        assert_eq!(result.placements.len(), 2, "cut {cut}");
        assert_eq!(
            result
                .placements
                .iter()
                .map(|p| p.allocation[3])
                .sum::<f64>(),
            10.,
            "cut {cut}"
        );
        assert_eq!(
            result
                .placements
                .iter()
                .map(|p| p.content_box[3])
                .sum::<f64>(),
            4.,
            "cut {cut}"
        );
        assert_eq!(
            result.placements.iter().map(|p| p.borders[0]).sum::<f64>(),
            1.,
            "cut {cut}"
        );
        assert_eq!(
            result.placements.iter().map(|p| p.borders[2]).sum::<f64>(),
            1.,
            "cut {cut}"
        );
        assert_eq!(result.next.unwrap().y, 12. - f64::from(cut));
        assert!(doc.validate_references().unwrap().is_empty());
    }
}

#[test]
fn later_document_layout_preserves_following_document_pages() {
    let mut doc = OfdDocument::new();
    doc.append_blank_page(0, 20., 20.).unwrap();
    doc.append_document_body().unwrap();
    doc.append_blank_page(1, 20., 20.).unwrap();
    doc.append_document_body().unwrap();
    doc.append_blank_page(2, 20., 20.).unwrap();
    let later = doc.pages().unwrap()[2].path.clone();
    let before = doc.entry(&later).unwrap().unwrap().to_vec();
    let mut style = style();
    style.start = Some(FlowStart {
        page_index: 1,
        y: 17.,
    });
    let result = doc
        .add_box_layout(
            1,
            &[boxed(LayoutBox {
                size: [4.; 2],
                integrity: true,
                background_color: Some([255, 0, 0]),
                ..Default::default()
            })],
            style,
        )
        .unwrap();
    assert_eq!(result.pages, [2]);
    assert_eq!(result.placements[0].page_index, 2);
    assert_eq!(doc.pages().unwrap()[3].document_index, 2);
    assert_eq!(doc.entry(&later).unwrap().unwrap(), before);
    assert!(doc.validate_references().unwrap().is_empty());
}

#[test]
fn coordinate_overflow_during_drawing_rolls_back_new_objects() {
    let mut doc = OfdDocument::new();
    doc.append_blank_page(0, 20., 20.).unwrap();
    doc.set_page_area(
        0,
        PageArea {
            physical_box: [1e308, 0., 20., 20.],
            application_box: None,
            content_box: None,
            bleed_box: None,
        },
    )
    .unwrap();
    let before = doc.to_bytes().unwrap();
    let mut style = style();
    style.start = Some(FlowStart {
        page_index: 0,
        y: 2.,
    });
    assert!(doc
        .add_box_layout(
            0,
            &[
                boxed(LayoutBox {
                    size: [4.; 2],
                    background_color: Some([255, 0, 0]),
                    ..Default::default()
                }),
                boxed(LayoutBox {
                    size: [4.; 2],
                    offset: [1e308, 0.],
                    ..Default::default()
                })
            ],
            style
        )
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn invalid_page_geometry_is_rejected_before_creating_pages() {
    for bad in [
        BoxLayoutStyle {
            page_size: [f64::NAN, 20.],
            ..style()
        },
        BoxLayoutStyle {
            page_size: [0., 20.],
            ..style()
        },
        BoxLayoutStyle {
            margins: [-1.; 4],
            ..style()
        },
        BoxLayoutStyle {
            margins: [10.; 4],
            ..style()
        },
    ] {
        let mut doc = OfdDocument::new();
        assert!(doc
            .add_box_layout(
                0,
                &[boxed(LayoutBox {
                    size: [4.; 2],
                    ..Default::default()
                })],
                bad
            )
            .is_err());
        assert!(doc.pages().unwrap().is_empty());
    }
}

#[test]
fn zero_height_boxes_terminate_without_creating_spurious_rows() {
    let mut doc = OfdDocument::new();
    let result = doc
        .add_box_layout(
            0,
            &[
                boxed(LayoutBox {
                    size: [4., 0.],
                    ..Default::default()
                }),
                boxed(LayoutBox {
                    size: [4., 1.],
                    ..Default::default()
                }),
            ],
            style(),
        )
        .unwrap();
    assert_eq!(result.placements.len(), 2);
    assert_eq!(result.placements[1].allocation, [2., 2., 4., 1.]);
    assert_eq!(result.next.unwrap().y, 3.);
}

#[test]
fn content_rectangles_place_text_and_images_inside_shared_decorated_boxes() {
    let mut doc = OfdDocument::new();
    let items = [BoxFloat::Left, BoxFloat::Right].map(|float| {
        boxed(LayoutBox {
            size: [5.; 2],
            margin: [0.5; 4],
            padding: [0.5; 4],
            border: [0.25; 4],
            float,
            clear: BoxClear::None,
            ..Default::default()
        })
    });
    let result = doc.add_box_layout(0, &items, style()).unwrap();
    assert_eq!(result.placements.len(), 2);
    let text = &result.placements[0];
    let image = &result.placements[1];
    doc.add_text_object(text.page_index, "Hi", text.content_box, 2., None)
        .unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    doc.add_image_object(image.page_index, bytes.get_ref(), "PNG", image.content_box)
        .unwrap();
    assert_eq!(
        doc.page_text_objects(0).unwrap()[0].boundary,
        Some(text.content_box)
    );
    assert_eq!(
        doc.page_images(0).unwrap()[0].boundary,
        Some(image.content_box)
    );
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(0).unwrap(), ["Hi"]);
    assert_eq!(reopened.page_images(0).unwrap().len(), 1);
    assert_eq!(
        reopened.page_png(0, 144.).unwrap(),
        doc.page_png(0, 144.).unwrap()
    );
}
