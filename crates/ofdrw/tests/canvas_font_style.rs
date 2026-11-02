use ofdrw::{parse_canvas_font_style, CanvasCommand, CanvasFont, CanvasTextAlign, OfdDocument};

#[test]
fn font_styles_match_java_and_reuse_existing_embedded_resources() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/canvas-font-style-java.json")).unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 986);
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let data = include_bytes!("fixtures/dejavu-sans.ttf");
    let ids = [
        doc.add_embedded_font("CanvasTest", data)
            .unwrap()
            .to_string(),
        doc.add_embedded_font("宋体", data).unwrap().to_string(),
    ];
    let before = doc.to_bytes().unwrap();
    for case in cases {
        let base = CanvasFont {
            size: 9.,
            weight: case["base_weight"].as_u64().unwrap() as u16,
            italic: case["base_italic"].as_bool().unwrap(),
            letter_spacing: 0.25,
            read_direction: 90,
            char_direction: 270,
            align: CanvasTextAlign::End,
            ..Default::default()
        };
        let style = doc
            .resolve_canvas_font_style(
                0,
                case["input"].as_str().unwrap(),
                &base,
                case["pixels_per_mm"].as_f64().unwrap(),
            )
            .unwrap();
        let expected = &case["expected"];
        assert!(
            (style.size - expected["size"].as_f64().unwrap()).abs() < 1e-12,
            "{case}"
        );
        assert_eq!(
            style.weight,
            expected["weight"].as_u64().unwrap() as u16,
            "{case}"
        );
        assert_eq!(
            style.italic,
            expected["italic"].as_bool().unwrap(),
            "{case}"
        );
        assert_eq!(
            style.font_id.as_ref(),
            Some(&ids[usize::from(expected["family"] == "宋体")])
        );
        assert_eq!(style.letter_spacing, base.letter_spacing);
        assert_eq!(
            (style.read_direction, style.char_direction, style.align),
            (base.read_direction, base.char_direction, base.align)
        );
    }
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn font_shorthand_supports_complete_multiword_families_and_quotes() {
    for input in [
        "italic bold 12px Times New Roman",
        "bold italic 12px 'Times New Roman'",
        "italic bold 12px \"Times New Roman\"",
    ] {
        let style = parse_canvas_font_style(input, 4.).unwrap();
        assert_eq!(style.family, "Times New Roman");
        assert_eq!(style.size, 3.);
        assert_eq!(style.weight, Some(700));
        assert_eq!(style.italic, Some(true));
    }
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/canvas-font-style-java.json")).unwrap();
    assert_eq!(
        reference["known_java_misparses"][0]["expected"]["family"],
        "Roman"
    );
    assert_eq!(reference["known_java_misparses"][0]["expected"]["size"], 1.);
    assert_eq!(
        reference["known_java_misparses"][2]["expected"]["weight"],
        400
    );
    assert_eq!(
        reference["known_java_misparses"][3]["expected"]["italic"],
        false
    );
    assert!(parse_canvas_font_style("ITALIC BOLD 12px CanvasTest", 3.78)
        .unwrap()
        .italic
        .unwrap());
    assert_eq!(
        parse_canvas_font_style("4mm 'Comma, Family & Name'", 3.78)
            .unwrap()
            .family,
        "Comma, Family & Name"
    );
    let numeric = parse_canvas_font_style("100 Times New Roman", 3.78).unwrap();
    assert_eq!(numeric.size, 100.);
    assert_eq!(numeric.weight, None);
}

#[test]
fn invalid_font_styles_and_pages_never_mutate_resources() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let before = doc.to_bytes().unwrap();
    for input in [
        "",
        "Font",
        "0px Font",
        "-1mm Font",
        "NaNpx Font",
        "1e-100mm Font",
        "1e30mm Font",
        "1e999px Font",
        "bold 12pt Font",
        "12px",
        "italic italic 12px Font",
        "100 200 4mm Font",
        "12px 'Font",
        "12px A,B",
        "12px A;B",
        "12px A\0B",
    ] {
        assert!(
            doc.resolve_canvas_font_style(0, input, &CanvasFont::default(), 3.78)
                .is_err(),
            "{input}"
        );
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
    for ppm in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(doc
            .resolve_canvas_font_style(0, "12px Font", &CanvasFont::default(), ppm)
            .is_err());
    }
    assert!(doc
        .resolve_canvas_font_style(1, "12px Font", &CanvasFont::default(), 3.78)
        .is_err());
    assert!(parse_canvas_font_style(&"a".repeat(4097), 3.78).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn named_font_creation_preserves_xml_and_draws_after_reopening() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let base = CanvasFont::default();
    let font = doc
        .resolve_canvas_font_style(0, "italic bold 4mm 'Font & Family'", &base, 3.78)
        .unwrap();
    let before = doc.to_bytes().unwrap();
    assert_eq!(
        doc.resolve_canvas_font_style(0, "italic bold 4mm 'Font & Family'", &base, 3.78)
            .unwrap(),
        font
    );
    assert_eq!(doc.to_bytes().unwrap(), before);
    assert_eq!(
        doc.fonts()
            .unwrap()
            .iter()
            .find(|item| Some(&item.id) == font.font_id.as_ref())
            .unwrap()
            .font_name,
        "Font & Family"
    );
    doc.draw_canvas(
        0,
        [0., 0., 40., 30.],
        &[
            CanvasCommand::SetFont { font },
            CanvasCommand::FillText {
                text: "Style".into(),
                point: [3., 12.],
            },
        ],
    )
    .unwrap();
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(0).unwrap(), vec!["Style"]);
    assert!(reopened.validate_references().unwrap().is_empty());
    assert_eq!(
        reopened.page_png(0, 144.).unwrap(),
        doc.page_png(0, 144.).unwrap()
    );
    assert!(lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok());
}

