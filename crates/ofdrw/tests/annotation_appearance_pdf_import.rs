mod common;
use ofdrw::OfdDocument;

fn check(name: &str, source: &[u8], reference: &[u8]) {
    let document = OfdDocument::from_pdf_bytes(source).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(document.pages().unwrap().len(), 1, "scratch page leaked");
    let annotations = document.annotations().unwrap();
    assert_eq!(annotations.len(), if name == "shared" { 2 } else { 1 });
    assert!(document.validate_references().unwrap().is_empty(), "{name}");
    let png = document.page_png(0, 144.).unwrap();
    let actual = image::load_from_memory(&png).unwrap().to_rgb8();
    let expected = image::load_from_memory(reference).unwrap().to_rgb8();
    common::assert_raster_matches(&actual, &expected, name);
    if name == "text" {
        assert!(document.page_svg(0).unwrap().contains("AP"));
    }
    if name != "shared" {
        assert_eq!(annotations[0].remark.as_deref(), Some("custom appearance"));
    }
    if name == "link" {
        assert!(annotations[0].parameters.contains_key("PDFSourceSubtype"));
        assert_eq!(
            annotations[0].actions[0]
                .attributes
                .get("URI")
                .map(String::as_str),
            Some("https://example.com/appearance")
        );
    }
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_png(0, 144.).unwrap(), png);
    assert!(reopened.validate_references().unwrap().is_empty());
    let pdf = reopened.to_pdf().unwrap();
    assert!(lopdf::Document::load_mem(&pdf).is_ok());
    if let Some(gs) = std::env::var_os("OFDRW_GHOSTSCRIPT") {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("export.pdf");
        let output = dir.path().join("export.png");
        std::fs::write(&input, pdf).unwrap();
        let status = std::process::Command::new(gs)
            .args([
                "-q",
                "-dSAFER",
                "-dBATCH",
                "-dNOPAUSE",
                "-sDEVICE=png16m",
                "-r144",
                "-dGraphicsAlphaBits=1",
            ])
            .arg(format!("-sOutputFile={}", output.display()))
            .arg(input)
            .status()
            .unwrap();
        assert!(status.success());
        common::assert_raster_matches(
            &image::open(output).unwrap().to_rgb8(),
            &expected,
            &format!("{name} export"),
        );
    }
}

macro_rules! case {
    ($test:ident,$name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_bytes!(concat!("fixtures/annotation-ap-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/annotation-ap-", $name, ".png")),
            );
        }
    };
}
case!(basic, "basic");
case!(bbox, "bbox");
case!(matrix, "matrix");
case!(reflected, "reflected");
case!(rotate90, "rotate90");
case!(rotate180, "rotate180");
case!(rotate270, "rotate270");
case!(crop, "crop");
case!(user_unit, "user-unit");
case!(states, "states");
case!(indirect, "indirect");
case!(inherited, "inherited");
case!(shared, "shared");
case!(empty, "empty");
case!(hidden, "hidden");
case!(link, "link");
case!(text, "text");
case!(image, "image");
case!(isolated, "isolated");
case!(alpha, "alpha");
case!(widget, "widget");
case!(free_text, "free-text");
case!(annotation_alpha, "annotation-alpha");
case!(no_rotate, "no-rotate");

