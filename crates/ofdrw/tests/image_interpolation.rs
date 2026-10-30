use ofdrw::OfdDocument;
use std::io::Cursor;

fn document() -> OfdDocument {
    let pixels = image::RgbaImage::from_fn(2, 1, |x, _| {
        if x == 0 {
            image::Rgba([255, 0, 0, 255])
        } else {
            image::Rgba([0, 0, 255, 128])
        }
    });
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    document_with_image(&png, "png")
}

fn document_with_image(data: &[u8], format: &str) -> OfdDocument {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 20.).unwrap();
    doc.add_image_object(0, data, format, [0., 0., 20., 20.])
        .unwrap();
    doc.add_image_object(0, data, format, [20., 0., 20., 20.])
        .unwrap();
    let page = doc.pages().unwrap()[0].path.clone();
    let xml = String::from_utf8(doc.entry(&page).unwrap().unwrap().to_vec()).unwrap();
    let mut index = 0;
    let xml = xml.replace(
        "<ofd:ImageObject",
        "<ofd:ImageObject Interpolate=\"SAMPLING\"",
    );
    let xml = xml
        .split("SAMPLING")
        .map(|part| {
            if index == 0 {
                index += 1;
                part.to_owned()
            } else {
                index += 1;
                format!("{}{part}", if index == 2 { "false" } else { "true" })
            }
        })
        .collect::<String>();
    let mut xml = xml;
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let images = parsed
        .descendants()
        .filter(|n| n.tag_name().name() == "ImageObject")
        .collect::<Vec<_>>();
    let shared = images[0].attribute("ResourceID").unwrap().to_owned();
    let range = images[1]
        .attributes()
        .find(|a| a.name() == "ResourceID")
        .unwrap()
        .range_value();
    xml.replace_range(range, &shared);
    doc.set_entry(&page, xml.into_bytes()).unwrap();
    doc
}

fn image_flags(bytes: &[u8]) -> Vec<bool> {
    let pdf = lopdf::Document::load_mem(bytes).unwrap();
    pdf.objects
        .values()
        .filter_map(|object| object.as_stream().ok())
        .filter(|s| s.dict.get(b"Subtype").and_then(lopdf::Object::as_name).ok() == Some(b"Image"))
        .map(|s| s.dict.get(b"Interpolate").unwrap().as_bool().unwrap())
        .collect()
}

#[test]
fn mixed_placements_keep_independent_color_and_alpha_sampling() {
    let doc = document();
    let svg = doc.page_svg(0).unwrap();
    assert!(svg.contains("image-rendering=\"optimizeSpeed\""));
    assert!(svg.contains("image-rendering=\"optimizeQuality\""));
    let flags = image_flags(&doc.to_pdf().unwrap());
    assert_eq!(flags.iter().filter(|v| **v).count(), 2);
    assert_eq!(flags.iter().filter(|v| !**v).count(), 2);
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(image_flags(&reopened.to_pdf().unwrap()), flags);
    let image = image::load_from_memory(&doc.page_png(0, 25.4).unwrap())
        .unwrap()
        .to_rgb8();
    assert_eq!(image.get_pixel(9, 10).0, [255, 0, 0]);
    assert_eq!(image.get_pixel(10, 10), image.get_pixel(19, 10));
    assert_ne!(image.get_pixel(9, 10), image.get_pixel(29, 10));
    assert!(doc.validate_references().unwrap().is_empty());
}

#[test]
fn malformed_native_sampling_fails_rendering() {
    let mut doc = document();
    let page = doc.pages().unwrap()[0].path.clone();
    let xml = String::from_utf8(doc.entry(&page).unwrap().unwrap().to_vec())
        .unwrap()
        .replace("Interpolate=\"false\"", "Interpolate=\"bad\"");
    doc.set_entry(&page, xml.into_bytes()).unwrap();
    assert!(doc.page_svg(0).is_err());
    assert!(doc.to_pdf().is_err());
}

#[test]
fn source_pdf_boolean_and_default_sampling_are_retained() {
    let source = include_bytes!("fixtures/image.pdf");
    for flag in [None, Some(false), Some(true)] {
        let mut pdf = lopdf::Document::load_mem(source).unwrap();
        for object in pdf.objects.values_mut() {
            if let Ok(stream) = object.as_stream_mut() {
                if stream
                    .dict
                    .get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .ok()
                    == Some(b"Image")
                {
                    match flag {
                        Some(v) => stream.dict.set("Interpolate", v),
                        None => {
                            stream.dict.remove(b"Interpolate");
                        }
                    }
                }
            }
        }
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        let imported = OfdDocument::from_pdf_bytes(&bytes).unwrap();
        let flags = image_flags(&imported.to_pdf().unwrap());
        assert!(!flags.is_empty());
        assert!(flags.iter().all(|v| *v == flag.unwrap_or(false)));
    }
}

#[test]
fn missing_native_flag_keeps_smooth_sampling_and_case_variants_are_supported() {
    for (from, to, expected) in [
        ("Interpolate=\"false\"", "Interpolate=\"FALSE\"", false),
        ("Interpolate=\"false\"", "", true),
    ] {
        let mut doc = document();
        let page = doc.pages().unwrap()[0].path.clone();
        let xml = String::from_utf8(doc.entry(&page).unwrap().unwrap().to_vec())
            .unwrap()
            .replace(from, to);
        doc.set_entry(&page, xml.into_bytes()).unwrap();
        let flags = image_flags(&doc.to_pdf().unwrap());
        assert_eq!(
            flags.iter().filter(|v| !**v).count(),
            if expected { 0 } else { 2 }
        );
    }
}

