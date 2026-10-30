mod common;
use lopdf::{Document as PdfDocument, Object, ObjectId};
use ofdrw::{OfdDocument, PageArea, PathStyle, ShapeAnnotationStyle};

fn fixture(name: &str) -> OfdDocument {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/annotation-ap-{name}.pdf"));
    OfdDocument::from_pdf_bytes(&std::fs::read(path).unwrap()).unwrap()
}

fn annots(pdf: &PdfDocument) -> Vec<ObjectId> {
    let page = *pdf.get_pages().values().next().unwrap();
    pdf.get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_reference().unwrap())
        .collect()
}

fn serialized(pdf: &mut PdfDocument) -> Vec<u8> {
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    bytes
}

fn raster(bytes: &[u8]) -> image::RgbImage {
    let doc = OfdDocument::from_pdf_bytes(bytes).unwrap();
    image::load_from_memory(&doc.page_png(0, 144.).unwrap())
        .unwrap()
        .to_rgb8()
}

#[test]
fn normal_appearance_is_an_independent_form_and_not_painted_in_page_content() {
    for name in [
        "basic", "link", "text", "image", "isolated", "empty", "hidden", "shared", "no-view",
        "nonprint",
    ] {
        let doc = fixture(name);
        let before = doc.to_bytes().unwrap();
        let bytes = doc.to_pdf().unwrap();
        assert_eq!(doc.to_bytes().unwrap(), before);
        let mut pdf = PdfDocument::load_mem(&bytes).unwrap();
        let refs = annots(&pdf);
        assert_eq!(refs.len(), if name == "shared" { 2 } else { 1 }, "{name}");
        for id in refs {
            let annotation = pdf.get_dictionary(id).unwrap();
            assert_eq!(
                annotation.get(b"Subtype").unwrap().as_name().unwrap(),
                if name == "link" {
                    b"Link".as_slice()
                } else {
                    b"Stamp".as_slice()
                }
            );
            let normal = annotation
                .get(b"AP")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"N")
                .unwrap()
                .as_reference()
                .unwrap();
            let form = pdf.get_object(normal).unwrap().as_stream().unwrap();
            assert_eq!(
                form.dict.get(b"Subtype").unwrap().as_name().unwrap(),
                b"Form"
            );
        }
        assert!(pdf
            .objects
            .values()
            .filter_map(|v| v.as_dict().ok())
            .all(|v| !v.has(b"OfdAppearanceId")));
        let page = *pdf.get_pages().values().next().unwrap();
        pdf.get_object_mut(page)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .remove(b"Annots");
        assert!(
            raster(&serialized(&mut pdf))
                .pixels()
                .all(|p| p.0 == [255, 255, 0]),
            "{name}: appearance was baked into page"
        );
    }
}

#[test]
fn no_view_print_and_invisible_flags_keep_their_distinct_meanings() {
    for (name, expected) in [
        ("no-view", 36),
        ("nonprint", 0),
        ("invisible-flag", 1),
        ("hidden", 6),
    ] {
        let doc = fixture(name);
        let pdf = PdfDocument::load_mem(&doc.to_pdf().unwrap()).unwrap();
        assert_eq!(
            pdf.get_dictionary(annots(&pdf)[0])
                .unwrap()
                .get(b"F")
                .unwrap()
                .as_i64()
                .unwrap(),
            expected,
            "{name}"
        );
        if let Some(gs) = std::env::var_os("OFDRW_GHOSTSCRIPT") {
            let dir = tempfile::tempdir().unwrap();
            let input = dir.path().join("export.pdf");
            let output = dir.path().join("export.png");
            std::fs::write(&input, doc.to_pdf().unwrap()).unwrap();
            let status = std::process::Command::new(gs)
                .args([
                    "-q",
                    "-dSAFER",
                    "-dBATCH",
                    "-dNOPAUSE",
                    "-dPrinted=true",
                    "-sDEVICE=png16m",
                    "-r144",
                    "-dGraphicsAlphaBits=1",
                ])
                .arg(format!("-sOutputFile={}", output.display()))
                .arg(input)
                .status()
                .unwrap();
            assert!(status.success());
            let expected = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("tests/fixtures/annotation-ap-{name}.print.png"));
            common::assert_raster_matches(
                &image::open(output).unwrap().to_rgb8(),
                &image::open(expected).unwrap().to_rgb8(),
                name,
            );
        }
    }
    let mut doc = fixture("no-view");
    let id = doc.annotations().unwrap()[0].id.clone();
    doc.set_annotation_visible(0, &id, true).unwrap();
    let pdf = PdfDocument::load_mem(&doc.to_pdf().unwrap()).unwrap();
    assert_eq!(
        pdf.get_dictionary(annots(&pdf)[0])
            .unwrap()
            .get(b"F")
            .unwrap()
            .as_i64()
            .unwrap(),
        4
    );
}

