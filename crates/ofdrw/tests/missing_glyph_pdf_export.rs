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
    if let Some(gs) = std::env::var_os("OFDRW_GHOSTSCRIPT") {
        let temp = tempfile::tempdir().unwrap();
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
