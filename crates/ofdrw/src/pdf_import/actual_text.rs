//! Keep replacement text separate from the glyphs that paint a marked sequence.
use super::*;

mod coalesce;

type NativeRun<'a, 'input> = (
    roxmltree::Node<'a, 'input>,
    roxmltree::Node<'a, 'input>,
    String,
);

pub(super) struct Replacement {
    text: String,
    first_id: usize,
}

impl Replacement {
    pub(super) fn read(
        pdf: &PdfDocument,
        properties: &Object,
        first_id: usize,
    ) -> Result<Option<Self>, OfdError> {
        let properties = pdf_resolved_value(Some(pdf), properties, "marked-content properties")?
            .as_dict()
            .map_err(|_| invalid("properties are not a dictionary"))?;
        let Ok(value) = properties.get(b"ActualText") else {
            return Ok(None);
        };
        let value = pdf_resolved_value(Some(pdf), value, "ActualText")?;
        let raw = value
            .as_str()
            .map_err(|_| invalid("is not a text string"))?;
        if raw.len() > 1_000_000 {
            return Err(OfdError::Limit("PDF ActualText is too long".into()));
        }
        let text = if let Some(payload) = raw.strip_prefix(b"\xfe\xff") {
            if !payload.len().is_multiple_of(2) {
                return Err(invalid("has truncated UTF-16"));
            }
            let units = payload
                .chunks_exact(2)
                .map(|v| u16::from_be_bytes([v[0], v[1]]))
                .collect::<Vec<_>>();
            String::from_utf16(&units).map_err(|_| invalid("has invalid UTF-16"))?
        } else if let Some(payload) = raw.strip_prefix(b"\xef\xbb\xbf") {
            std::str::from_utf8(payload)
                .map_err(|_| invalid("has invalid UTF-8"))?
                .to_owned()
        } else {
            let text =
                lopdf::decode_text_string(value).map_err(|_| invalid("cannot be decoded"))?;
            if text.chars().count() != raw.len() {
                return Err(invalid("contains undefined PDFDocEncoding characters"));
            }
            text
        };
        if text.encode_utf16().count() > 200_000 {
            return Err(OfdError::Limit(
                "PDF ActualText exceeds 200000 code units".into(),
            ));
        }
        Ok(Some(Self { text, first_id }))
    }

