//! Recover separate native paths without merging independently painted glyphs.
use roxmltree::Node;

use super::{invalid, mapped_glyphs};
use crate::{decode_text_code, element_text, is_ofd_element, OfdDocument, OfdError, PageInfo};

pub(super) struct Budget {
    glyphs: usize,
    points: usize,
    bytes: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            glyphs: 10_000,
            points: 500_000,
            bytes: 32_000_000,
        }
    }
}

/// Recover glyph geometry with an optional outer user-space placement.
fn glyph_paths(
    document: &OfdDocument,
    page: &PageInfo,
    object: Node<'_, '_>,
    outer_matrix: [f64; 6],
    budget: &mut Budget,
) -> Result<Vec<String>, OfdError> {
    let codes = object
        .children()
        .filter(|n| is_ofd_element(*n, "TextCode"))
        .collect::<Vec<_>>();
    let [code] = codes.as_slice() else {
        return Err(invalid(
            "outline conversion requires a single native text run",
        ));
    };
    let text = decode_text_code(element_text(*code).as_ref())?;
    let glyphs = mapped_glyphs(object, &text)?;
    if glyphs.len() > budget.glyphs {
        return Err(OfdError::Limit(
            "PDF ActualText exceeds 10000 outlined glyphs".into(),
        ));
    }
    budget.glyphs -= glyphs.len();
    let size = crate::svg_number(object.attribute("Size"), 0.)?;
    let h_scale = crate::svg_number(object.attribute("HScale"), 1.)?;
    if size <= 0. || h_scale <= 0. {
        return Err(invalid("outline conversion has invalid text size or scale"));
    }
    let fonts = document.fonts_for_page(&page.path, page.document_index)?;
    let path = object
        .attribute("Font")
        .and_then(|id| {
            fonts
                .iter()
                .find(|font| crate::resource_id_matches(Some(&font.id), id))
        })
        .and_then(|font| font.embedded_path.as_deref())
        .ok_or_else(|| invalid("outline conversion requires an embedded font"))?;
    let bytes = document.required_entry(path)?;
    let wrapped;
    let face = match ttf_parser::Face::parse(bytes, 0) {
        Ok(face) => face,
        Err(_) if ttf_parser::cff::Table::parse(bytes).is_some() => {
            wrapped = crate::pdf_import::cff_font::wrap(bytes)?;
            ttf_parser::Face::parse(&wrapped, 0)
                .map_err(|_| invalid("outline conversion requires a usable font"))?
        }
        Err(_) => return Err(invalid("outline conversion requires a usable font")),
    };
    let matrices = face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"CFF "))
        .filter(|_| face.tables().glyf.is_none())
        .map(crate::pdf_import::cff_font::GlyphMatrices::parse)
        .transpose()?;
    if face.tables().glyf.is_none() && matrices.is_none() {
        return Err(invalid(
            "outline conversion requires TrueType or CFF glyphs",
        ));
    }
    let (sin, cos) = f64::from(crate::text_direction(object, "CharDirection")?)
        .to_radians()
        .sin_cos();
    let moves = glyphs.len().saturating_sub(1);
    let dx = crate::expand_text_deltas(code.attribute("DeltaX"), moves)?;
    let dy = crate::expand_text_deltas(code.attribute("DeltaY"), moves)?;
    let mut x = crate::text_code_coordinate(*code, "X", 0.)?;
    let mut y = crate::text_code_coordinate(*code, "Y", size)?;
    let mut paths = Vec::with_capacity(glyphs.len());
    for (index, glyph) in glyphs.into_iter().enumerate() {
        if index != 0 {
            x += dx.as_ref().map_or(0., |v| v[index - 1]);
            y += dy.as_ref().map_or(0., |v| v[index - 1]);
        }
        let glyph = u16::try_from(glyph)
            .map_err(|_| invalid("outline conversion has an invalid glyph ID"))?;
        if glyph >= face.number_of_glyphs() {
            return Err(invalid("outline conversion has an invalid glyph ID"));
        }
        let [a, b, c, d, e, f] = match &matrices {
            Some(matrices) => matrices.for_glyph(glyph)?,
            None => {
                let scale = 1. / f64::from(face.units_per_em());
                [scale, 0., 0., scale, 0., 0.]
            }
        };
        let transform = [
            size * (cos * h_scale * a + sin * b),
            size * (sin * h_scale * a - cos * b),
            size * (cos * h_scale * c + sin * d),
            size * (sin * h_scale * c - cos * d),
            x + size * (cos * h_scale * e + sin * f),
            y + size * (sin * h_scale * e - cos * f),
        ];
        let transform = crate::multiply_matrix(outer_matrix, transform);
        if transform.iter().any(|v| !v.is_finite()) {
            return Err(invalid("outline conversion has nonfinite glyph placement"));
        }
        let mut outline = Outline {
            transform,
            commands: String::new(),
            budget,
            failed: None,
        };
        let complete = face
            .outline_glyph(ttf_parser::GlyphId(glyph), &mut outline)
            .is_some();
        if let Some(error) = outline.failed {
            return Err(error);
        }
        if !complete && !outline.commands.is_empty() {
            return Err(invalid(
                "outline conversion encountered an incomplete glyph",
            ));
        }
        paths.push(outline.commands);
    }
    Ok(paths)
}

