mod common;
use ofdrw::OfdDocument;

#[test]
fn image_intents_match_independent_rasters_and_round_trip() {
    for (name, source, reference) in [
        (
            "inline-trailing-cr",
            include_bytes!("fixtures/image-intent-inline-trailing-cr.pdf").as_slice(),
            include_bytes!("fixtures/image-intent-inline-trailing-cr.png").as_slice(),
        ),
        (
            "inline-embedded-ei",
            include_bytes!("fixtures/image-intent-inline-embedded-ei.pdf").as_slice(),
            include_bytes!("fixtures/image-intent-inline-embedded-ei.png").as_slice(),
        ),
        (
            "inline-flate",
            include_bytes!("fixtures/image-intent-inline-flate.pdf").as_slice(),
            include_bytes!("fixtures/image-intent-inline-flate.png").as_slice(),
        ),
        (
            "form-inline",
            include_bytes!("fixtures/image-intent-form-inline.pdf").as_slice(),
            include_bytes!("fixtures/image-intent-form-inline.png").as_slice(),
        ),
        (
            "xobject",
            include_bytes!("fixtures/image-intent-xobject.pdf").as_slice(),
            include_bytes!("fixtures/image-intent-xobject.png").as_slice(),
        ),
        (
            "form",
            include_bytes!("fixtures/image-intent-form.pdf").as_slice(),
            include_bytes!("fixtures/image-intent-form.png").as_slice(),
        ),
        (
            "inline",
            include_bytes!("fixtures/image-intent-inline.pdf").as_slice(),
            include_bytes!("fixtures/image-intent-inline.png").as_slice(),
        ),
        (
            "masked",
            include_bytes!("fixtures/image-intent-masked.pdf").as_slice(),
            include_bytes!("fixtures/image-intent-masked.png").as_slice(),
        ),
    ] {
        let doc = OfdDocument::from_pdf_bytes(source).unwrap();
        assert_eq!(doc.page_images(0).unwrap().len(), 5, "{name}");
        let png = doc.page_png(0, 72.).unwrap();
        let actual = image::load_from_memory(&png).unwrap().to_rgb8();
        let expected = image::load_from_memory(reference).unwrap().to_rgb8();
        common::assert_raster_matches(&actual, &expected, name);
        let centers = [12, 30, 48, 66, 84].map(|x| actual.get_pixel(x, 24).0);
        assert_eq!(centers[0], centers[2], "q/Q: {name}");
        assert_ne!(centers[0], centers[1], "ri: {name}");
        assert_ne!(centers[0], centers[3], "ExtGState: {name}");
        if name != "masked" {
            assert_eq!(centers[1], centers[4], "image override: {name}");
        }
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_png(0, 72.).unwrap(), png, "{name}");
        assert!(reopened.validate_references().unwrap().is_empty());
        assert!(lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok());
    }
}