    pub(super) fn apply(self, output: &mut OfdDocument, page_index: usize) -> Result<(), OfdError> {
        let pages = output.pages()?;
        let page = pages
            .get(page_index)
            .ok_or(OfdError::PageIndex {
                index: page_index,
                count: pages.len(),
            })?
            .clone();
        let original = output.required_entry(&page.path)?.to_vec();
        let parsed = parse_xml(&page.path, &original)?;
        let objects = direct_page_objects(parsed.root_element())
            .filter(|object| {
                object
                    .attribute("ID")
                    .and_then(|id| id.parse::<usize>().ok())
                    .is_some_and(|id| id >= self.first_id)
            })
            .collect::<Vec<_>>();
        let texts = objects
            .iter()
            .copied()
            .filter(|node| is_ofd_element(*node, "TextObject"))
            .collect::<Vec<_>>();
        if texts.is_empty() {
            if self.text.is_empty() || objects.is_empty() {
                return Ok(());
            }
            if objects
                .iter()
                .any(|node| is_ofd_element(*node, "CompositeObject"))
            {
                return Err(invalid("across a composite object is not yet supported"));
            }
            let boundary = objects
                .iter()
                .find_map(|node| node.attribute("Boundary").and_then(parse_box))
                .ok_or_else(|| invalid("painted content has no valid boundary"))?;
            let size = boundary[3].clamp(0.1, 12.);
            let id = output.add_text_object(page_index, &self.text, boundary, size, None)?;
            output.set_page_text_paint_flags(page_index, &id, Some(false), Some(false))?;
            return Ok(());
        }
        if objects
            .iter()
            .any(|node| is_ofd_element(*node, "CompositeObject"))
        {
            return Err(invalid("across a composite object is not yet supported"));
        }
        let source = parsed.input_text();
        if self.text.is_empty() {
            // These objects contribute semantics only. Removing their native
            // text does not change graphics or the already advanced PDF cursor.
            if texts.iter().any(|object| {
                object.attribute("Fill") != Some("false")
                    || object
                        .attribute("Stroke")
                        .is_some_and(|value| value != "false")
            }) {
                return Err(invalid(
                    "empty replacement of painted text is not yet supported",
                ));
            }
            let mut result = source.to_owned();
            for object in texts.iter().rev() {
                result.replace_range(object.range(), "");
            }
            return output.set_entry(&page.path, result.into_bytes());
        }
        let mut runs = Vec::new();
        for object in texts {
            let codes = object
                .children()
                .filter(|n| is_ofd_element(*n, "TextCode"))
                .collect::<Vec<_>>();
            let [code] = codes.as_slice() else {
                return Err(invalid("requires single native text runs"));
            };
            let current = crate::decode_text_code(crate::element_text(*code).as_ref())?;
            runs.push((object, *code, current));
        }
        if runs
            .iter()
            .map(|(_, _, text)| text.as_str())
            .collect::<String>()
            == self.text
        {
            return Ok(());
        }
        use unicode_segmentation::UnicodeSegmentation;
        let boundaries = self
            .text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .chain(std::iter::once(self.text.len()))
            .collect::<Vec<_>>();
        let count = boundaries.len() - 1;
        if count < runs.len() {
            let merged = coalesce::merge(output, &page, &runs, &self.text)?;
            let mut result = source.to_owned();
            for (object, _, _) in runs.iter().skip(1).rev() {
                result.replace_range(object.range(), "");
            }
            result.replace_range(runs[0].0.range(), &merged);
            return output.set_entry(&page.path, result.into_bytes());
        }
        // ActualText supplies no individual caret positions. Share its intact
        // graphemes across the original runs while retaining all painted glyph
        // positions and fonts. Extraction concatenates the replacement once.
        let run_count = runs.len();
        let mut edits = Vec::new();
        for (index, (object, code, current)) in runs.into_iter().enumerate() {
            let replacement_text = &self.text[boundaries[index * count / run_count]
                ..boundaries[(index + 1) * count / run_count]];
            if current == replacement_text {
                continue;
            }
            let glyphs = mapped_glyphs(object, &current)?;
            let transforms = object
                .children()
                .filter(|n| is_ofd_element(*n, "CGTransform"))
                .collect::<Vec<_>>();
            let transform = transforms
                .first()
                .ok_or_else(|| invalid("has no native glyph mapping"))?;
            let code_xml = &source[code.range()];
            let start = code_xml
                .find('>')
                .ok_or_else(|| invalid("native TextCode is malformed"))?
                + 1;
            let end = code_xml
                .rfind("</")
                .ok_or_else(|| invalid("native TextCode is malformed"))?;
            let tag = qualified_tag(source, *transform)?;
            let glyph_node = transform
                .children()
                .find(|n| is_ofd_element(*n, "Glyphs"))
                .ok_or_else(|| invalid("has no native glyph list"))?;
            let glyph_tag = qualified_tag(source, glyph_node)?;
            let ids = glyphs
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            let count = replacement_text.encode_utf16().count();
            let replacement = format!("<{tag} CodePosition=\"0\" CodeCount=\"{count}\" GlyphCount=\"{}\"><{glyph_tag}>{ids}</{glyph_tag}></{tag}>", glyphs.len());
            for redundant in transforms.iter().skip(1) {
                edits.push((redundant.range(), String::new()));
            }
            edits.push((
                code.range().start + start..code.range().start + end,
                crate::escape_text_code(replacement_text),
            ));
            edits.push((transform.range(), replacement));
        }
        edits.sort_by_key(|(range, _)| range.start);
        let mut result = source.to_owned();
        for (range, value) in edits.into_iter().rev() {
            result.replace_range(range, &value);
        }
        output.set_entry(&page.path, result.into_bytes())
    }
}