#[test]
fn source_pdf_indirect_boolean_and_invalid_types() {
    for value in [lopdf::Object::Boolean(true), lopdf::Object::Integer(1)] {
        let mut pdf = lopdf::Document::load_mem(include_bytes!("fixtures/image.pdf")).unwrap();
        let valid = value.as_bool().is_ok();
        let reference = pdf.add_object(value);
        for object in pdf.objects.values_mut() {
            if let Ok(stream) = object.as_stream_mut() {
                if stream
                    .dict
                    .get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .ok()
                    == Some(b"Image")
                {
                    stream.dict.set("Interpolate", reference);
                }
            }
        }
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        let imported = OfdDocument::from_pdf_bytes(&bytes);
        if valid {
            let flags = image_flags(&imported.unwrap().to_pdf().unwrap());
            assert!(!flags.is_empty());
            assert!(flags.iter().all(|v| *v));
        } else {
            assert!(
                matches!(imported, Err(ofdrw::OfdError::Invalid(message)) if message.contains("Interpolate"))
            );
        }
    }
}

#[test]
fn nearest_sampling_matches_ghostscript_pixels() {
    let Some(gs) = std::env::var_os("OFDRW_GHOSTSCRIPT") else {
        return;
    };
    let mut doc = document();
    let page = doc.pages().unwrap()[0].path.clone();
    let xml = String::from_utf8(doc.entry(&page).unwrap().unwrap().to_vec())
        .unwrap()
        .replace("Interpolate=\"true\"", "Interpolate=\"false\"");
    doc.set_entry(&page, xml.into_bytes()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("sampling.pdf");
    let png = dir.path().join("sampling.png");
    std::fs::write(&pdf, doc.to_pdf().unwrap()).unwrap();
    let result = std::process::Command::new(gs)
        .args([
            "-q",
            "-dSAFER",
            "-dBATCH",
            "-dNOPAUSE",
            "-sDEVICE=png16m",
            "-r25.4",
            "-dGraphicsAlphaBits=1",
        ])
        .arg(format!("-sOutputFile={}", png.display()))
        .arg(pdf)
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let native = image::load_from_memory(&doc.page_png(0, 25.4).unwrap())
        .unwrap()
        .to_rgb8();
    let reference = image::open(png).unwrap().to_rgb8();
    assert_eq!(native.dimensions(), reference.dimensions());
    assert!(native
        .as_raw()
        .iter()
        .zip(reference.as_raw())
        .all(|(a, b)| a.abs_diff(*b) <= 1));
}

#[test]
fn jpeg_sampling_keeps_original_compressed_payload() {
    let pixels = image::RgbImage::from_pixel(2, 2, image::Rgb([50, 100, 150]));
    let mut jpeg = Vec::new();
    image::DynamicImage::ImageRgb8(pixels)
        .write_to(&mut Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
        .unwrap();
    let bytes = document_with_image(&jpeg, "jpeg").to_pdf().unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let images = pdf
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .filter(|s| s.dict.get(b"Subtype").and_then(lopdf::Object::as_name).ok() == Some(b"Image"))
        .collect::<Vec<_>>();
    assert_eq!(images.len(), 2);
    for image in images {
        assert_eq!(
            image.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"DCTDecode"
        );
        assert_eq!(image.content, jpeg);
    }
    let flags = image_flags(&bytes);
    assert_eq!(flags.iter().filter(|v| **v).count(), 1);
    assert_eq!(flags.iter().filter(|v| !**v).count(), 1);
}

#[test]
fn image_sampling_api_edits_one_placement_and_restores_default() {
    let mut doc = document();
    let id = doc.page_objects(0).unwrap()[0].id.clone().unwrap();
    doc.set_page_image_interpolate(0, &id, Some(true)).unwrap();
    assert!(image_flags(&doc.to_pdf().unwrap())
        .iter()
        .all(|value| *value));
    doc.set_page_image_interpolate(0, &id, Some(false)).unwrap();
    assert_eq!(
        image_flags(&doc.to_pdf().unwrap())
            .iter()
            .filter(|v| !**v)
            .count(),
        2
    );
    doc.set_page_image_interpolate(0, &id, None).unwrap();
    assert!(image_flags(&doc.to_pdf().unwrap())
        .iter()
        .all(|value| *value));
    let bytes = doc.to_bytes().unwrap();
    for (page, object) in [(1, id.as_str()), (0, "missing"), (0, "")] {
        assert!(doc
            .set_page_image_interpolate(page, object, Some(false))
            .is_err());
        assert_eq!(doc.to_bytes().unwrap(), bytes);
    }
    let mut reopened = OfdDocument::from_bytes(&bytes).unwrap();
    reopened
        .set_page_image_interpolate(0, &id, Some(false))
        .unwrap();
    assert_eq!(
        image_flags(&reopened.to_pdf().unwrap())
            .iter()
            .filter(|v| !**v)
            .count(),
        2
    );
}

#[test]
fn image_sampling_api_rejects_other_object_types_and_signed_pages() {
    let mut doc = OfdDocument::new();
    doc.add_text_page("text", 40., 20.).unwrap();
    let id = doc.page_objects(0).unwrap()[0].id.clone().unwrap();
    let before = doc.to_bytes().unwrap();
    assert!(doc.set_page_image_interpolate(0, &id, Some(false)).is_err());
    assert_eq!(doc.to_bytes().unwrap(), before);
    let mut signed = OfdDocument::open(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-sign/src/test/resources/signedFile.ofd"),
    )
    .unwrap();
    let before = signed.to_bytes().unwrap();
    assert!(
        matches!(signed.set_page_image_interpolate(0, "62", Some(false)),
        Err(ofdrw::OfdError::Invalid(message)) if message.contains("signed"))
    );
    assert_eq!(signed.to_bytes().unwrap(), before);
}