fn modified(name: &str, change: impl FnOnce(&mut lopdf::Document)) -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/annotation-ap-{name}.pdf"));
    let mut pdf = lopdf::Document::load(path).unwrap();
    change(&mut pdf);
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn state_selection_and_filtered_normal_stream() {
    let off = modified("states", |pdf| {
        pdf.get_object_mut((6, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("AS", lopdf::Object::Name(b"Off".to_vec()));
    });
    let doc = OfdDocument::from_pdf_bytes(&off).unwrap();
    let png = image::load_from_memory(&doc.page_png(0, 144.).unwrap())
        .unwrap()
        .to_rgb8();
    assert_eq!(png.get_pixel(80, 64).0, [0, 255, 0]);
    let compressed = modified("basic", |pdf| {
        pdf.get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .compress()
            .unwrap();
        // Rollover/down appearances must not replace N in a fixed import.
        let ap = pdf
            .get_object_mut((6, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"AP")
            .unwrap()
            .as_dict_mut()
            .unwrap();
        ap.set("R", lopdf::Object::Reference((10, 0)));
        ap.set("D", lopdf::Object::Reference((10, 0)));
    });
    let baseline =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/annotation-ap-basic.pdf")).unwrap();
    assert_eq!(
        OfdDocument::from_pdf_bytes(&compressed)
            .unwrap()
            .page_png(0, 144.)
            .unwrap(),
        baseline.page_png(0, 144.).unwrap()
    );
}

#[test]
fn malformed_appearance_dictionaries_states_and_geometry_return_errors() {
    use lopdf::{dictionary, Object};
    for (key, value) in [
        ("AP", Object::Null),
        ("AP", Object::Dictionary(dictionary! {})),
        ("AP", Object::Dictionary(dictionary! {"N"=>1})),
        (
            "AP",
            Object::Dictionary(
                dictionary! {"N"=>Object::Dictionary(dictionary!{"Off"=>Object::Reference((5,0))})},
            ),
        ),
        (
            "Rect",
            Object::Array(vec![0.into(), 0.into(), 0.into(), 20.into()]),
        ),
    ] {
        let bytes = modified("basic", |pdf| {
            pdf.get_object_mut((6, 0))
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set(key, value);
        });
        assert!(OfdDocument::from_pdf_bytes(&bytes).is_err(), "{key}");
    }
    for (key, value) in [
        (
            "BBox",
            Object::Array(vec![0.into(), 0.into(), 0.into(), 20.into()]),
        ),
        ("BBox", Object::Array(vec![0.into(), 0.into(), 20.into()])),
        (
            "Matrix",
            Object::Array(vec![
                0.into(),
                0.into(),
                0.into(),
                0.into(),
                0.into(),
                0.into(),
            ]),
        ),
        ("Matrix", Object::Name(b"Invalid".to_vec())),
        ("Subtype", Object::Name(b"Image".to_vec())),
    ] {
        let bytes = modified("basic", |pdf| {
            pdf.get_object_mut((5, 0))
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .set(key, value);
        });
        assert!(OfdDocument::from_pdf_bytes(&bytes).is_err(), "{key}");
    }
    for state in [Object::Name(b"Missing".to_vec()), 1.into()] {
        let bytes = modified("states", |pdf| {
            pdf.get_object_mut((6, 0))
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("AS", state);
        });
        assert!(OfdDocument::from_pdf_bytes(&bytes).is_err());
    }
    let cyclic = modified("inherited", |pdf| {
        let stream = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
        stream.content = b"/OfdNormalAppearance Do".to_vec();
        stream.dict.set("Length", stream.content.len() as i64);
    });
    assert!(
        matches!(OfdDocument::from_pdf_bytes(&cyclic),Err(ofdrw::OfdError::Invalid(message)) if message.contains("cyclic"))
    );
}

#[test]
fn appearance_visibility_print_flags_and_optional_content_remain_effective() {
    use lopdf::{dictionary, Object};
    let base =
        OfdDocument::from_pdf_bytes(include_bytes!("fixtures/annotation-ap-basic.pdf")).unwrap();
    let hidden = OfdDocument::from_pdf_bytes(include_bytes!("fixtures/annotation-ap-hidden.pdf"))
        .unwrap()
        .page_png(0, 144.)
        .unwrap();
    for flags in [2, 32] {
        let bytes = modified("basic", |pdf| {
            pdf.get_object_mut((6, 0))
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("F", flags);
        });
        assert_eq!(
            OfdDocument::from_pdf_bytes(&bytes)
                .unwrap()
                .page_png(0, 144.)
                .unwrap(),
            hidden
        );
    }
    let bytes = modified("basic", |pdf| {
        pdf.get_object_mut((6, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("F", 0);
    });
    let unprinted = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    assert_eq!(
        unprinted.page_png(0, 144.).unwrap(),
        base.page_png(0, 144.).unwrap()
    );
    assert_eq!(
        OfdDocument::from_pdf_bytes(&unprinted.to_pdf().unwrap())
            .unwrap()
            .page_png(0, 144.)
            .unwrap(),
        hidden
    );
    let bytes = modified("basic", |pdf| {
        pdf.objects.insert(
            (17, 0),
            Object::Dictionary(
                dictionary! {"Type"=>"OCG","Name"=>Object::string_literal("hidden")},
            ),
        );
        pdf.catalog_mut().unwrap().set("OCProperties",dictionary!{"OCGs"=>vec![Object::Reference((17,0))],"D"=>dictionary!{"BaseState"=>"OFF"}});
        pdf.get_object_mut((6, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("OC", Object::Reference((17, 0)));
    });
    let hidden_group = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    assert!(hidden_group.annotations().unwrap().is_empty());
    assert_eq!(hidden_group.page_png(0, 144.).unwrap(), hidden);
}

#[test]
fn custom_appearance_preserves_link_quads_and_supported_source_subtypes() {
    use lopdf::Object;
    let bytes = modified("link", |pdf| {
        pdf.get_object_mut((6, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set(
                "QuadPoints",
                Object::Array(
                    [16, 12, 80, 12, 72, 44, 24, 44]
                        .into_iter()
                        .map(Object::Integer)
                        .collect(),
                ),
            );
    });
    let link = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    let annotations = link.annotations().unwrap();
    assert_eq!(
        annotations[0].actions[0]
            .region
            .as_ref()
            .unwrap()
            .areas
            .len(),
        1
    );
    let output = lopdf::Document::load_mem(&link.to_pdf().unwrap()).unwrap();
    let page = *output.get_pages().values().next().unwrap();
    let list = output
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap();
    let annot = output
        .get_dictionary(list[0].as_reference().unwrap())
        .unwrap();
    assert_eq!(
        annot.get(b"QuadPoints").unwrap().as_array().unwrap().len(),
        8
    );
    for subtype in [
        "Popup",
        "Text",
        "Square",
        "Circle",
        "Line",
        "Ink",
        "Polygon",
        "PolyLine",
        "Highlight",
        "Underline",
        "StrikeOut",
        "Squiggly",
        "Caret",
        "Redact",
        "Watermark",
    ] {
        let bytes = modified("basic", |pdf| {
            pdf.get_object_mut((6, 0))
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Subtype", Object::Name(subtype.as_bytes().to_vec()));
        });
        let doc = OfdDocument::from_pdf_bytes(&bytes).unwrap();
        assert_eq!(
            doc.annotations().unwrap()[0].subtype.as_deref(),
            Some(subtype)
        );
        assert!(doc.validate_references().unwrap().is_empty());
    }
}