#[test]
fn links_keep_their_full_appearance_rectangle_and_multiple_hit_quads() {
    let mut source =
        PdfDocument::load_mem(include_bytes!("fixtures/annotation-ap-link.pdf")).unwrap();
    source
        .get_object_mut((6, 0))
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "QuadPoints",
            Object::Array(
                [
                    20, 16, 40, 16, 36, 28, 24, 28, 48, 24, 72, 24, 68, 40, 52, 40,
                ]
                .into_iter()
                .map(Object::Integer)
                .collect(),
            ),
        );
    let doc = OfdDocument::from_pdf_bytes(&serialized(&mut source)).unwrap();
    let pdf = PdfDocument::load_mem(&doc.to_pdf().unwrap()).unwrap();
    assert_eq!(annots(&pdf).len(), 1);
    let annotation = pdf.get_dictionary(annots(&pdf)[0]).unwrap();
    let rect = annotation
        .get(b"Rect")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_float().unwrap())
        .collect::<Vec<_>>();
    for (value, expected) in rect.iter().zip([16., 12., 80., 44.]) {
        assert!((*value - expected).abs() < 1e-4);
    }
    assert_eq!(
        annotation
            .get(b"QuadPoints")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        16
    );
    common::assert_raster_matches(
        &raster(&doc.to_pdf().unwrap()),
        &image::load_from_memory(&doc.page_png(0, 144.).unwrap())
            .unwrap()
            .to_rgb8(),
        "link AP and quads",
    );
}

#[test]
fn exported_stamp_can_be_resized_by_editing_only_its_pdf_rectangle() {
    let mut pdf = PdfDocument::load_mem(&fixture("basic").to_pdf().unwrap()).unwrap();
    let id = annots(&pdf)[0];
    pdf.get_object_mut(id).unwrap().as_dict_mut().unwrap().set(
        "Rect",
        Object::Array([24, 20, 56, 36].into_iter().map(Object::Integer).collect()),
    );
    let image = raster(&serialized(&mut pdf));
    assert_eq!(image.get_pixel(64, 68).0, [255, 0, 0]);
    assert_eq!(image.get_pixel(96, 80).0, [0, 0, 255]);
    assert_eq!(image.get_pixel(140, 68).0, [255, 255, 0]);
}

#[test]
fn mixed_native_and_imported_appearances_retain_annotation_paint_order() {
    let mut doc = fixture("basic");
    let boundary = doc.annotations().unwrap()[0].appearance_box.unwrap();
    doc.add_path_annotation(
        0,
        "M 0 0 L 30 0 L 30 20 L 0 20 C",
        boundary,
        PathStyle {
            fill: true,
            stroke: false,
            fill_color: Some([0, 255, 0]),
            ..PathStyle::default()
        },
    )
    .unwrap();
    doc.add_shape_annotation(
        0,
        "Square",
        [boundary[0] + 3., boundary[1] + 2., 5., 4.],
        &ShapeAnnotationStyle {
            stroke_color: None,
            fill_color: Some([0, 0, 255]),
            line_width: 0.,
            ..ShapeAnnotationStyle::default()
        },
    )
    .unwrap();
    let expected = image::load_from_memory(&doc.page_png(0, 144.).unwrap())
        .unwrap()
        .to_rgb8();
    common::assert_raster_matches(
        &raster(&doc.to_pdf().unwrap()),
        &expected,
        "mixed annotation order",
    );
}

