//! Position semantic graphemes independently of painted RTL ligatures.
use super::*;
use unicode_segmentation::UnicodeSegmentation;

pub(super) struct Semantic(Vec<Segment>);
struct Segment {
    x: f64,
    y: f64,
    offset: f64,
    scale: f64,
    text: String,
}

impl Semantic {
    pub(super) fn svg(
        &self,
        style: &str,
        h_scale: f64,
        direction: i32,
    ) -> Result<String, OfdError> {
        let (cos, sin) = match direction {
            0 => (1., 0.),
            90 => (0., 1.),
            180 => (-1., 0.),
            270 => (0., -1.),
            _ => return Err(OfdError::Invalid("invalid RTL semantic direction".into())),
        };
        // A whitespace-only SVG text node has no bounds and is discarded by
        // usvg. Carry its explicit origin into the next nonempty text node.
        let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
        let mut pending = Vec::new();
        for (index, segment) in self.0.iter().enumerate() {
            pending.push(index);
            if !segment.text.chars().all(char::is_whitespace) {
                groups.push((index, std::mem::take(&mut pending)));
            }
        }
        if let Some((_, last)) = groups.last_mut() {
            last.extend(pending);
        }
        let mut svg = String::new();
        for (anchor, indices) in groups {
            let anchor = &self.0[anchor];
            let mut text = String::new();
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for index in indices {
                let segment = &self.0[index];
                let dx = segment.x + cos * h_scale * segment.offset - anchor.x;
                let dy = segment.y + sin * h_scale * segment.offset - anchor.y;
                let x = ((cos * dx + sin * dy) / h_scale - anchor.offset) / anchor.scale;
                let y = -sin * dx + cos * dy;
                if !x.is_finite() || !y.is_finite() {
                    return Err(OfdError::Invalid(
                        "RTL semantic spacing is not finite".into(),
                    ));
                }
                for _ in segment.text.chars() {
                    xs.push(x.to_string());
                    ys.push(y.to_string());
                }
                text.push_str(&segment.text);
            }
            svg.push_str(&format!(
                "<text data-ofd-rtl-semantic=\"true\" xml:space=\"preserve\" transform=\"translate({} {}) rotate({direction}) scale({h_scale} 1) translate({} 0) scale({} 1)\" x=\"{}\" y=\"{}\" {style}>{}</text>",
                anchor.x, anchor.y, anchor.offset, anchor.scale, xs.join(" "), ys.join(" "), escape_svg_text(&text),
            ));
            if svg.len() > 32_000_000 {
                return Err(OfdError::Limit("RTL semantic SVG is too large".into()));
            }
        }
        Ok(svg)
    }
}

pub(super) fn has_shared_mark_clusters(svg: &str) -> Result<bool, OfdError> {
    let parsed = parse_xml("PDF semantic SVG", svg.as_bytes())?;
    let mut corrected = false;
    let mut marked = false;
    for node in parsed
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "text")
    {
        corrected |= node.attribute("data-ofd-rtl-semantic") == Some("true");
        marked |= node
            .descendants()
            .filter_map(|n| n.text().filter(|_| n.is_text()))
            .any(|text| {
                text.chars()
                    .any(|c| unicode_bidi::bidi_class(c) == unicode_bidi::BidiClass::NSM)
            });
    }
    // svg2pdf shares one glyph-to-Unicode map for the page's font. A mark
    // cluster in a later run can overwrite an isolated letter's mapping.
    // Reuse the original whole-run layer for the entire page in that case.
    Ok(corrected && marked)
}

