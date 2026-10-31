//! Keep imported custom appearances in PDF annotation Form XObjects.
use super::*;
use lopdf::{dictionary, Document, Object, ObjectId};

fn invalid(message: impl std::fmt::Display) -> OfdError {
    OfdError::Invalid(format!("PDF annotation appearance: {message}"))
}

fn flags(annotation: &AnnotationInfo) -> Result<i64, OfdError> {
    let mut result = pdf_annotation_flags_for_output(annotation)?.bits();
    if let Some(source) = annotation.parameters.get("PDFSourceFlags") {
        let source = source
            .parse::<u32>()
            .map_err(|_| invalid("invalid PDFSourceFlags"))?;
        result |= source & 1;
        if source & 32 != 0 && result & 2 != 0 {
            // NoView may still print. Do not turn it into the stronger Hidden
            // flag, while allowing an explicit Visible=true edit to clear it.
            result = if source & 2 == 0 {
                (result & !2) | 32
            } else {
                result | 32
            };
        }
    }
    Ok(i64::from(result))
}

fn svg(
    document: &OfdDocument,
    page: &PageInfo,
    annotation: &AnnotationInfo,
    options: RenderOptions,
) -> Result<String, OfdError> {
    let [x, y, width, height] = annotation
        .appearance_box
        .ok_or_else(|| invalid("missing appearance boundary"))?;
    if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
        return Err(invalid("invalid appearance dimensions"));
    }
    let fonts = document.fonts_for_page(&page.path, page.document_index)?;
    let composites =
        document.page_resource_elements(&page.path, page.document_index, "CompositeGraphicUnit")?;
    let params = document.page_resource_elements(&page.path, page.document_index, "DrawParam")?;
    let colors = document.color_resources_for_page(&page.path, page.document_index)?;
    let resources = SvgRenderResources {
        document_index: page.document_index,
        source_path: &page.path,
        fonts: &fonts,
        composite_units: &composites,
        draw_params: &params,
        colors: &colors,
        embedded_font_families: None,
        prefer_image_substitution: options.prefer_image_substitution,
        pdf_semantics: true,
    };
    let bytes = document.required_entry(&annotation.source_path)?;
    let parsed = parse_xml(&annotation.source_path, bytes)?;
    let node = parsed
        .root_element()
        .children()
        .find(|n| is_ofd_element(*n, "Annot") && n.attribute("ID") == Some(annotation.id.as_str()))
        .ok_or_else(|| invalid("annotation is missing"))?;
    let appearance = node
        .children()
        .find(|n| is_ofd_element(*n, "Appearance"))
        .ok_or_else(|| invalid("Appearance is missing"))?;
    let mut result=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{height}mm\" viewBox=\"0 0 {width} {height}\">");
    if let Some([ax, ay, aw, ah]) = page.application_box {
        if aw <= 0. || ah <= 0. || [ax, ay, aw, ah].iter().any(|v| !v.is_finite()) {
            return Err(invalid("invalid page application box"));
        }
        result.push_str(&format!("<defs><clipPath id=\"ofd-appearance-page\"><rect x=\"{}\" y=\"{}\" width=\"{aw}\" height=\"{ah}\"/></clipPath></defs><g clip-path=\"url(#ofd-appearance-page)\">",ax-x,ay-y));
    }
    let mut serial = 0;
    for object in appearance.descendants().filter(|n| {
        ["TextObject", "PathObject", "ImageObject", "CompositeObject"]
            .iter()
            .any(|kind| is_ofd_element(*n, kind))
    }) {
        if object
            .ancestors()
            .any(|n| is_ofd_element(n, "Clip") || is_ofd_element(n, "Pattern"))
        {
            continue;
        }
        document.render_svg_object(object, &resources, &mut serial, &mut result, 0, &[])?;
    }
    if page.application_box.is_some() {
        result.push_str("</g>");
    }
    result.push_str("</svg>");
    Ok(result)
}

