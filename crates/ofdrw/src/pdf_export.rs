//! Normalize generated PDF image sampling and Unicode font mappings.
use crate::OfdError;

pub(super) fn finish(bytes: Vec<u8>) -> Result<Vec<u8>, OfdError> {
    // svg2pdf 0.13 writes RGB/gray image streams (including alpha masks)
    // without Interpolate. PDF then defaults to nearest-neighbor sampling,
    // unlike the smooth image sampling used by our SVG and PNG renderer.
    // This is only a fast-path hint: the parsed object type decides what is
    // edited, so matching bytes inside a stream cannot modify other objects.
    if ![b"/Image".as_slice(), b"/ToUnicode".as_slice()]
        .iter()
        .any(|hint| bytes.windows(hint.len()).any(|s| s == *hint))
    {
        return Ok(bytes);
    }
    let mut pdf = lopdf::Document::load_mem(&bytes)
        .map_err(|error| OfdError::Invalid(format!("cannot read generated PDF: {error}")))?;
    let mut changed = normalize_unicode_maps(&mut pdf)?;
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
    pdf.save_to(&mut result)
        .map_err(|error| OfdError::Invalid(format!("cannot write normalized PDF: {error}")))?;
    Ok(result)
}

fn normalize_unicode_maps(pdf: &mut lopdf::Document) -> Result<bool, OfdError> {
    let maps = pdf
        .objects
        .values()
        .filter_map(|object| object.as_dict().ok())
        .filter(|dict| {
            dict.get(b"Type").and_then(lopdf::Object::as_name).ok() == Some(b"Font".as_slice())
        })
        .filter_map(|dict| {
            dict.get(b"ToUnicode")
                .and_then(lopdf::Object::as_reference)
                .ok()
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut changed = false;
    for id in maps {
        let stream = pdf
            .get_object_mut(id)
            .and_then(lopdf::Object::as_stream_mut)
            .map_err(|e| {
                OfdError::Invalid(format!("cannot read generated ToUnicode stream: {e}"))
            })?;
        let data = stream.get_plain_content().map_err(|e| {
            OfdError::Invalid(format!("cannot decode generated ToUnicode stream: {e}"))
        })?;
        let source = std::str::from_utf8(&data)
            .map_err(|e| OfdError::Invalid(format!("invalid generated ToUnicode CMap: {e}")))?;
        // pdf-writer 0.12 emits a general CMap header with type 0. ToUnicode
        // requires type 2. Its optional CMapVersion/WMode entries also prevent
        // some readers from decoding otherwise valid mappings. The font's
        // Encoding, widths and all bfchar/bfrange records remain unchanged.
        let mut normalized = String::new();
        let mut edited = false;
        for line in source.split_inclusive('\n') {
            match line.trim_end_matches('\n') {
                "/CMapType 0 def" => {
                    normalized.push_str("/CMapType 2 def\n");
                    edited = true;
                }
                "/CMapVersion 1 def" | "/WMode 0 def" => edited = true,
                _ => normalized.push_str(line),
            }
        }
        if edited {
            stream.set_plain_content(normalized.into_bytes());
            stream.compress().map_err(|e| {
                OfdError::Invalid(format!("cannot compress generated ToUnicode stream: {e}"))
            })?;
            changed = true;
        }
    }
    Ok(changed)
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

    #[test]
    fn unicode_normalization_only_edits_referenced_font_maps() {
        let data = b"/CMapType 0 def\n/CMapVersion 1 def\n/WMode 0 def\n1 beginbfchar\n<0001> <D884DF50>\nendbfchar\n".to_vec();
        let mut pdf = lopdf::Document::with_version("1.7");
        let map = pdf.add_object(lopdf::Stream::new(lopdf::Dictionary::new(), data.clone()));
        pdf.add_object(
            lopdf::dictionary! {"Type" => "Font", "Subtype" => "Type0", "ToUnicode" => map},
        );
        let decoy = pdf.add_object(lopdf::Stream::new(lopdf::Dictionary::new(), data.clone()));
        assert!(normalize_unicode_maps(&mut pdf).unwrap());
        let actual = pdf
            .get_object(map)
            .unwrap()
            .as_stream()
            .unwrap()
            .get_plain_content()
            .unwrap();
        assert_eq!(
            actual,
            b"/CMapType 2 def\n1 beginbfchar\n<0001> <D884DF50>\nendbfchar\n"
        );
        assert_eq!(
            pdf.get_object(decoy).unwrap().as_stream().unwrap().content,
            data
        );
        assert!(!normalize_unicode_maps(&mut pdf).unwrap());
    }

    #[test]
    fn text_only_pdf_normalization_is_idempotent() {
        let mut document = crate::OfdDocument::new();
        document.add_text_page("Unicode export", 50., 20.).unwrap();
        let bytes = document.to_pdf().unwrap();
        assert_eq!(finish(bytes.clone()).unwrap(), bytes);
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let fonts = pdf
            .objects
            .values()
            .filter_map(|o| o.as_dict().ok())
            .filter_map(|d| {
                d.get(b"ToUnicode")
                    .and_then(lopdf::Object::as_reference)
                    .ok()
            })
            .collect::<Vec<_>>();
        assert!(!fonts.is_empty());
        for id in fonts {
            let stream = pdf.get_object(id).unwrap().as_stream().unwrap();
            let content = String::from_utf8(stream.get_plain_content().unwrap()).unwrap();
            assert!(content.contains("/CMapType 2 def"));
            assert!(!content.contains("/CMapVersion"));
            assert!(!content.contains("/WMode"));
        }
    }
}