pub(super) fn for_code(
    face: &ttf_parser::Face<'_>,
    code: Node<'_, '_>,
    glyphs: &[ttf_parser::GlyphId],
    size: f64,
) -> Result<Option<Semantic>, OfdError> {
    let text = decode_text_code(element_text(code).as_ref())?;
    if !text.chars().any(|c| {
        matches!(
            unicode_bidi::bidi_class(c),
            unicode_bidi::BidiClass::R | unicode_bidi::BidiClass::AL
        )
    }) {
        return Ok(None);
    }
    // Combining-mark glyphs may be shared by distinct source clusters. The
    // converter currently assigns one Unicode string per glyph ID, so retain
    // the original semantic run until it supports context-specific mappings.
    if text
        .chars()
        .any(|c| unicode_bidi::bidi_class(c) == unicode_bidi::BidiClass::NSM)
    {
        return Ok(None);
    }
    let Some(font) = rustybuzz::Face::from_slice(face.raw_face().data, 0) else {
        return Ok(None);
    };
    let scale = size / f64::from(face.units_per_em());
    let mut shaped = Vec::new();
    for (run, direction, script) in visual_shape_runs(&text)? {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(&text[run.clone()]);
        buffer.guess_segment_properties();
        buffer.set_direction(direction);
        buffer.set_script(script);
        let result = rustybuzz::shape(&font, &[], buffer);
        let mut starts = result
            .glyph_infos()
            .iter()
            .map(|g| g.cluster as usize)
            .collect::<Vec<_>>();
        starts.push(run.len());
        starts.sort_unstable();
        starts.dedup();
        for (info, position) in result.glyph_infos().iter().zip(result.glyph_positions()) {
            let start = info.cluster as usize;
            let Some(&end) = starts.get(starts.partition_point(|value| *value <= start)) else {
                return Ok(None);
            };
            shaped.push((
                info.glyph_id,
                run.start + start..run.start + end,
                *position,
                direction,
            ));
        }
    }
    // CGTransform may deliberately map text to unrelated outlines. Only infer
    // caret positions when shaping reproduces the source glyph sequence.
    if shaped.len() != glyphs.len()
        || shaped
            .iter()
            .zip(glyphs)
            .any(|(g, expected)| g.0 != u32::from(expected.0))
    {
        return Ok(None);
    }
    let dx = expand_text_deltas(code.attribute("DeltaX"), glyphs.len().saturating_sub(1))?;
    let dy = expand_text_deltas(code.attribute("DeltaY"), glyphs.len().saturating_sub(1))?;
    let mut x = text_code_coordinate(code, "X", 0.)?;
    let mut y = text_code_coordinate(code, "Y", size)?;
    let mut positions = Vec::new();
    for index in 0..glyphs.len() {
        if index > 0 {
            x += dx.as_ref().map_or(0., |values| values[index - 1]);
            y += dy.as_ref().map_or(0., |values| values[index - 1]);
        }
        positions.push((x, y));
    }
    let mut segments = Vec::new();
    let mut widths = HashMap::new();
    let mut index = 0;
    while index < shaped.len() {
        let (_, cluster, position, direction) = &shaped[index];
        let mut end = index + 1;
        while end < shaped.len() && shaped[end].1 == *cluster {
            end += 1;
        }
        let advance = shaped[index..end]
            .iter()
            .map(|g| f64::from(g.2.x_advance))
            .sum::<f64>()
            * scale;
        let origin_x = positions[index].0 - f64::from(position.x_offset) * scale;
        let origin_y = positions[index].1 + f64::from(position.y_offset) * scale;
        let mut graphemes = text[cluster.clone()].graphemes(true).collect::<Vec<_>>();
        if *direction == rustybuzz::Direction::RightToLeft {
            graphemes.reverse();
        }
        let cell = advance / graphemes.len().max(1) as f64;
        for (number, grapheme) in graphemes.into_iter().enumerate() {
            let width = *widths.entry(grapheme).or_insert_with(|| {
                let mut buffer = rustybuzz::UnicodeBuffer::new();
                buffer.push_str(grapheme);
                buffer.guess_segment_properties();
                let alone = rustybuzz::shape(&font, &[], buffer);
                alone
                    .glyph_positions()
                    .iter()
                    .map(|p| f64::from(p.x_advance))
                    .sum::<f64>()
                    * scale
            });
            let ratio = if width > 0. && cell > 0. {
                cell / width
            } else {
                1.
            };
            let offset = number as f64 * cell;
            if [origin_x, origin_y, ratio, offset]
                .iter()
                .any(|v| !v.is_finite())
            {
                return Err(OfdError::Invalid(
                    "RTL semantic position is not finite".into(),
                ));
            }
            segments.push(Segment {
                x: origin_x,
                y: origin_y,
                offset,
                scale: ratio,
                text: grapheme.into(),
            });
            if segments.len() > MAX_POSITIONED_GLYPHS {
                return Err(OfdError::Limit(
                    "RTL semantic text exceeds positioned glyph limit".into(),
                ));
            }
        }
        index = end;
    }
    Ok(Some(Semantic(segments)))
}

#[cfg(test)]
mod tests {
    use super::*;
    const FONT: &[u8] = include_bytes!("../tests/fixtures/dejavu-sans.ttf");

