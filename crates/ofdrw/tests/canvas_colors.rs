use ofdrw::{parse_canvas_color, SolidPaintStyle};

#[test]
fn canvas_colors_match_independently_executed_java_named_color() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/canvas-color-java.json")).unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert!(cases.len() > 450);
    for case in cases {
        let input = case["input"].as_str().unwrap();
        let expected = case["expected"].as_array().map(|values| SolidPaintStyle {
            color: std::array::from_fn(|i| values[i].as_u64().unwrap() as u8),
            alpha: values
                .get(3)
                .map_or(255, |value| value.as_u64().unwrap() as u8),
        });
        assert_eq!(parse_canvas_color(input), expected, "{input:?}");
    }
}

#[test]
fn invalid_canvas_channels_are_rejected_without_java_exceptions_or_wrapping() {
    for input in [
        "rgb(-1,0,0)",
        "rgb(256,0,0)",
        "rgb(1,2,3,4)",
        "rgba(1,2,3,NaN)",
        "rgba(1,2,3,inf)",
        "rgba(1,2,3,-0.1)",
        "rgba(1,2,3,1.1)",
        "rgba(1,2,3,1,2)",
        "#-1",
        "--1",
        "#éé",
        "rgb(1,2,😀)",
    ] {
        assert_eq!(parse_canvas_color(input), None, "{input:?}");
    }
    assert_eq!(parse_canvas_color(&"1".repeat(4097)), None);
}

#[test]
fn parsed_canvas_colors_paint_and_round_trip_with_byte_opacity() {
    use ofdrw::{CanvasCommand, DrawParamStyle, OfdDocument};
    for (input, expected) in [
        ("red", [255, 0, 0]),
        ("#abc", [170, 187, 204]),
        ("rgba(10,20,30,0.5)", [133, 138, 143]),
        ("rgba(10,20,30,0)", [255, 255, 255]),
    ] {
        let mut document = OfdDocument::new();
        document.add_blank_page(40., 30.).unwrap();
        document
            .draw_canvas(
                0,
                [0., 0., 40., 30.],
                &[
                    CanvasCommand::SetDrawParam {
                        style: Box::new(DrawParamStyle {
                            fill: Some(parse_canvas_color(input).unwrap()),
                            ..Default::default()
                        }),
                    },
                    CanvasCommand::FillRect {
                        rect: [2., 2., 20., 15.],
                    },
                ],
            )
            .unwrap();
        let png = document.page_png(0, 25.4).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgb8();
        for (actual, expected) in image.get_pixel(10, 10).0.into_iter().zip(expected) {
            assert!(
                actual.abs_diff(expected) <= 1,
                "{input}: {actual}, {expected}"
            );
        }
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_png(0, 25.4).unwrap(), png, "{input}");
        assert!(reopened.validate_references().unwrap().is_empty());
        assert!(lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok());
    }
}
