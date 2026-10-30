//! Preserve an isolated PDF transparency group as an atomic vector object.
use super::*;

pub(super) fn import(
    output: &mut OfdDocument,
    page: &PdfPageContext<'_>,
    parent: &GraphicsState,
    content: lopdf::content::Content<Vec<Operation>>,
    graphics: GraphicsState,
    active_forms: &mut Vec<ObjectId>,
) -> Result<(), OfdError> {
    let width = (page.box_[2] - page.box_[0]) * MM_PER_PT;
    let height = (page.box_[3] - page.box_[1]) * MM_PER_PT;
    let alpha = (parent.fill_alpha * 255.).round() as u8;
    let scratch = output.pages()?.len();
    output.add_blank_page(width, height)?;
    let path = output.pages()?[scratch].path.clone();
    let context = PdfPageContext {
        index: scratch,
        ..*page
    };
    import_content(output, &context, content, graphics, active_forms)?;
    let (children, invisible_text) = {
        let bytes = output.required_entry(&path)?;
        let parsed = parse_xml(&path, bytes)?;
        let source = parsed.input_text();
        let children = direct_page_objects(parsed.root_element())
            .map(|node| &source[node.range()])
            .collect::<String>();
        let invisible_text = if alpha == 0 {
            let mut text = Vec::new();
            for object in direct_page_objects(parsed.root_element()) {
                text.extend(semantic_text::from_composite(output, scratch, object)?);
            }
            Some(text)
        } else {
            None
        };
        (children, invisible_text)
    };
    // Keep nested font/image/pattern resources in the shared document scope.
    // Only the temporary placement page is discarded.
    output.remove_page(scratch)?;
    output.remove_entry(&path)?;
    // PDF text remains selectable when its group's compositing alpha is zero.
    // Retain that text as nonpainting objects rather than hiding it behind an
    // Alpha=0 OFD composite, which would also hide it from text extraction.
    if let Some(texts) = invisible_text {
        for text in texts {
            output.append_page_object(page.index, |_, id| text.with_id(id))?;
        }
        return Ok(());
    }
    if children.is_empty() {
        return Ok(());
    }
    let resource =
        output.add_composite_graphic_resource(page.index, [width, height], |_, _, _| children)?;
    let first_id = output.next_id;
    let id = output.add_composite_object(
        page.index,
        &resource,
        [0., 0., width, height],
        crate::CompositeStyle {
            alpha: Some(alpha),
            ..crate::CompositeStyle::default()
        },
    )?;
    attach_clips(output, page.index, &id, "CompositeObject", &parent.clips)?;
    if let Some(mask) = &parent.soft_mask {
        mask.apply(output, page, first_id, parent.blend_mode)
    } else {
        attach_blend_mode(output, page.index, &id, parent.blend_mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modified_source(change: impl FnOnce(&mut PdfDocument)) -> Vec<u8> {
        let mut pdf = PdfDocument::load_mem(include_bytes!(
            "../../tests/fixtures/isolated-group-child-alpha.pdf"
        ))
        .unwrap();
        change(&mut pdf);
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn nonisolated_transparency_still_requires_backdrop_support() {
        let bytes = modified_source(|pdf| {
            pdf.get_object_mut((5, 0))
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .get_mut(b"Group")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("I", false);
        });
        assert!(
            matches!(OfdDocument::from_pdf_bytes(&bytes), Err(OfdError::Invalid(message)) if message.contains("requires opaque content"))
        );
    }

    #[test]
    fn knockout_transparency_is_not_flattened_as_an_ordinary_group() {
        let bytes = modified_source(|pdf| {
            pdf.get_object_mut((5, 0))
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .get_mut(b"Group")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("K", true);
        });
        assert!(
            matches!(OfdDocument::from_pdf_bytes(&bytes), Err(OfdError::Invalid(message)) if message.contains("requires opaque content"))
        );
    }

    #[test]
    fn cyclic_isolated_forms_return_an_error() {
        let bytes = modified_source(|pdf| {
            let stream = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
            stream.content = b"/Fm Do".to_vec();
            stream.dict.set("Length", stream.content.len() as i64);
        });
        assert!(
            matches!(OfdDocument::from_pdf_bytes(&bytes), Err(OfdError::Invalid(message)) if message.contains("cyclic PDF Form"))
        );
    }

    #[test]
    fn resources_remain_valid_after_adding_another_page() {
        let bytes = include_bytes!("../../tests/fixtures/isolated-group-nested.pdf");
        let mut document = OfdDocument::from_pdf_bytes(bytes).unwrap();
        let before = document.page_png(0, 72.).unwrap();
        document.add_blank_page(40., 30.).unwrap();
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert!(reopened.validate_references().unwrap().is_empty());
        assert_eq!(reopened.page_png(0, 72.).unwrap(), before);
        assert_eq!(reopened.pages().unwrap().len(), 2);
    }
}