#[test]
fn ambiguous_font_names_fail_before_changing_resources() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let data = include_bytes!("fixtures/dejavu-sans.ttf");
    doc.add_embedded_font("Duplicate", data).unwrap();
    doc.add_embedded_font("Duplicate", data).unwrap();
    let before = doc.to_bytes().unwrap();
    assert!(doc
        .resolve_canvas_font_style(0, "4mm Duplicate", &CanvasFont::default(), 3.78)
        .is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
}

#[test]
fn resolved_pixel_font_matches_independent_java_canvas_writer_pixels() {
    let java = OfdDocument::from_bytes(include_bytes!(
        "fixtures/canvas-font-style-java-restored-font.ofd"
    ))
    .unwrap();
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    doc.set_page_area(
        0,
        ofdrw::PageArea {
            physical_box: [0., 0., 40., 30.],
            application_box: Some([0., 0., 40., 30.]),
            content_box: None,
            bleed_box: None,
        },
    )
    .unwrap();
    doc.add_embedded_font("CanvasTest", include_bytes!("fixtures/dejavu-sans.ttf"))
        .unwrap();
    let base = CanvasFont {
        weight: 600,
        italic: true,
        letter_spacing: 0.25,
        ..Default::default()
    };
    let font = doc
        .resolve_canvas_font_style(0, "italic bold 12px CanvasTest", &base, 3.78)
        .unwrap();
    doc.draw_canvas(
        0,
        [0., 0., 30., 20.],
        &[
            CanvasCommand::SetFont { font },
            CanvasCommand::FillText {
                text: "WiA".into(),
                point: [2., 12.],
            },
        ],
    )
    .unwrap();
    assert_eq!(doc.page_text(0).unwrap(), java.page_text(0).unwrap());
    let raw =
        OfdDocument::from_bytes(include_bytes!("fixtures/canvas-font-style-java.ofd")).unwrap();
    assert_eq!(raw.page_text(0).unwrap(), doc.page_text(0).unwrap());
    let provenance: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/canvas-font-style-java.json")).unwrap();
    assert_eq!(provenance["font_lookup_loses_embedded_file"], true);
    for dpi in [25.4, 101.6] {
        assert!(
            doc.page_png(0, dpi).unwrap() == java.page_png(0, dpi).unwrap(),
            "Java writer raster at {dpi} DPI"
        );
    }
}

#[test]
fn fractional_font_metrics_match_independent_java_at_small_and_large_sizes() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/canvas-fractional-java.json")).unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 1296);
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut fonts = std::collections::BTreeMap::new();
    for name in [
        "dejavu-sans.ttf",
        "dejavu-serif.ttf",
        "kerning-simhei.ttf",
        "test-name-cff.otf",
    ] {
        fonts.insert(
            name,
            doc.add_embedded_font(name, &std::fs::read(root.join(name)).unwrap())
                .unwrap()
                .to_string(),
        );
    }
    for case in cases {
        let font = CanvasFont {
            font_id: Some(fonts[case["font"].as_str().unwrap()].clone()),
            size: case["size"].as_f64().unwrap(),
            letter_spacing: case["letter_spacing"].as_f64().unwrap(),
            ..Default::default()
        };
        let actual = doc
            .measure_canvas_text_area(0, case["text"].as_str().unwrap(), &font)
            .unwrap();
        assert_eq!(
            actual.width,
            case["width"].as_f64().unwrap(),
            "width {case}"
        );
        assert_eq!(
            actual.height,
            case["height"].as_f64().unwrap(),
            "height {case}"
        );
        for (actual, expected) in actual
            .char_areas
            .iter()
            .zip(case["char_areas"].as_array().unwrap())
        {
            let expected: [f64; 4] = std::array::from_fn(|i| expected[i].as_f64().unwrap());
            assert_eq!(actual, &expected, "boxes {case}");
        }
    }
}
