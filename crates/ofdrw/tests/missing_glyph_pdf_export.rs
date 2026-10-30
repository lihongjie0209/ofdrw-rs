//! Missing glyphs retain distinct per-occurrence Unicode without a CJK font.
fn convert(text: &str, font: &[u8], family: &str) -> Vec<u8> {
    let mut options = svg2pdf::usvg::Options::default();
    options.fontdb_mut().load_font_data(font.to_vec());
    options.font_resolver.select_fallback = Box::new(|_, _, _| None);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="80"><text x="10" y="40" font-family="{family}" font-size="20">{text}</text></svg>"#
    );
    let tree = svg2pdf::usvg::Tree::from_str(&svg, &options).unwrap();
    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .unwrap()
}

fn check_with_font(text: &str, replacements: &[&str], font: &[u8], family: &str) {
    let bytes = convert(text, font, family);
    let imported = ofdrw::OfdDocument::from_pdf_bytes(&bytes).unwrap();
    assert_eq!(imported.page_text(0).unwrap().concat(), text);
    let reopened = ofdrw::OfdDocument::from_bytes(&imported.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(0).unwrap().concat(), text);
    assert!(reopened.validate_references().unwrap().is_empty());
    let exported = reopened.to_pdf().unwrap();
    assert_eq!(
        ofdrw::OfdDocument::from_pdf_bytes(&exported)
            .unwrap()
            .page_text(0)
            .unwrap()
            .concat(),
        text
    );
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let mut actual = Vec::new();
    for object in pdf.objects.values() {
        let Ok(stream) = object.as_stream() else {
            continue;
        };
        let Ok(data) = stream.get_plain_content() else {
            continue;
        };
        let Ok(content) = lopdf::content::Content::decode(&data) else {
            continue;
        };
        for operation in content.operations {
            if operation.operator != "BDC" {
                continue;
            }
            let Ok(dict) = operation.operands[1].as_dict() else {
                continue;
            };
            let value = dict.get(b"ActualText").unwrap().as_str().unwrap();
            assert_eq!(&value[..2], &[0xfe, 0xff]);
            let units = value[2..]
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect::<Vec<_>>();
            actual.push(String::from_utf16(&units).unwrap());
        }
    }
    assert_eq!(actual, replacements);
    let mut legacy = pdf.clone();
    for object in legacy.objects.values_mut() {
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        let Ok(data) = stream.get_plain_content() else {
            continue;
        };
        let Ok(mut content) = lopdf::content::Content::decode(&data) else {
            continue;
        };
        if !content.operations.iter().any(|op| op.operator == "BDC") {
            continue;
        }
        content
            .operations
            .retain(|op| op.operator != "BDC" && op.operator != "EMC");
        stream.set_plain_content(content.encode().unwrap());
    }
    let mut legacy_bytes = Vec::new();
    legacy.save_to(&mut legacy_bytes).unwrap();
    let legacy_native = ofdrw::OfdDocument::from_pdf_bytes(&legacy_bytes).unwrap();
    assert_eq!(
        imported.page_png(0, 72.).unwrap(),
        legacy_native.page_png(0, 72.).unwrap(),
        "ActualText changed native painted pixels"
    );
    if let Some(gs) = std::env::var_os("OFDRW_GHOSTSCRIPT") {
        let temp = tempfile::tempdir().unwrap();
        let before = temp.path().join("before.pdf");
        legacy.save(&before).unwrap();
        let after = temp.path().join("after.pdf");
        std::fs::write(&after, &bytes).unwrap();
        let mut rasters = Vec::new();
        for (index, input) in [before, after].iter().enumerate() {
            let output = temp.path().join(format!("{index}.png"));
            let result = std::process::Command::new(&gs)
                .args([
                    "-q",
                    "-dSAFER",
                    "-dBATCH",
                    "-dNOPAUSE",
                    "-sDEVICE=png16m",
                    "-r144",
                ])
                .arg(format!("-sOutputFile={}", output.display()))
                .arg(input)
                .output()
                .unwrap();
            assert!(result.status.success(), "{result:?}");
            rasters.push(image::open(output).unwrap().to_rgb8());
        }
        assert_eq!(rasters[0], rasters[1], "ActualText changed painted pixels");
    }
    if let Some(reader) = std::env::var_os("OFDRW_PDFTOTEXT") {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("missing.pdf");
        std::fs::write(&path, &bytes).unwrap();
        let result = std::process::Command::new(reader)
            .args(["-raw", "-enc", "UTF-8"])
            .arg(path)
            .arg("-")
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), text);
    }
}

fn check(text: &str, replacements: &[&str]) {
    check_with_font(
        text,
        replacements,
        include_bytes!("fixtures/dejavu-sans.ttf"),
        "DejaVu Sans",
    );
}