struct Outline<'a> {
    transform: [f64; 6],
    commands: String,
    budget: &'a mut Budget,
    failed: Option<OfdError>,
}

impl Outline<'_> {
    fn append(&mut self, value: &str) {
        if self.failed.is_some() {
            return;
        }
        if value.len() > self.budget.bytes {
            self.failed = Some(OfdError::Limit(
                "PDF ActualText outline exceeds 32000000 bytes".into(),
            ));
        } else {
            self.budget.bytes -= value.len();
            self.commands.push_str(value);
        }
    }
    fn point(&mut self, command: &str, x: f32, y: f32) {
        if self.failed.is_some() {
            return;
        }
        if self.budget.points == 0 {
            self.failed = Some(OfdError::Limit(
                "PDF ActualText exceeds 500000 outline points".into(),
            ));
            return;
        }
        self.budget.points -= 1;
        let [a, b, c, d, e, f] = self.transform;
        let (x, y) = (f64::from(x), f64::from(y));
        let (x, y) = (a * x + c * y + e, b * x + d * y + f);
        if !x.is_finite() || !y.is_finite() {
            self.failed = Some(invalid("outline conversion has nonfinite points"));
            return;
        }
        self.append(&format!("{command} {x} {y}"));
    }
}

impl ttf_parser::OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.point(" M", x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.point(" L", x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.point(" Q", x1, y1);
        self.point("", x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.point(" B", x1, y1);
        self.point("", x2, y2);
        self.point("", x, y);
    }
    fn close(&mut self) {
        self.append(" C");
    }
}

pub(super) fn replace(
    document: &mut OfdDocument,
    page: &PageInfo,
    object: Node<'_, '_>,
    budget: &mut Budget,
) -> Result<String, OfdError> {
    let object_matrix = object
        .attribute("CTM")
        .map(crate::parse_ctm)
        .unwrap_or(Some([1., 0., 0., 1., 0., 0.]))
        .ok_or_else(|| invalid("outline conversion has invalid text matrix"))?;
    let stroke = object.attribute("Stroke") == Some("true");
    // PDF strokes use user-space units, independently of the text matrix (9.3.6).
    let baked = if stroke {
        object_matrix
    } else {
        [1., 0., 0., 1., 0., 0.]
    };
    let paths = glyph_paths(document, page, object, baked, budget)?
        .into_iter()
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Ok(String::new());
    }
    let boundary = object
        .attribute("Boundary")
        .and_then(crate::parse_box)
        .filter(|b| b[2] > 0. && b[3] > 0.)
        .ok_or_else(|| invalid("outline conversion has invalid boundary"))?;
    let source = object.document().input_text();
    let mut attributes = String::new();
    for name in [
        "CTM",
        "DrawParam",
        "LineWidth",
        "Cap",
        "Join",
        "MiterLimit",
        "DashOffset",
        "DashPattern",
    ] {
        if stroke && name == "CTM" {
            continue;
        }
        if let Some(value) = object.attribute(name) {
            attributes.push_str(&format!(" {name}=\"{}\"", crate::escape_xml(value)));
        }
    }
    for (name, default) in [("Fill", "true"), ("Stroke", "false")] {
        attributes.push_str(&format!(
            " {name}=\"{}\"",
            crate::escape_xml(object.attribute(name).unwrap_or(default))
        ));
    }
    let mut paints = String::new();
    for child in object.children().filter(Node::is_element) {
        if is_ofd_element(child, "FillColor") || is_ofd_element(child, "StrokeColor") {
            paints.push_str(&relocated_paint(child, object_matrix, baked, boundary)?);
        } else if !["TextCode", "CGTransform", "Clips", "Actions"]
            .iter()
            .any(|name| is_ofd_element(child, name))
        {
            return Err(invalid("outline conversion has an unsupported text child"));
        }
    }
    // Imported page sources use their root namespace; resources declare it explicitly.
    let namespace = object.tag_name().namespace().unwrap_or(crate::OFD_NS);
    let mut namespaces = String::new();
    for namespace in object.namespaces() {
        if namespace.name() == Some("ofd") {
            continue;
        }
        let name = namespace
            .name()
            .map_or(String::new(), |name| format!(":{name}"));
        namespaces.push_str(&format!(
            " xmlns{name}=\"{}\"",
            crate::escape_xml(namespace.uri())
        ));
    }
    // Pattern CellContent objects are copied per glyph, so every inline object
    // needs its own reserved ID rather than duplicating the source paint IDs.
    let paint_prefix = format!(
        "<ofd:Paints xmlns:ofd=\"{}\"{namespaces}>",
        crate::escape_xml(namespace)
    );
    let paint_source = format!("{paint_prefix}{paints}</ofd:Paints>");
    let parsed_paints = crate::parse_xml("ActualText paints", paint_source.as_bytes())?;
    let paint_ids = parsed_paints
        .descendants()
        .filter(|node| crate::has_ofd_namespace(*node))
        .filter_map(|node| node.attribute_node("ID"))
        .map(|attribute| {
            let range = attribute.range();
            range.start - paint_prefix.len()..range.end - paint_prefix.len()
        })
        .collect::<Vec<_>>();
    let stride = 1 + paint_ids.len();
    let child_count = paths
        .len()
        .checked_mul(stride)
        .ok_or_else(|| OfdError::Limit("ActualText object count overflow".into()))?;
    let overhead = paths.len().saturating_mul(
        attributes
            .len()
            .saturating_add(paints.len())
            .saturating_add(namespaces.len())
            .saturating_add(namespace.len())
            .saturating_add(paint_ids.len().saturating_mul(24))
            .saturating_add(256),
    );
    if overhead > budget.bytes {
        return Err(OfdError::Limit(
            "PDF ActualText outline exceeds 32000000 bytes".into(),
        ));
    }
    budget.bytes -= overhead;
    let resource = document.add_composite_graphic_resource_with_children(
        page.index, [boundary[2], boundary[3]], child_count, |base, width, height| {
            let mut xml = String::new();
            for (index, path) in paths.iter().enumerate() {
                let id = base + index * stride;
                let mut paints = paints.clone();
                for (offset, range) in paint_ids.iter().enumerate().rev() {
                    paints.replace_range(range.clone(), &format!("ID=\"{}\"", id + 1 + offset));
                }
                xml.push_str(&format!("<ofd:PathObject xmlns:ofd=\"{}\" ID=\"{}\" Boundary=\"0 0 {width} {height}\"{namespaces}{attributes}>{paints}<ofd:AbbreviatedData>{path}</ofd:AbbreviatedData></ofd:PathObject>", crate::escape_xml(namespace), id));
            }
            xml
        },
    )?;
    let tag = crate::qualified_tag(source, object)?;
    let prefix = tag
        .rsplit_once(':')
        .map_or(String::new(), |(prefix, _)| format!("{prefix}:"));
    let mut xml = format!("<{prefix}CompositeObject ResourceID=\"{resource}\"");
    for name in ["ID", "Boundary", "Visible", "Alpha", "BlendMode", "Name"] {
        if let Some(value) = object.attribute(name) {
            xml.push_str(&format!(" {name}=\"{}\"", crate::escape_xml(value)));
        }
    }
    xml.push('>');
    for child in object.children().filter(Node::is_element) {
        if is_ofd_element(child, "Actions") {
            xml.push_str(&source[child.range()]);
        }
        if is_ofd_element(child, "Clips") {
            xml.push_str(&relocated_clips(child, object.attribute("CTM"))?);
        }
    }
    xml.push_str(&format!("</{prefix}CompositeObject>"));
    if xml.len() > budget.bytes {
        return Err(OfdError::Limit(
            "PDF ActualText outline exceeds 32000000 bytes".into(),
        ));
    }
    budget.bytes -= xml.len();
    Ok(xml)
}

