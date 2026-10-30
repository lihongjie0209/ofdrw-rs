pub fn extract(bytes: &[u8]) -> String {
    let mut pdf = lopdf::Document::load_mem(bytes).unwrap();
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
    let content = lopdf::content::Content::decode(&pdf.get_page_content(page_id)).unwrap();
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
    pdf.extract_text(&[1]).unwrap()
}

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