#[test]
fn metadata_names_and_popup_survive_native_and_pdf_roundtrips() {
    let mut doc = fixture("shared");
    let ids = doc
        .annotations()
        .unwrap()
        .into_iter()
        .map(|a| a.id)
        .collect::<Vec<_>>();
    let link = doc
        .add_uri_link_annotation(0, "https://example.com/plain", [0., 0., 2., 2.])
        .unwrap();
    for id in ids.iter().chain(std::iter::once(&link)) {
        doc.set_annotation_parameter(0, id, "PDFName", "批注🙂")
            .unwrap();
        doc.set_annotation_creator(0, id, "作者🙂").unwrap();
        doc.set_annotation_remark(0, id, Some("备注🙂")).unwrap();
    }
    doc.set_annotation_parameter(0, &ids[0], "PDFPopupBoundary", "1 2 8 6")
        .unwrap();
    doc.set_annotation_parameter(0, &ids[0], "PDFPopupOpen", "true")
        .unwrap();
    let bytes = doc.to_pdf().unwrap();
    let pdf = PdfDocument::load_mem(&bytes).unwrap();
    let names = annots(&pdf)
        .into_iter()
        .filter_map(|id| pdf.get_dictionary(id).unwrap().get(b"NM").ok())
        .map(|v| lopdf::decode_text_string(v).unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(names.len(), 3);
    let reopened = OfdDocument::from_pdf_bytes(&bytes).unwrap();
    let annotations = reopened.annotations().unwrap();
    assert_eq!(annotations.len(), 3);
    assert!(annotations
        .iter()
        .all(|a| a.remark.as_deref() == Some("备注🙂")
            && a.attributes.get("Creator").map(String::as_str) == Some("作者🙂")));
    assert_eq!(
        annotations[0]
            .parameters
            .get("PDFPopupOpen")
            .map(String::as_str),
        Some("true")
    );
}

#[test]
fn application_clipping_and_other_pages_keep_their_own_coordinate_systems() {
    let mut doc = fixture("basic");
    let physical = doc.pages().unwrap()[0].physical_box.unwrap();
    doc.set_page_area(
        0,
        PageArea {
            physical_box: physical,
            application_box: Some([6., 6., 12., 8.]),
            content_box: None,
            bleed_box: None,
        },
    )
    .unwrap();
    doc.add_text_page("other page", 40., 30.).unwrap();
    let expected = doc.page_png(0, 144.).unwrap();
    let pdf = doc.to_pdf().unwrap();
    let reopened = OfdDocument::from_pdf_bytes(&pdf).unwrap();
    assert_eq!(reopened.pages().unwrap().len(), 2);
    assert!(reopened.validate_references().unwrap().is_empty());
    common::assert_raster_matches(
        &image::load_from_memory(&reopened.page_png(0, 144.).unwrap())
            .unwrap()
            .to_rgb8(),
        &image::load_from_memory(&expected).unwrap().to_rgb8(),
        "annotation application clip",
    );
    assert_eq!(reopened.page_text(1).unwrap().concat(), "other page");
}

#[test]
fn invalid_retained_flags_fail_export_without_changing_the_source() {
    let mut doc = fixture("basic");
    let id = doc.annotations().unwrap()[0].id.clone();
    for value in ["invalid", "-1", "4294967296"] {
        doc.set_annotation_parameter(0, &id, "PDFSourceFlags", value)
            .unwrap();
        let before = doc.to_bytes().unwrap();
        assert!(
            matches!(doc.to_pdf(),Err(ofdrw::OfdError::Invalid(message)) if message.contains("PDFSourceFlags"))
        );
        assert_eq!(doc.to_bytes().unwrap(), before);
    }
}
