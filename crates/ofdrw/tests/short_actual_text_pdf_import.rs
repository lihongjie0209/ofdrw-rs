mod common;
use ofdrw::OfdDocument;

fn check(name: &str, source: &[u8], reference: &[u8]) {
    let document = OfdDocument::from_pdf_bytes(source).unwrap();
    assert_eq!(document.page_text(0).unwrap(), ["X"], "{name}");
    let objects = document.page_text_objects(0).unwrap();
    assert_eq!(objects.len(), 1, "{name}");
    assert_eq!(objects[0].cg_transforms[0].code_count, 1, "{name}");
    assert_eq!(objects[0].cg_transforms[0].glyph_count, 3, "{name}");
    assert!(document.page_images(0).unwrap().is_empty(), "{name}");
    assert!(document.page_paths(0).unwrap().is_empty(), "{name}");
    let png = document.page_png(0, 144.).unwrap();
    common::assert_raster_matches(
        &image::load_from_memory(&png).unwrap().to_rgb8(),
        &image::load_from_memory(reference).unwrap().to_rgb8(),
        name,
    );
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_png(0, 144.).unwrap(), png, "{name}");
    assert!(reopened.validate_references().unwrap().is_empty(), "{name}");
    let exported = OfdDocument::from_pdf_bytes(&reopened.to_pdf().unwrap()).unwrap();
    assert_eq!(exported.page_text(0).unwrap().concat(), "X", "{name}");
    assert!(exported.validate_references().unwrap().is_empty(), "{name}");
}

macro_rules! case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_bytes!(concat!("fixtures/actual-text-short-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/actual-text-short-", $name, ".png")),
            );
        }
    };
}
case!(truetype, "truetype");
case!(affine, "affine");
case!(reflected, "reflected");
case!(cff, "cff");
case!(cff_matrix, "cff-matrix");
case!(cid_cff_matrix, "cid-cff-matrix");

#[test]
fn empty_actual_text_discards_hidden_semantics_and_keeps_visible_paths() {
    use lopdf::{content::Operation, Object};
    for source in [
        include_bytes!("fixtures/actual-text-short-truetype.pdf").as_slice(),
        include_bytes!("fixtures/actual-text-short-cff.pdf").as_slice(),
    ] {
        let mut pdf = lopdf::Document::load_mem(source).unwrap();
        let page = pdf.get_pages()[&1];
        let mut content = lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();
        for operation in &mut content.operations {
            if operation.operator == "BDC" {
                operation.operands[1]
                    .as_dict_mut()
                    .unwrap()
                    .set("ActualText", Object::string_literal(""));
            }
        }
        content
            .operations
            .insert(0, Operation::new("Tr", vec![3.into()]));
        content.operations.extend([
            Operation::new("rg", vec![1.into(), 0.into(), 0.into()]),
            Operation::new("re", vec![0.into(), 0.into(), 10.into(), 10.into()]),
            Operation::new("f", vec![]),
        ]);
        let stream = pdf.get_page_contents(page)[0];
        pdf.get_object_mut(stream)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_plain_content(content.encode().unwrap());
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        let document = OfdDocument::from_pdf_bytes(&bytes).unwrap();
        assert!(document.page_text(0).unwrap().is_empty());
        assert!(document.page_text_objects(0).unwrap().is_empty());
        assert_eq!(document.page_paths(0).unwrap().len(), 1);
        let png = document.page_png(0, 144.).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgb8();
        assert!(image.pixels().any(|pixel| pixel.0 == [255, 0, 0]));
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert!(reopened.page_text(0).unwrap().is_empty());
        assert_eq!(reopened.page_png(0, 144.).unwrap(), png);
        let exported = OfdDocument::from_pdf_bytes(&reopened.to_pdf().unwrap()).unwrap();
        assert!(exported.page_text(0).unwrap().is_empty());
        assert!(exported.validate_references().unwrap().is_empty());
    }
}