    fn source(text: &str) -> String {
        let mut doc = OfdDocument::new();
        let font = doc.add_embedded_font("DejaVu Sans", FONT).unwrap();
        doc.add_blank_page(120., 30.).unwrap();
        doc.add_shaped_text_object(0, text, [5., 5., 110., 20.], 6., &font.to_string())
            .unwrap();
        String::from_utf8(
            doc.required_entry(&doc.pages().unwrap()[0].path)
                .unwrap()
                .to_vec(),
        )
        .unwrap()
    }
    fn semantic(source: &str, mismatch: bool) -> Result<Option<Semantic>, OfdError> {
        let parsed = parse_xml("page", source.as_bytes()).unwrap();
        let object = parsed
            .descendants()
            .find(|n| is_ofd_element(*n, "TextObject"))
            .unwrap();
        let code = object
            .children()
            .find(|n| is_ofd_element(*n, "TextCode"))
            .unwrap();
        let mut glyphs = parse_cg_transforms(object).unwrap()[0]
            .glyphs
            .iter()
            .map(|id| ttf_parser::GlyphId(*id as u16))
            .collect::<Vec<_>>();
        if mismatch {
            glyphs[0] = ttf_parser::GlyphId(0);
        }
        for_code(
            &ttf_parser::Face::parse(FONT, 0).unwrap(),
            code,
            &glyphs,
            6.,
        )
    }
    #[test]
    fn split_ligatures_keep_individual_unicode_and_finite_positions() {
        let semantic = semantic(&source("سلام"), false).unwrap().unwrap();
        assert_eq!(
            semantic
                .0
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>(),
            ["م", "ا", "ل", "س"]
        );
        assert!(semantic
            .0
            .iter()
            .all(|s| [s.x, s.y, s.offset, s.scale].iter().all(|v| v.is_finite()) && s.scale > 0.));
        assert!(semantic
            .svg("font-size=\"6\"", 1.3, 90)
            .unwrap()
            .contains("rotate(90) scale(1.3 1)"));
    }
    #[test]
    fn unmatched_glyph_aliases_and_complex_mark_clusters_keep_the_original_path() {
        assert!(semantic(&source("سلام"), true).unwrap().is_none());
        assert!(semantic(&source("سَلَام"), false).unwrap().is_none());
        assert!(semantic(&source("office"), false).unwrap().is_none());
    }
    #[test]
    fn whitespace_has_a_nonempty_carrier_instead_of_disappearing() {
        let semantic = semantic(&source("سلام لا سلام"), false).unwrap().unwrap();
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\">{}</svg>",
            semantic.svg("font-size=\"6\"", 1., 0).unwrap()
        );
        let parsed = parse_xml("semantic SVG", svg.as_bytes()).unwrap();
        let values = parsed
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "text")
            .map(|n| n.text().unwrap())
            .collect::<Vec<_>>();
        assert!(values.iter().all(|s| !s.chars().all(char::is_whitespace)));
        assert_eq!(values.concat(), "مالس ال مالس");
    }
    #[test]
    fn invalid_semantic_spacing_is_an_error() {
        let semantic = semantic(&source("سلام"), false).unwrap().unwrap();
        assert!(semantic.svg("", 0., 0).is_err());
        assert!(semantic.svg("", 1., 45).is_err());
    }
    #[test]
    fn marked_runs_cannot_overwrite_corrected_letters_in_shared_font_maps() {
        for values in [["سلام", "سَ"], ["سَ", "سلام"]] {
            let mut doc = OfdDocument::new();
            let font = doc.add_embedded_font("DejaVu Sans", FONT).unwrap();
            doc.add_blank_page(120., 60.).unwrap();
            for (index, text) in values.iter().enumerate() {
                doc.add_shaped_text_object(
                    0,
                    text,
                    [5., 5. + index as f64 * 20., 110., 20.],
                    6.,
                    &font.to_string(),
                )
                .unwrap();
            }
            let svg = doc
                .page_svg_for_output(0, RenderOptions::default(), true, 0, false)
                .unwrap();
            let legacy = doc
                .page_svg_for_output_inner(0, RenderOptions::default(), true, 0, false, false)
                .unwrap();
            assert_eq!(svg, legacy);
            assert!(!svg.contains("data-ofd-rtl-semantic"));
        }
    }
    #[test]
    fn clip_text_does_not_replace_the_painted_objects_semantic_run() {
        let mut doc = OfdDocument::new();
        let font = doc.add_embedded_font("DejaVu Sans", FONT).unwrap();
        doc.add_blank_page(120., 30.).unwrap();
        doc.add_shaped_text_object(0, "سلام", [5., 5., 110., 20.], 6., &font.to_string())
            .unwrap();
        let path = doc.pages().unwrap()[0].path.clone();
        let original = std::str::from_utf8(doc.required_entry(&path).unwrap()).unwrap();
        let clip = format!("<ofd:Clips><ofd:Clip><ofd:Area><ofd:Text Font=\"{font}\" Size=\"6\" Boundary=\"0 0 100 20\"><ofd:TextCode X=\"0\" Y=\"6\">Q</ofd:TextCode></ofd:Text></ofd:Area></ofd:Clip></ofd:Clips>");
        let xml = original.replacen("<ofd:TextCode", &format!("{clip}<ofd:TextCode"), 1);
        doc.set_entry(&path, xml.into_bytes()).unwrap();
        let svg = doc
            .page_svg_for_output(0, RenderOptions::default(), true, 0, false)
            .unwrap();
        let parsed = parse_xml("semantic SVG", svg.as_bytes()).unwrap();
        let semantic = parsed
            .descendants()
            .filter(|n| n.is_element() && n.attribute("data-ofd-rtl-semantic") == Some("true"))
            .filter_map(|n| n.text())
            .collect::<String>();
        assert_eq!(semantic, "مالس");
        assert_eq!(doc.page_text(0).unwrap(), ["سلام"]);
        assert!(doc.validate_references().unwrap().is_empty());
    }
}
