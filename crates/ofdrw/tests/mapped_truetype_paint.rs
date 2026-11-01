mod common;
use ofdrw::OfdDocument;

fn check(name: &str, source: &[u8], reference: &[u8]) {
    // Remove only the marked-content scope: the original glyph program and
    // independent PDF raster remain identical to the empty ActualText fixture.
    let mut pdf = lopdf::Document::load_mem(source).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    let mut content = lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();
    content
        .operations
        .retain(|op| op.operator != "BDC" && op.operator != "EMC");
    let stream = pdf.add_object(lopdf::Stream::new(
        lopdf::Dictionary::new(),
        content.encode().unwrap(),
    ));
    pdf.get_object_mut(page)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Contents", stream);
    let mut source = Vec::new();
    pdf.save_to(&mut source).unwrap();
    let document = OfdDocument::from_pdf_bytes(&source).unwrap();
    assert!(!document.page_text(0).unwrap().is_empty());
    assert!(!document.page_text_objects(0).unwrap().is_empty());
    let dpi = if name.ends_with("stroke") { 576. } else { 144. };
    let raster = image::load_from_memory(&document.page_png(0, dpi).unwrap())
        .unwrap()
        .to_rgb8();
    let reference = image::load_from_memory(reference).unwrap().to_rgb8();
    common::assert_raster_matches(&raster, &reference, name);
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(
        reopened.page_png(0, dpi).unwrap(),
        document.page_png(0, dpi).unwrap()
    );
    assert!(reopened.validate_references().unwrap().is_empty());
    let exported = OfdDocument::from_pdf_bytes(&reopened.to_pdf().unwrap()).unwrap();
    assert!(!exported.page_text(0).unwrap().is_empty());
    assert!(exported.validate_references().unwrap().is_empty());
    common::assert_raster_matches(
        &image::load_from_memory(&exported.page_png(0, dpi).unwrap())
            .unwrap()
            .to_rgb8(),
        &reference,
        name,
    );
}

macro_rules! case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_bytes!(concat!("fixtures/actual-text-empty-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/actual-text-empty-", $name, ".png")),
            );
        }
    };
}
case!(truetype, "truetype");
case!(affine, "affine");
case!(reflected, "reflected");
case!(truetype_stroke, "truetype-stroke");
case!(truetype_dashed_stroke, "truetype-dashed-stroke");
case!(cff_dashed_stroke, "cff-dashed-stroke");