fn mapped_glyphs(object: roxmltree::Node<'_, '_>, text: &str) -> Result<Vec<u32>, OfdError> {
    let mut mappings = crate::parse_cg_transforms(object)?;
    mappings.sort_by_key(|mapping| mapping.code_position);
    let mut covered = 0;
    let mut glyphs = Vec::new();
    for mapping in mappings {
        if mapping.code_position != covered {
            return Err(invalid("requires complete source glyph mappings"));
        }
        covered += mapping.code_count;
        glyphs.extend(mapping.glyphs);
    }
    if glyphs.is_empty() || covered != text.encode_utf16().count() {
        return Err(invalid(
            "replacement requires complete source glyph mappings",
        ));
    }
    Ok(glyphs)
}

fn invalid(message: &str) -> OfdError {
    OfdError::Invalid(format!("PDF ActualText {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, StringFormat};

    fn read(raw: &[u8]) -> Result<Replacement, OfdError> {
        Replacement::read(
            &PdfDocument::with_version("2.0"),
            &Object::Dictionary(dictionary! {
                "ActualText" => Object::String(raw.to_vec(), StringFormat::Hexadecimal),
            }),
            0,
        )
        .map(|value| value.unwrap())
    }

    fn shaped(texts: &[&str]) -> (OfdDocument, Vec<String>) {
        let mut doc = OfdDocument::new();
        doc.add_blank_page(80., 30.).unwrap();
        let font = doc
            .add_embedded_font(
                "DejaVu Sans",
                include_bytes!("../../tests/fixtures/dejavu-sans.ttf"),
            )
            .unwrap()
            .to_string();
        let ids = texts
            .iter()
            .enumerate()
            .map(|(index, text)| {
                doc.add_shaped_text_object(
                    0,
                    text,
                    [5. + index as f64 * 20., 5., 20., 15.],
                    6.,
                    &font,
                )
                .unwrap()
            })
            .collect();
        (doc, ids)
    }

    #[test]
    fn decodes_pdfdoc_utf16_utf8_without_stripping_content_boms() {
        assert_eq!(read(b"word\xa0").unwrap().text, "word€");
        assert_eq!(
            read(b"\xfe\xff\x4e\x2d\xd8\x84\xdf\x50").unwrap().text,
            "中\u{31350}"
        );
        assert_eq!(
            read("\u{feff}中\u{feff}".as_bytes()).unwrap().text,
            "中\u{feff}"
        );
        assert_eq!(read(b"\xfe\xff\xfe\xff\x00A").unwrap().text, "\u{feff}A");
        assert_eq!(read(b"\xfe\xff\xff\xfd").unwrap().text, "\u{fffd}");
    }

    #[test]
    fn rejects_invalid_encodings_and_limits() {
        for bytes in [
            b"\xfe\xff\x00".as_slice(),
            b"\xfe\xff\xd8\x00",
            b"\xef\xbb\xbf\xff",
            b"\x9f",
        ] {
            assert!(read(bytes).is_err(), "{bytes:?}");
        }
        assert!(read(&vec![b'A'; 1_000_001]).is_err());
        assert!(read(&vec![b'A'; 200_001]).is_err());
    }

    #[test]
    fn resolves_indirect_strings_and_ignores_missing_property() {
        let mut pdf = PdfDocument::with_version("2.0");
        let value = pdf.add_object(Object::string_literal("replacement"));
        let properties = pdf.add_object(dictionary! { "ActualText" => value });
        assert_eq!(
            Replacement::read(&pdf, &Object::Reference(properties), 42)
                .unwrap()
                .unwrap()
                .text,
            "replacement"
        );
        assert!(
            Replacement::read(&pdf, &Object::Dictionary(dictionary! {"MCID" => 1}), 0)
                .unwrap()
                .is_none()
        );
        assert!(Replacement::read(
            &pdf,
            &Object::Dictionary(dictionary! {"ActualText" => 1}),
            0
        )
        .is_err());
    }

    #[test]
    fn replacement_retains_glyphs_positions_paint_and_supplementary_boundaries() {
        let (mut doc, ids) = shaped(&["AB"]);
        let before = doc.page_png(0, 72.).unwrap();
        let original = doc.page_text_objects(0).unwrap();
        let replacement = "中\u{31350}文\0";
        Replacement {
            text: replacement.into(),
            first_id: ids[0].parse().unwrap(),
        }
        .apply(&mut doc, 0)
        .unwrap();
        assert_eq!(doc.page_text(0).unwrap(), [replacement]);
        assert_eq!(doc.page_png(0, 72.).unwrap(), before);
        let after = doc.page_text_objects(0).unwrap();
        assert_eq!(
            after[0].cg_transforms[0].glyphs,
            original[0]
                .cg_transforms
                .iter()
                .flat_map(|mapping| mapping.glyphs.iter().copied())
                .collect::<Vec<_>>()
        );
        assert_eq!(after[0].cg_transforms[0].code_count, 5);
        assert!(doc.validate_references().unwrap().is_empty());
    }

    #[test]
    fn multiple_runs_share_intact_graphemes_and_keep_original_pixels() {
        let (mut doc, ids) = shaped(&["A", "B"]);
        let before = doc.page_png(0, 72.).unwrap();
        Replacement {
            text: "a\u{301}中\u{31350}".into(),
            first_id: ids[0].parse().unwrap(),
        }
        .apply(&mut doc, 0)
        .unwrap();
        assert_eq!(doc.page_text(0).unwrap().concat(), "a\u{301}中\u{31350}");
        assert_eq!(doc.page_text(0).unwrap()[0], "a\u{301}");
        assert_eq!(doc.page_png(0, 72.).unwrap(), before);
        assert!(doc.validate_references().unwrap().is_empty());
    }

    #[test]
    fn short_replacement_preserves_all_original_glyphs_and_pixels() {
        let (mut doc, ids) = shaped(&["AB", "C", "DE"]);
        let original = doc.page_text_objects(0).unwrap();
        let glyphs = original
            .iter()
            .flat_map(|object| {
                object
                    .cg_transforms
                    .iter()
                    .flat_map(|mapping| mapping.glyphs.iter().copied())
            })
            .collect::<Vec<_>>();
        let before = [72., 144.].map(|dpi| doc.page_png(0, dpi).unwrap());
        Replacement {
            text: "X".into(),
            first_id: ids[0].parse().unwrap(),
        }
        .apply(&mut doc, 0)
        .unwrap();
        let objects = doc.page_text_objects(0).unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].cg_transforms[0].code_count, 1);
        assert_eq!(objects[0].cg_transforms[0].glyphs, glyphs);
        assert_eq!(doc.page_text(0).unwrap(), ["X"]);
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_text(0).unwrap(), ["X"]);
        for (index, dpi) in [72., 144.].into_iter().enumerate() {
            assert_eq!(reopened.page_png(0, dpi).unwrap(), before[index]);
        }
        assert!(reopened.validate_references().unwrap().is_empty());
    }

    #[test]
    fn short_replacement_keeps_combining_and_supplementary_graphemes_intact() {
        for text in ["a\u{301}", "\u{31350}"] {
            let (mut doc, ids) = shaped(&["A", "B", "C"]);
            let before = doc.page_png(0, 144.).unwrap();
            Replacement {
                text: text.into(),
                first_id: ids[0].parse().unwrap(),
            }
            .apply(&mut doc, 0)
            .unwrap();
            assert_eq!(doc.page_text(0).unwrap(), [text]);
            assert_eq!(
                doc.page_text_objects(0).unwrap()[0].cg_transforms[0].code_count,
                2
            );
            assert_eq!(doc.page_png(0, 144.).unwrap(), before);
            assert!(doc.validate_references().unwrap().is_empty());
        }
    }

    #[test]
    fn short_replacement_preserves_cff_glyph_outlines() {
        let mut doc =
            OfdDocument::from_pdf_bytes(include_bytes!("../../tests/fixtures/actual-text-cff.pdf"))
                .unwrap();
        let objects = doc.page_text_objects(0).unwrap();
        assert_eq!(objects.len(), 2);
        let before = image::load_from_memory(&doc.page_png(0, 144.).unwrap())
            .unwrap()
            .to_rgba8();
        Replacement {
            text: "X".into(),
            first_id: 0,
        }
        .apply(&mut doc, 0)
        .unwrap();
        assert_eq!(doc.page_text(0).unwrap(), ["X"]);
        let after = image::load_from_memory(&doc.page_png(0, 144.).unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(after.dimensions(), before.dimensions());
        // Sharing a Boundary can change mask antialiasing at former text-box
        // edges. Interior pixels stay exact; integration tests compare the
        // merged paint with an independently rasterized original PDF.
        for (x, y, pixel) in before.enumerate_pixels() {
            if *pixel != *after.get_pixel(x, y) {
                assert!(
                    objects.iter().filter_map(|object| object.boundary).any(
                        |[left, top, width, height]| {
                            let [left, top, right, bottom] =
                                [left, top, left + width, top + height]
                                    .map(|value| value * 144. / 25.4);
                            let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                            px >= left - 1.
                                && px <= right + 1.
                                && py >= top - 1.
                                && py <= bottom + 1.
                                && [
                                    (px - left).abs(),
                                    (px - right).abs(),
                                    (py - top).abs(),
                                    (py - bottom).abs(),
                                ]
                                .into_iter()
                                .any(|distance| distance <= 1.)
                        }
                    ),
                    "changed CFF interior pixel ({x}, {y})"
                );
            }
        }
        assert!(doc.validate_references().unwrap().is_empty());
    }

    #[test]
    fn short_replacement_preserves_local_namespace_bindings() {
        let (mut doc, ids) = shaped(&["A", "B"]);
        let path = doc.pages().unwrap()[0].path.clone();
        let mut source = String::from_utf8(doc.required_entry(&path).unwrap().to_vec()).unwrap();
        for (name, prefix) in [("TextCode", "t"), ("CGTransform", "c"), ("Glyphs", "g")] {
            source = source
                .replace(
                    &format!("<ofd:{name}"),
                    &format!("<{prefix}:{name} xmlns:{prefix}=\"{}\"", crate::OFD_NS),
                )
                .replace(&format!("</ofd:{name}>"), &format!("</{prefix}:{name}>"));
        }
        doc.set_entry(&path, source.into_bytes()).unwrap();
        let before = doc.page_png(0, 144.).unwrap();
        Replacement {
            text: "X".into(),
            first_id: ids[0].parse().unwrap(),
        }
        .apply(&mut doc, 0)
        .unwrap();
        assert_eq!(doc.page_text(0).unwrap(), ["X"]);
        assert_eq!(doc.page_png(0, 144.).unwrap(), before);
        assert!(doc.validate_references().unwrap().is_empty());
    }

    #[test]
    fn short_replacement_preserves_affine_and_reflected_glyph_positions() {
        for linear in [[0.8, 0.3, -0.2, 1.1], [-1.0, 0.2, 0.1, 1.0]] {
            let (mut doc, ids) = shaped(&["A", "B"]);
            for (index, id) in ids.iter().enumerate() {
                doc.set_page_object_ctm(
                    0,
                    id,
                    Some([
                        linear[0],
                        linear[1],
                        linear[2],
                        linear[3],
                        8. - index as f64,
                        2.,
                    ]),
                )
                .unwrap();
            }
            let before = doc.page_png(0, 144.).unwrap();
            Replacement {
                text: "X".into(),
                first_id: ids[0].parse().unwrap(),
            }
            .apply(&mut doc, 0)
            .unwrap();
            assert_eq!(doc.page_text(0).unwrap(), ["X"]);
            assert_eq!(doc.page_png(0, 144.).unwrap(), before);
            assert!(doc.validate_references().unwrap().is_empty());
        }
    }

    fn rejects_short_replacement_without_mutation(
        mut doc: OfdDocument,
        first_id: &str,
        message: &str,
    ) {
        let before = doc.to_bytes().unwrap();
        assert!(
            matches!(Replacement { text: "X".into(), first_id: first_id.parse().unwrap() }.apply(&mut doc, 0), Err(OfdError::Invalid(value)) if value.contains(message))
        );
        assert_eq!(doc.to_bytes().unwrap(), before);
    }

    #[test]
    fn short_replacement_rejects_different_styles_atomically() {
        let (mut doc, ids) = shaped(&["A", "B"]);
        doc.set_page_text_paint_flags(0, &ids[1], Some(false), None)
            .unwrap();
        rejects_short_replacement_without_mutation(doc, &ids[0], "matching text styles");
    }

    #[test]
    fn short_replacement_retains_intervening_graphic_paint_order() {
        let (mut doc, ids) = shaped(&["A", "B"]);
        let path = doc
            .add_path_object(0, "M 0 0 L 10 10", [1., 2., 10., 10.], PathStyle::default())
            .unwrap();
        doc.move_page_object(0, &path, Some(&ids[1])).unwrap();
        rejects_short_replacement_without_mutation(doc, &ids[0], "consecutive");
    }

    #[test]
    fn short_replacement_rejects_boundary_clipping_changes_atomically() {
        let (mut doc, ids) = shaped(&["A", "B"]);
        doc.set_page_object_ctm(0, &ids[0], Some([1., 0., 0., 1., -2., 0.]))
            .unwrap();
        rejects_short_replacement_without_mutation(doc, &ids[0], "boundary clipping");
    }

    #[test]
    fn short_replacement_rejects_group_opacity_changes_atomically() {
        let (mut doc, ids) = shaped(&["A", "B"]);
        for id in &ids {
            doc.set_page_object_alpha(0, id, 128).unwrap();
        }
        rejects_short_replacement_without_mutation(doc, &ids[0], "opaque");
    }

    #[test]
    fn short_replacement_rejects_singular_text_matrices_atomically() {
        let (mut doc, ids) = shaped(&["A", "B"]);
        for id in &ids {
            doc.set_page_object_ctm(0, id, Some([1., 2., 2., 4., 0., 0.]))
                .unwrap();
        }
        rejects_short_replacement_without_mutation(doc, &ids[0], "singular");
    }

    #[test]
    fn short_replacement_bounds_the_combined_glyph_count() {
        let text = "A".repeat(5_001);
        let (mut doc, ids) = shaped(&[&text, &text]);
        for id in &ids {
            doc.set_page_object_boundary(0, id, [5., 5., 50_000., 15.])
                .unwrap();
        }
        let before = doc.to_bytes().unwrap();
        assert!(
            matches!(Replacement { text: "X".into(), first_id: ids[0].parse().unwrap() }.apply(&mut doc, 0), Err(OfdError::Limit(value)) if value.contains("10000 merged glyphs"))
        );
        assert_eq!(doc.to_bytes().unwrap(), before);
    }

    #[test]
    fn scope_does_not_rewrite_earlier_objects_and_failed_replacements_are_atomic() {
        let (mut doc, ids) = shaped(&["A", "B"]);
        Replacement {
            text: "中".into(),
            first_id: ids[1].parse().unwrap(),
        }
        .apply(&mut doc, 0)
        .unwrap();
        assert_eq!(doc.page_text(0).unwrap(), ["A", "中"]);
        let before = doc.to_bytes().unwrap();
        assert!(Replacement {
            text: String::new(),
            first_id: ids[0].parse().unwrap()
        }
        .apply(&mut doc, 0)
        .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }

    #[test]
    fn empty_replacement_removes_only_nonpainting_text_and_retains_graphics() {
        let (mut doc, ids) = shaped(&["A", "B", "C"]);
        for id in &ids[1..] {
            doc.set_page_text_paint_flags(0, id, Some(false), Some(false))
                .unwrap();
        }
        let path = doc
            .add_path_object(0, "M 0 0 L 10 10", [1., 2., 10., 10.], PathStyle::default())
            .unwrap();
        doc.move_page_object(0, &path, Some(&ids[2])).unwrap();
        let reference = doc.page_png(0, 144.).unwrap();
        Replacement {
            text: String::new(),
            first_id: ids[1].parse().unwrap(),
        }
        .apply(&mut doc, 0)
        .unwrap();
        assert_eq!(doc.page_text(0).unwrap(), ["A"]);
        assert_eq!(doc.page_png(0, 144.).unwrap(), reference);
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_text(0).unwrap(), ["A"]);
        assert!(reopened.validate_references().unwrap().is_empty());
    }

    #[test]
    fn empty_replacement_rejects_mixed_visible_and_hidden_text_atomically() {
        for (fill, stroke) in [(true, false), (false, true)] {
            let (mut doc, ids) = shaped(&["A", "B"]);
            doc.set_page_text_paint_flags(0, &ids[0], Some(false), Some(false))
                .unwrap();
            doc.set_page_text_paint_flags(0, &ids[1], Some(fill), Some(stroke))
                .unwrap();
            let before = doc.to_bytes().unwrap();
            assert!(Replacement {
                text: String::new(),
                first_id: ids[0].parse().unwrap(),
            }
            .apply(&mut doc, 0)
            .is_err());
            assert_eq!(doc.to_bytes().unwrap(), before);
        }
    }

    #[test]
    fn empty_replacement_does_not_require_glyph_programs_for_hidden_text() {
        let mut doc = OfdDocument::new();
        doc.add_blank_page(30., 20.).unwrap();
        let id = doc
            .add_text_object(0, "hidden", [0., 0., 20., 10.], 5., None)
            .unwrap();
        doc.set_page_text_paint_flags(0, &id, Some(false), Some(false))
            .unwrap();
        Replacement {
            text: String::new(),
            first_id: id.parse().unwrap(),
        }
        .apply(&mut doc, 0)
        .unwrap();
        assert!(doc.page_text(0).unwrap().is_empty());
        assert!(doc.validate_references().unwrap().is_empty());
    }

    #[test]
    fn unchanged_unmapped_text_remains_valid_and_changed_text_requires_glyphs() {
        let mut doc = OfdDocument::new();
        doc.add_text_page("A", 30., 20.).unwrap();
        let before = doc.to_bytes().unwrap();
        Replacement {
            text: "A".into(),
            first_id: 0,
        }
        .apply(&mut doc, 0)
        .unwrap();
        assert_eq!(doc.to_bytes().unwrap(), before);
        assert!(Replacement {
            text: "B".into(),
            first_id: 0
        }
        .apply(&mut doc, 0)
        .is_err());
        assert_eq!(doc.to_bytes().unwrap(), before);
    }

    #[test]
    fn graphic_only_replacement_adds_nonpainting_text_and_empty_scopes_add_nothing() {
        let mut doc = OfdDocument::new();
        doc.add_blank_page(30., 20.).unwrap();
        Replacement {
            text: "ignored".into(),
            first_id: 0,
        }
        .apply(&mut doc, 0)
        .unwrap();
        assert!(doc.page_text(0).unwrap().is_empty());
        doc.add_path_object(0, "M 0 0 L 10 10", [1., 2., 10., 10.], PathStyle::default())
            .unwrap();
        let before = doc.page_png(0, 72.).unwrap();
        Replacement {
            text: "description".into(),
            first_id: 0,
        }
        .apply(&mut doc, 0)
        .unwrap();
        assert_eq!(doc.page_text(0).unwrap(), ["description"]);
        assert_eq!(doc.page_png(0, 72.).unwrap(), before);
        assert!(doc.validate_references().unwrap().is_empty());
    }
}