fn form(
    document: &OfdDocument,
    page: &PageInfo,
    annotation: &AnnotationInfo,
    options: RenderOptions,
    pdf: &mut Document,
) -> Result<ObjectId, OfdError> {
    let source = svg(document, page, annotation, options)?;
    let mut svg_options = svg2pdf::usvg::Options {
        fontdb: pdf_system_fonts(),
        ..svg2pdf::usvg::Options::default()
    };
    for font in document.fonts_for_page_sources(page.index)? {
        if let Some(path) = font.embedded_path {
            svg_options
                .fontdb_mut()
                .load_font_data(document.required_entry(&path)?.to_vec());
        }
    }
    if PDF_FONTS.get().is_some_and(|fonts| fonts.is_empty()) {
        let database = svg_options.fontdb_mut();
        let families = database
            .faces()
            .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
            .collect::<Vec<_>>();
        if let Some(fallback) = preferred_font_family(&families) {
            database.set_serif_family(fallback.clone());
            database.set_sans_serif_family(fallback);
        }
    }
    let tree = svg2pdf::usvg::Tree::from_str(&source, &svg_options).map_err(invalid)?;
    let (chunk, root) =
        svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default()).map_err(invalid)?;
    let start = pdf
        .max_id
        .checked_add(1)
        .and_then(|v| i32::try_from(v).ok())
        .ok_or_else(|| invalid("object identifier overflow"))?;
    let mut allocator = pdf_writer::Ref::new(start);
    let mut references = HashMap::new();
    let chunk = chunk.renumber(|old| *references.entry(old).or_insert_with(|| allocator.bump()));
    let root = (references[&root].get() as u32, 0);
    let mut fragment = pdf_writer::Pdf::new();
    fragment.extend(&chunk);
    let fragment = Document::load_mem(&fragment.finish()).map_err(invalid)?;
    pdf.max_id = pdf.max_id.max(fragment.max_id);
    pdf.objects.extend(fragment.objects);
    Ok(root)
}

