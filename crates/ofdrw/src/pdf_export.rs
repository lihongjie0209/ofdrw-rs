//! Preserve the image sampling used by OFD's SVG renderer in exported PDFs.
use crate::OfdError;

pub(super) fn finish(bytes: Vec<u8>) -> Result<Vec<u8>, OfdError> {
    // svg2pdf 0.13 writes RGB/gray image streams (including alpha masks)
    // without Interpolate. PDF then defaults to nearest-neighbor sampling,
    // unlike the smooth image sampling used by our SVG and PNG renderer.
    // This is only a fast-path hint: the parsed object type decides what is
    // edited, so matching bytes inside a stream cannot modify other objects.
    if !bytes.windows(b"/Image".len()).any(|s| s == b"/Image") {
        return Ok(bytes);
    }
    let mut pdf = lopdf::Document::load_mem(&bytes)
        .map_err(|error| OfdError::Invalid(format!("cannot read generated PDF: {error}")))?;
    let mut changed = false;
    for object in pdf.objects.values_mut() {
        if let Ok(stream) = object.as_stream_mut() {
            if stream
                .dict
                .get(b"Subtype")
                .and_then(lopdf::Object::as_name)
                .ok()
                == Some(b"Image".as_slice())
            {
                stream.dict.set("Interpolate", true);
                changed = true;
            }
        }
    }
    if !changed {
        return Ok(bytes);
    }
    let mut result = Vec::new();
    pdf.save_to(&mut result).map_err(|error| {
        OfdError::Invalid(format!("cannot write interpolated PDF images: {error}"))
    })?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn vector_pdf_bytes_are_unchanged() {
        let mut document = crate::OfdDocument::new();
        document.add_blank_page(10., 10.).unwrap();
        let bytes = document.to_pdf().unwrap();
        assert_eq!(finish(bytes.clone()).unwrap(), bytes);
    }

    #[test]
    fn color_and_alpha_stream_samples_are_preserved() {
        let mut before = lopdf::Document::with_version("1.7");
        let mask = before.add_object(lopdf::Stream::new(
            lopdf::dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 1,
                "Height" => 1, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
            },
            vec![128],
        ));
        before.add_object(lopdf::Stream::new(
            lopdf::dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 1,
                "Height" => 1, "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
                "SMask" => mask,
            },
            vec![255, 30, 20],
        ));
        let pages = before.add_object(lopdf::dictionary! {
            "Type" => "Pages", "Count" => 0, "Kids" => Vec::<lopdf::Object>::new(),
        });
        let root = before.add_object(lopdf::dictionary! {"Type" => "Catalog", "Pages" => pages});
        before.trailer.set("Root", root);
        let mut bytes = Vec::new();
        before.save_to(&mut bytes).unwrap();
        let after = lopdf::Document::load_mem(&finish(bytes).unwrap()).unwrap();
        let mut images = 0;
        for (id, object) in &before.objects {
            if let Ok(stream) = object.as_stream() {
                if stream
                    .dict
                    .get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .ok()
                    == Some(b"Image".as_slice())
                {
                    assert!(stream.dict.get(b"Interpolate").is_err());
                    let actual = after.get_object(*id).unwrap().as_stream().unwrap();
                    assert_eq!(actual.content, stream.content);
                    assert!(actual.dict.get(b"Interpolate").unwrap().as_bool().unwrap());
                    images += 1;
                }
            }
        }
        assert_eq!(images, 2, "expected both RGB image and alpha mask");
    }
}