fn relocated_paint(
    color: Node<'_, '_>,
    original: [f64; 6],
    baked: [f64; 6],
    boundary: [f64; 4],
) -> Result<String, OfdError> {
    let source = color.document().input_text();
    let offset = color.range().start;
    let mut edits = Vec::new();
    for pattern in color.children().filter(Node::is_element) {
        if !is_ofd_element(pattern, "Pattern") {
            if baked != [1., 0., 0., 1., 0., 0.] {
                return Err(invalid(
                    "stroking transformed gradient text is not yet supported",
                ));
            }
            continue;
        }
        let matrix = pattern
            .attribute("CTM")
            .map(crate::parse_ctm)
            .unwrap_or(Some([1., 0., 0., 1., 0., 0.]))
            .ok_or_else(|| invalid("outline conversion has invalid pattern matrix"))?;
        let matrix = match pattern.attribute("RelativeTo").unwrap_or("Object") {
            value if value.eq_ignore_ascii_case("Object") => matrix,
            value if value.eq_ignore_ascii_case("Page") => {
                let mut page_matrix = original;
                page_matrix[4] += boundary[0];
                page_matrix[5] += boundary[1];
                let inverse = crate::inverse_matrix(page_matrix)
                    .ok_or_else(|| invalid("outline conversion has singular pattern placement"))?;
                crate::multiply_matrix(inverse, matrix)
            }
            _ => return Err(invalid("outline conversion has invalid pattern relativity")),
        };
        let matrix = crate::multiply_matrix(baked, matrix);
        if matrix.iter().any(|v| !v.is_finite()) {
            return Err(invalid("outline conversion has nonfinite pattern matrix"));
        }
        let matrix = matrix
            .iter()
            .map(f64::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        let mut missing = String::new();
        for (name, value) in [("CTM", matrix.as_str()), ("RelativeTo", "Object")] {
            let text = format!("{name}=\"{value}\"");
            if let Some(attribute) = pattern.attribute_node(name) {
                edits.push((attribute.range(), text));
            } else {
                missing.push(' ');
                missing.push_str(&text);
            }
        }
        if !missing.is_empty() {
            let start = pattern.range().start + crate::qualified_tag(source, pattern)?.len() + 1;
            edits.push((start..start, missing));
        }
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut result = source[color.range()].to_owned();
    for (range, value) in edits.into_iter().rev() {
        result.replace_range(range.start - offset..range.end - offset, &value);
    }
    Ok(result)
}

fn relocated_clips(clips: Node<'_, '_>, matrix: Option<&str>) -> Result<String, OfdError> {
    let source = clips.document().input_text();
    let mut xml = source[clips.range()].to_owned();
    if clips.attribute("TransFlag") != Some("true") {
        return Ok(xml);
    }
    let matrix = matrix
        .map(crate::parse_ctm)
        .unwrap_or(Some([1., 0., 0., 1., 0., 0.]))
        .ok_or_else(|| invalid("outline conversion has invalid text matrix"))?;
    let offset = clips.range().start;
    let areas = clips
        .children()
        .filter(|n| is_ofd_element(*n, "Clip"))
        .flat_map(|clip| clip.children().filter(|n| is_ofd_element(*n, "Area")))
        .collect::<Vec<_>>();
    for area in areas.into_iter().rev() {
        let local = area
            .attribute("CTM")
            .map(crate::parse_ctm)
            .unwrap_or(Some([1., 0., 0., 1., 0., 0.]))
            .ok_or_else(|| invalid("outline conversion has invalid clip matrix"))?;
        let matrix = crate::multiply_matrix(matrix, local);
        if matrix.iter().any(|v| !v.is_finite()) {
            return Err(invalid("outline conversion has nonfinite clip matrix"));
        }
        let value = matrix
            .iter()
            .map(f64::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        if let Some(attribute) = area.attribute_node("CTM") {
            let range = attribute.range();
            xml.replace_range(
                range.start - offset..range.end - offset,
                &format!("CTM=\"{value}\""),
            );
        } else {
            let tag = crate::qualified_tag(source, area)?;
            xml.insert_str(
                area.range().start + tag.len() + 1 - offset,
                &format!(" CTM=\"{value}\""),
            );
        }
    }
    Ok(xml)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> (OfdDocument, String) {
        let mut doc = OfdDocument::new();
        doc.add_blank_page(50., 30.).unwrap();
        let font = doc
            .add_embedded_font(
                "DejaVu",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/dejavu-serif.ttf"
                )),
            )
            .unwrap()
            .to_string();
        let id = doc
            .add_shaped_text_object(0, "ABA", [2., 3., 40., 20.], 6., &font)
            .unwrap();
        (doc, id)
    }

    #[test]
    fn outlines_keep_each_glyph_separate_and_match_native_paint() {
        let (mut doc, id) = source();
        let before = doc.page_png(0, 144.).unwrap();
        let page = doc.pages().unwrap().remove(0);
        let parsed = crate::parse_xml(&page.path, doc.required_entry(&page.path).unwrap()).unwrap();
        let object = parsed
            .descendants()
            .find(|n| n.attribute("ID") == Some(id.as_str()))
            .unwrap();
        let paths = glyph_paths(
            &doc,
            &page,
            object,
            [1., 0., 0., 1., 0., 0.],
            &mut Budget::default(),
        )
        .unwrap();
        assert_eq!(paths.len(), 3);
        assert!(paths
            .iter()
            .all(|path| path.contains(" Q") || path.contains(" L")));
        doc.remove_page_object(0, &id).unwrap();
        for path in paths {
            doc.add_path_object(
                0,
                &path,
                [2., 3., 40., 20.],
                crate::PathStyle {
                    fill: true,
                    stroke: false,
                    ..Default::default()
                },
            )
            .unwrap();
        }
        assert_eq!(doc.page_png(0, 144.).unwrap(), before);
        assert!(doc.validate_references().unwrap().is_empty());
    }

    #[test]
    fn replacement_preserves_group_opacity_affine_geometry_and_both_clip_modes() {
        for follows_ctm in [false, true] {
            let (mut doc, id) = source();
            doc.set_page_object_ctm(0, &id, Some([0.8, 0.15, -0.2, 0.9, 3., 1.]))
                .unwrap();
            doc.set_page_object_alpha(0, &id, 128).unwrap();
            doc.set_page_object_clip_rects(0, &id, &[[0., 0., 10., 10.]], follows_ctm)
                .unwrap();
            let before = doc.page_png(0, 144.).unwrap();
            super::super::Replacement {
                text: String::new(),
                first_id: id.parse().unwrap(),
            }
            .apply(&mut doc, 0)
            .unwrap();
            assert!(doc.page_text(0).unwrap().is_empty());
            assert_eq!(
                doc.page_png(0, 144.).unwrap(),
                before,
                "clip follows CTM: {follows_ctm}"
            );
            assert!(doc.validate_references().unwrap().is_empty());
        }
    }

    #[test]
    fn patterned_glyphs_keep_page_and_object_paint_placement_and_unique_ids() {
        use crate::{
            PathStyle, PatternCell, PatternReflectMethod, PatternRelativeTo, PatternStyle,
        };
        for relative_to in [PatternRelativeTo::Object, PatternRelativeTo::Page] {
            let (mut doc, id) = source();
            doc.set_page_object_ctm(0, &id, Some([0.8, 0.15, -0.2, 0.9, 3., 1.]))
                .unwrap();
            doc.set_page_object_pattern_fill(
                0,
                &id,
                &PatternStyle {
                    width: 2.,
                    height: 2.,
                    x_step: None,
                    y_step: None,
                    reflect: PatternReflectMethod::Normal,
                    relative_to,
                    ctm: Some([1., 0., 0., 1., 1., 1.]),
                    alpha: None,
                    thumbnail: None,
                    cells: vec![PatternCell::Path {
                        commands: "M 0 0 L 1 0 L 1 2 L 0 2 C".into(),
                        boundary: [0., 0., 2., 2.],
                        style: PathStyle {
                            fill: true,
                            stroke: false,
                            fill_color: Some([255, 0, 0]),
                            ..Default::default()
                        },
                    }],
                },
            )
            .unwrap();
            super::super::Replacement {
                text: String::new(),
                first_id: id.parse().unwrap(),
            }
            .apply(&mut doc, 0)
            .unwrap();
            assert!(doc.page_text(0).unwrap().is_empty());
            let data = image::load_from_memory(&doc.page_png(0, 144.).unwrap())
                .unwrap()
                .to_rgb8();
            assert!(data.pixels().any(|pixel| pixel[0] > 200 && pixel[1] < 100));
            let path = doc
                .entry_names()
                .find(|name| name.contains("CompositeRes_"))
                .unwrap();
            let parsed = crate::parse_xml(path, doc.required_entry(path).unwrap()).unwrap();
            let expected = match relative_to {
                PatternRelativeTo::Object => [1., 0., 0., 1., 1., 1.],
                PatternRelativeTo::Page => [
                    1.2,
                    -0.2,
                    0.2666666666666667,
                    1.0666666666666667,
                    -5.6,
                    -2.4,
                ],
            };
            let patterns = parsed
                .descendants()
                .filter(|node| is_ofd_element(*node, "Pattern"))
                .collect::<Vec<_>>();
            assert_eq!(patterns.len(), 3);
            for pattern in patterns {
                assert_eq!(pattern.attribute("RelativeTo"), Some("Object"));
                let matrix = crate::parse_ctm(pattern.attribute("CTM").unwrap()).unwrap();
                for (value, expected) in matrix.into_iter().zip(expected) {
                    assert!((value - expected).abs() < 1e-12);
                }
            }
            assert!(doc.validate_references().unwrap().is_empty());
        }
    }

    #[test]
    fn glyph_point_and_output_budgets_are_enforced() {
        let (doc, id) = source();
        let page = doc.pages().unwrap().remove(0);
        let parsed = crate::parse_xml(&page.path, doc.required_entry(&page.path).unwrap()).unwrap();
        let object = parsed
            .descendants()
            .find(|n| n.attribute("ID") == Some(id.as_str()))
            .unwrap();
        for mut budget in [
            Budget {
                glyphs: 2,
                ..Default::default()
            },
            Budget {
                points: 1,
                ..Default::default()
            },
            Budget {
                bytes: 1,
                ..Default::default()
            },
        ] {
            assert!(matches!(
                glyph_paths(&doc, &page, object, [1., 0., 0., 1., 0., 0.], &mut budget),
                Err(OfdError::Limit(_))
            ));
        }
    }
}