#[test]
fn name_keyed_cff_missing_characters() {
    check_with_font(
        "中文",
        &["中", "文"],
        include_bytes!("fixtures/test-name-cff.otf"),
        "OFD Test CFF",
    );
}

#[test]
fn cid_keyed_cff_missing_characters() {
    check_with_font(
        "中文",
        &["中", "文"],
        include_bytes!("fixtures/test-cid-cff.otf"),
        "OFD Test CFF",
    );
}

#[test]
fn distinct_missing_characters() {
    check("中文", &["中", "文"]);
}
#[test]
fn repeated_missing_characters() {
    check("中文中", &["中", "文", "中"]);
}
#[test]
fn existing_and_missing_glyphs() {
    check("A中文B", &["中", "文"]);
}
#[test]
fn supplementary_missing_character() {
    check("A\u{31350}中B", &["\u{31350}", "中"]);
}
#[test]
fn supported_glyphs_do_not_gain_actual_text() {
    check("AB", &[]);
}

#[test]
fn native_cff_actual_text_preserves_paint_and_repeated_round_trips() {
    let original =
        ofdrw::OfdDocument::from_pdf_bytes(include_bytes!("fixtures/type1-raw-cff.pdf")).unwrap();
    let document =
        ofdrw::OfdDocument::from_pdf_bytes(include_bytes!("fixtures/actual-text-cff.pdf")).unwrap();
    assert_eq!(document.page_text(0).unwrap(), ["中", "文"]);
    assert_eq!(
        document.page_png(0, 72.).unwrap(),
        original.page_png(0, 72.).unwrap()
    );
    let mut bytes = document.to_pdf().unwrap();
    for _ in 0..2 {
        let imported = ofdrw::OfdDocument::from_pdf_bytes(&bytes).unwrap();
        assert_eq!(imported.page_text(0).unwrap().concat(), "中文");
        assert!(imported.validate_references().unwrap().is_empty());
        bytes = imported.to_pdf().unwrap();
    }
}

fn wrap_replacement(source: &[u8], text: &str) -> Vec<u8> {
    let mut pdf = lopdf::Document::load_mem(source).unwrap();
    let page = pdf.get_pages()[&1];
    let original = pdf.get_page_content(page).unwrap();
    let mut data = b"/Span << /ActualText <FEFF".to_vec();
    for unit in text.encode_utf16() {
        data.extend_from_slice(format!("{unit:04X}").as_bytes());
    }
    data.extend_from_slice(b"> >> BDC\n");
    data.extend_from_slice(&original);
    data.extend_from_slice(b"\nEMC");
    let stream = pdf.add_object(lopdf::Stream::new(lopdf::dictionary! {}, data));
    pdf.get_object_mut(page)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Contents", stream);
    let mut result = Vec::new();
    pdf.save_to(&mut result).unwrap();
    result
}

#[test]
fn actual_text_retains_vertical_affine_stroked_and_clipped_cff_paint() {
    for source in [
        include_bytes!("fixtures/type0-vertical-cid-cff.pdf").as_slice(),
        include_bytes!("fixtures/type1-affine-cff.pdf").as_slice(),
        include_bytes!("fixtures/type1-stroked-affine-cff.pdf").as_slice(),
        include_bytes!("fixtures/type0-clipped-fd-cff.pdf").as_slice(),
    ] {
        let original = ofdrw::OfdDocument::from_pdf_bytes(source).unwrap();
        let imported =
            ofdrw::OfdDocument::from_pdf_bytes(&wrap_replacement(source, "中文abcd")).unwrap();
        assert_eq!(imported.page_text(0).unwrap().concat(), "中文abcd");
        assert_eq!(
            imported.page_png(0, 72.).unwrap(),
            original.page_png(0, 72.).unwrap()
        );
        assert!(imported.validate_references().unwrap().is_empty());
        let reopened = ofdrw::OfdDocument::from_bytes(&imported.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_text(0).unwrap().concat(), "中文abcd");
    }
}

#[test]
fn graphic_actual_text_retains_image_pixels_and_survives_native_pdf_export() {
    let source = include_bytes!("fixtures/image.pdf");
    let original = ofdrw::OfdDocument::from_pdf_bytes(source).unwrap();
    let imported =
        ofdrw::OfdDocument::from_pdf_bytes(&wrap_replacement(source, "caption")).unwrap();
    assert_eq!(imported.page_text(0).unwrap(), ["caption"]);
    assert_eq!(
        imported.page_png(0, 72.).unwrap(),
        original.page_png(0, 72.).unwrap()
    );
    assert!(imported.validate_references().unwrap().is_empty());
    let reopened = ofdrw::OfdDocument::from_pdf_bytes(&imported.to_pdf().unwrap()).unwrap();
    assert_eq!(reopened.page_text(0).unwrap().concat(), "caption");
}
