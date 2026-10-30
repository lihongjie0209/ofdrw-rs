//! Verify semantic text survives the SVG/PDF path, including nonpainting text.
use ofdrw::OfdDocument;

fn check(name: &str, source: &[u8], expected: &str) {
    let document = OfdDocument::from_pdf_bytes(source).unwrap();
    assert_eq!(
        document.page_text(0).unwrap().concat(),
        expected,
        "{name} OFD"
    );
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    let bytes = reopened.to_pdf().unwrap();
    let mut pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let page_id = pdf.get_pages()[&1];
    let resources = pdf
        .get_object(page_id)
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .clone();
    let content = lopdf::content::Content::decode(&pdf.get_page_content(page_id).unwrap()).unwrap();
    let mut operations = Vec::new();
    let mut fonts = lopdf::Dictionary::new();
    flatten_forms(
        &pdf,
        &resources,
        content.operations,
        &mut operations,
        &mut fonts,
        0,
    );
    // lopdf's text extractor reads page streams but does not follow Form Do
    // operators. Flatten only the generated content's Form/font scopes so its
    // existing ToUnicode decoder can independently verify all text operators.
    let stream = pdf.add_object(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        lopdf::content::Content { operations }.encode().unwrap(),
    ));
    let page = pdf.get_object_mut(page_id).unwrap().as_dict_mut().unwrap();
    page.set("Contents", stream);
    page.set("Resources", lopdf::dictionary! { "Font" => fonts });
    let extracted = pdf.extract_text(&[1]).unwrap();
    // lopdf inserts replacement characters for this fixture's unmapped
    // variation-cluster glyphs; Poppler omits them. Keep both reader results
    // explicit rather than silently filtering arbitrary replacement chars.
    let native_expected = if name == "type0-variation-glyph" {
        "\u{fffd}0\u{fe00}\u{fffd}0\u{fe00}"
    } else {
        expected
    };
    assert_eq!(
        extracted.split_whitespace().collect::<String>(),
        native_expected,
        "{name} PDF: {extracted:?}"
    );
    if let Some(program) = std::env::var_os("OFDRW_PDFTOTEXT") {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("export.pdf");
        std::fs::write(&path, &bytes).unwrap();
        let result = std::process::Command::new(program)
            .args(["-enc", "UTF-8"])
            .arg(&path)
            .arg("-")
            .output()
            .unwrap();
        assert!(result.status.success(), "{name}: {result:?}");
        let extracted = String::from_utf8(result.stdout).unwrap();
        assert_eq!(
            extracted.split_whitespace().collect::<String>(),
            expected,
            "{name} Poppler: {extracted:?}"
        );
    }
}
macro_rules! export_case {
    ($test:ident, $fixture:literal, $text:literal) => {
        #[test]
        fn $test() {
            check(
                $fixture,
                include_bytes!(concat!("fixtures/", $fixture, ".pdf")),
                $text,
            );
        }
    };
}
export_case!(masked_text, "soft-mask-text", "A");
export_case!(masked_fill_stroke, "soft-mask-text-fill-stroke-alpha", "A");
export_case!(masked_group, "soft-mask-group-text", "A");
export_case!(masked_nested_group, "soft-mask-nested-group-text", "AB");
export_case!(masked_reused_form, "soft-mask-group-text-reuse", "AA");
export_case!(masked_inner_mask, "soft-mask-group-text-inner-mask", "A");
export_case!(masked_affine_group, "soft-mask-group-text-transform", "A");
export_case!(truetype_ligatures, "truetype-ligature-glyph", "fiAfi");
export_case!(cid_ligatures, "type0-ligature-glyph", "fiAfi");
export_case!(
    glyph_name_sequences,
    "truetype-glyph-name-sequence",
    "fiAfi"
);
export_case!(
    variation_selectors,
    "type0-variation-glyph",
    "0\u{fe00}0\u{fe00}"
);
export_case!(
    supplementary_unicode,
    "type0-supplementary-collection-unicode",
    "\u{31350}"
);
export_case!(
    identity_unicode,
    "type0-identity-collection-unicode",
    "中文"
);
export_case!(
    embedded_collection_unicode,
    "type0-embedded-collection-unicode",
    "中文"
);
export_case!(outline_unicode, "outline-tounicode", "中文");
export_case!(metadata_unicode, "metadata-tounicode", "中文");
export_case!(rotated_unicode, "rotated-tounicode", "中文");

// Generated PDFs use explicit per-glyph Tf operators and self-contained Form
// resources. Preserve invocation order, including repeated Form references.
fn flatten_forms(
    pdf: &lopdf::Document,
    resources: &lopdf::Dictionary,
    source: Vec<lopdf::content::Operation>,
    output: &mut Vec<lopdf::content::Operation>,
    fonts: &mut lopdf::Dictionary,
    depth: usize,
) {
    assert!(depth < 32, "generated PDF Form cycle or excessive nesting");
    let mut local_fonts = std::collections::HashMap::new();
    for mut operation in source {
        if operation.operator == "Tf" {
            let name = operation.operands[0].as_name().unwrap().to_vec();
            let replacement = local_fonts.entry(name.clone()).or_insert_with(|| {
                let name_out = format!("TextExportFont{}", fonts.len());
                let local = resources.get(b"Font").unwrap();
                let local = pdf.dereference(local).unwrap().1.as_dict().unwrap();
                fonts.set(name_out.clone(), local.get(&name).unwrap().clone());
                name_out.into_bytes()
            });
            operation.operands[0] = lopdf::Object::Name(replacement.clone());
            output.push(operation);
        } else if operation.operator == "Do" {
            let name = operation.operands[0].as_name().unwrap();
            let objects = pdf
                .dereference(resources.get(b"XObject").unwrap())
                .unwrap()
                .1
                .as_dict()
                .unwrap();
            let object = pdf
                .dereference(objects.get(name).unwrap())
                .unwrap()
                .1
                .as_stream()
                .unwrap();
            if object.dict.get(b"Subtype").unwrap().as_name().unwrap() == b"Form" {
                let child_resources = object
                    .dict
                    .get(b"Resources")
                    .ok()
                    .map(|r| pdf.dereference(r).unwrap().1.as_dict().unwrap())
                    .unwrap_or(resources);
                let data = object
                    .decompressed_content()
                    .unwrap_or_else(|_| object.content.clone());
                let content = lopdf::content::Content::decode(&data).unwrap();
                flatten_forms(
                    pdf,
                    child_resources,
                    content.operations,
                    output,
                    fonts,
                    depth + 1,
                );
            }
        } else {
            output.push(operation);
        }
    }
}