pub(super) fn write(
    document: &OfdDocument,
    bytes: Vec<u8>,
    options: RenderOptions,
    annotations: &[AnnotationInfo],
) -> Result<Vec<u8>, OfdError> {
    if !annotations
        .iter()
        .any(|a| a.parameters.contains_key("PDFSourceSubtype"))
    {
        return Ok(bytes);
    }
    let mut pdf = Document::load_mem(&bytes).map_err(invalid)?;
    let output_pages = pdf.get_pages().into_values().collect::<Vec<_>>();
    for (page, page_ref) in document.pages()?.iter().zip(output_pages) {
        let native = annotations
            .iter()
            .filter(|a| {
                a.document_index == page.document_index && same_page_id(&a.page_id, &page.id)
            })
            .collect::<Vec<_>>();
        if !native
            .iter()
            .any(|a| a.parameters.contains_key("PDFSourceSubtype"))
        {
            continue;
        }
        let annots = match pdf
            .get_dictionary(page_ref)
            .map_err(invalid)?
            .get(b"Annots")
        {
            Ok(value) => value.as_array().map_err(invalid)?.clone(),
            Err(_) => Vec::new(),
        };
        let mut names = annots
            .iter()
            .filter_map(|item| item.as_reference().ok())
            .filter_map(|id| pdf.get_dictionary(id).ok())
            .filter_map(|dict| dict.get(b"NM").ok())
            .filter_map(|value| lopdf::decode_text_string(value).ok())
            .collect::<HashSet<_>>();
        let mut ids = HashSet::new();
        let mut by_id = HashMap::<String, Vec<ObjectId>>::new();
        let mut untagged = Vec::new();
        for item in annots {
            let id = item.as_reference().map_err(invalid)?;
            let source_id = pdf
                .get_dictionary(id)
                .map_err(invalid)?
                .get(b"OfdAppearanceId")
                .ok()
                .map(|value| {
                    value.as_str().map_err(invalid).and_then(|value| {
                        std::str::from_utf8(value)
                            .map(str::to_owned)
                            .map_err(invalid)
                    })
                })
                .transpose()?;
            if let Some(source_id) = source_id {
                by_id.entry(source_id).or_default().push(id);
                pdf.get_object_mut(id)
                    .and_then(Object::as_dict_mut)
                    .map_err(invalid)?
                    .remove(b"OfdAppearanceId");
            } else {
                untagged.push(item);
            }
        }
        let mut ordered = Vec::new();
        for annotation in native {
            if !ids.insert(resource_id_key(&annotation.id)) {
                return Err(invalid("duplicate annotation ID"));
            }
            let generated = by_id.remove(&annotation.id).unwrap_or_default();
            let needs_form = annotation.parameters.contains_key("PDFSourceSubtype")
                || !["PDFTextIcon", "PDFMarkupQuads", "PDFShape"]
                    .iter()
                    .any(|key| annotation.parameters.contains_key(*key));
            if !needs_form {
                ordered.extend(generated.into_iter().map(Object::Reference));
                continue;
            }
            let output_flags = flags(annotation)?;
            let root = form(document, page, annotation, options, &mut pdf)?;
            let link_refs = generated
                .iter()
                .copied()
                .filter(|id| {
                    pdf.get_dictionary(*id)
                        .ok()
                        .and_then(|d| d.get(b"Subtype").ok())
                        .and_then(|v| v.as_name().ok())
                        != Some(b"FileAttachment".as_slice())
                })
                .collect::<Vec<_>>();
            ordered.extend(generated.into_iter().map(Object::Reference));
            if !link_refs.is_empty() {
                // Several Link hit regions can share one native appearance.
                // Paint it once; explicit empty appearances prevent viewers
                // from synthesizing extra artwork for the remaining regions.
                let empty = if link_refs.len() > 1 {
                    Some(pdf.add_object(lopdf::Stream::new(
                        dictionary! {
                            "Type" => "XObject", "Subtype" => "Form",
                            "BBox" => vec![0.into(), 0.into(), 1.into(), 1.into()],
                            "Resources" => dictionary! {},
                        },
                        Vec::new(),
                    )))
                } else {
                    None
                };
                for (index, id) in link_refs.into_iter().enumerate() {
                    let dict = pdf
                        .get_object_mut(id)
                        .and_then(Object::as_dict_mut)
                        .map_err(invalid)?;
                    let appearance = if index == 0 {
                        root
                    } else {
                        empty.unwrap_or(root)
                    };
                    dict.set("AP", dictionary! {"N"=>Object::Reference(appearance)});
                    dict.set("F", output_flags);
                }
                continue;
            }
            let boundary = annotation
                .appearance_box
                .ok_or_else(|| invalid("missing boundary"))?;
            let rect = pdf_link_rect(
                boundary,
                page.physical_box
                    .ok_or_else(|| invalid("missing page box"))?,
            )?;
            let mut dict = dictionary! {
                "Type"=>"Annot","Subtype"=>"Stamp","P"=>Object::Reference(page_ref),
                "Rect"=>Object::Array(rect.into_iter().map(|v|Object::Real(v as f32)).collect()),
                "AP"=>dictionary!{"N"=>Object::Reference(root)},
                "F"=>output_flags,
                "NM"=>pdf_text(&unique_pdf_annotation_name(annotation.parameters.get("PDFName").unwrap_or(&annotation.id),&mut names)),
            };
            if let Some(text) = &annotation.remark {
                dict.set("Contents", pdf_text(text));
            }
            if let Some(author) = annotation.attributes.get("Creator") {
                dict.set("T", pdf_text(author));
            }
            if let Some(day) = &annotation.last_mod_date {
                if pdf_date_from_ofd(day).is_none() {
                    return Err(invalid("invalid LastModDate"));
                }
                dict.set(
                    "M",
                    Object::string_literal(format!("D:{}000000Z", day.replace('-', ""))),
                );
            }
            let id = pdf.add_object(dict);
            ordered.push(Object::Reference(id));
            if let Some((rect, open)) = pdf_popup_output_data(
                annotation,
                page.physical_box
                    .ok_or_else(|| invalid("missing page box"))?,
            )? {
                let popup=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>Object::Reference(id),"P"=>Object::Reference(page_ref),
                    "Rect"=>Object::Array(rect.into_iter().map(|v|Object::Real(v as f32)).collect()),"Open"=>open});
                pdf.get_object_mut(id)
                    .and_then(Object::as_dict_mut)
                    .map_err(invalid)?
                    .set("Popup", Object::Reference(popup));
                untagged.push(Object::Reference(popup));
            }
        }
        if !by_id.is_empty() {
            return Err(invalid("unmatched annotation identifier"));
        }
        untagged.extend(ordered);
        pdf.get_object_mut(page_ref)
            .and_then(Object::as_dict_mut)
            .map_err(invalid)?
            .set("Annots", untagged);
    }
    let mut result = Vec::new();
    pdf.save_to(&mut result).map_err(invalid)?;
    Ok(result)
}

fn pdf_text(text: &str) -> Object {
    let data = [0xfe, 0xff]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_be_bytes))
        .collect();
    Object::String(data, lopdf::StringFormat::Hexadecimal)
}
