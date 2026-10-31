//! Share a short replacement across compatible, consecutive native text runs.
use std::collections::HashMap;

use roxmltree::{Attribute, Node};

use super::{invalid, mapped_glyphs, NativeRun};
use crate::{is_ofd_element, parse_box, parse_ctm, qualified_tag, OfdDocument, OfdError, PageInfo};

pub(super) fn merge(
    document: &OfdDocument,
    page: &PageInfo,
    runs: &[NativeRun<'_, '_>],
    replacement: &str,
) -> Result<String, OfdError> {
    if replacement.is_empty() {
        return Err(invalid(
            "empty replacement of painted text is not yet supported",
        ));
    }
    let first = runs[0].0;
    let mut siblings = first
        .parent()
        .ok_or_else(|| invalid("native text has no parent"))?
        .children()
        .filter(Node::is_element)
        .skip_while(|node| *node != first);
    for (object, _, _) in runs {
        if siblings.next() != Some(*object) || !same_style(first, *object) {
            return Err(invalid(
                "short replacement requires consecutive matching text styles",
            ));
        }
    }
    let paint = solid_paint(first)?;
    let size = crate::svg_number(first.attribute("Size"), 0.0)?;
    let h_scale = crate::svg_number(first.attribute("HScale"), 1.0)?;
    let direction = f64::from(crate::text_direction(first, "CharDirection")?);
    if size <= 0.0 || h_scale <= 0.0 {
        return Err(invalid("short replacement has invalid text size or scale"));
    }
    let fonts = document.fonts_for_page(&page.path, page.document_index)?;
    let font = first
        .attribute("Font")
        .and_then(|id| {
            fonts
                .iter()
                .find(|font| crate::resource_id_key(&font.id) == crate::resource_id_key(id))
        })
        .and_then(|font| font.embedded_path.as_deref())
        .ok_or_else(|| invalid("short replacement requires an embedded font"))?;
    let bytes = document.required_entry(font)?;
    let wrapped;
    let face = match ttf_parser::Face::parse(bytes, 0) {
        Ok(face) => face,
        Err(_) if ttf_parser::cff::Table::parse(bytes).is_some() => {
            wrapped = crate::pdf_import::cff_font::wrap(bytes)?;
            ttf_parser::Face::parse(&wrapped, 0)
                .map_err(|_| invalid("short replacement requires a usable outline font"))?
        }
        Err(_) => return Err(invalid("short replacement requires a usable outline font")),
    };
    let cff_matrices = face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"CFF "))
        .filter(|_| face.tables().glyf.is_none())
        .map(crate::pdf_import::cff_font::GlyphMatrices::parse)
        .transpose()?;
    let (base_matrix, base_boundary) = placement(first)?;
    let [a, b, c, d, _, _] = base_matrix;
    let determinant = a * d - b * c;
    if !determinant.is_finite() || determinant == 0.0 {
        return Err(invalid("short replacement has a singular text matrix"));
    }
    let (sin, cos) = direction.to_radians().sin_cos();
    let outline_linear = [
        size * h_scale * (a * cos + c * sin),
        size * h_scale * (b * cos + d * sin),
        size * (a * sin - c * cos),
        size * (b * sin - d * cos),
    ];
    let base_origin = [
        base_boundary[0] + base_matrix[4],
        base_boundary[1] + base_matrix[5],
    ];
    let mut bounds = [
        base_boundary[0],
        base_boundary[1],
        base_boundary[0] + base_boundary[2],
        base_boundary[1] + base_boundary[3],
    ];
    let mut glyphs = Vec::new();
    let mut positions = Vec::new();
    let mut outline_bounds = HashMap::new();
    let mut outline_budget = 500_000;
    for (object, code, text) in runs {
        if solid_paint(*object)? != paint {
            return Err(invalid("short replacement has different text paints"));
        }
        if code.attributes().any(|attribute| {
            attribute.namespace().is_some()
                || !matches!(attribute.name(), "X" | "Y" | "DeltaX" | "DeltaY")
        }) {
            return Err(invalid(
                "short replacement has unsupported text positioning attributes",
            ));
        }
        let (matrix, boundary) = placement(*object)?;
        if matrix[..4] != base_matrix[..4] {
            return Err(invalid("short replacement has different text matrices"));
        }
        let mapped = mapped_glyphs(*object, text)?;
        if glyphs.len() + mapped.len() > 10_000 {
            return Err(OfdError::Limit(
                "PDF ActualText exceeds 10000 merged glyphs".into(),
            ));
        }
        let dx = crate::expand_text_deltas(code.attribute("DeltaX"), mapped.len() - 1)?;
        let dy = crate::expand_text_deltas(code.attribute("DeltaY"), mapped.len() - 1)?;
        let origin = [boundary[0] + matrix[4], boundary[1] + matrix[5]];
        let translation = [origin[0] - base_origin[0], origin[1] - base_origin[1]];
        let shift = [
            (d * translation[0] - c * translation[1]) / determinant,
            (a * translation[1] - b * translation[0]) / determinant,
        ];
        let mut x = crate::text_code_coordinate(*code, "X", 0.0)?;
        let mut y = crate::text_code_coordinate(*code, "Y", size)?;
        for (index, glyph) in mapped.iter().enumerate() {
            if index > 0 {
                x += dx.as_ref().map_or(0.0, |values| values[index - 1]);
                y += dy.as_ref().map_or(0.0, |values| values[index - 1]);
            }
            let position = [x + shift[0], y + shift[1]];
            if position.iter().any(|value| !value.is_finite()) {
                return Err(invalid("short replacement has nonfinite glyph positions"));
            }
            // The font and linear transform are shared by all runs. A repeated
            // glyph needs one outline walk and a constant-time clipping check.
            let bounds = if let Some(bounds) = outline_bounds.get(glyph) {
                *bounds
            } else {
                let bounds = glyph_bounds(
                    &face,
                    *glyph,
                    outline_linear,
                    cff_matrices.as_ref(),
                    &mut outline_budget,
                )?;
                outline_bounds.insert(*glyph, bounds);
                bounds
            };
            check_boundary(
                bounds,
                boundary,
                [origin[0] + a * x + c * y, origin[1] + b * x + d * y],
            )?;
            positions.push(position);
        }
        glyphs.extend(mapped);
        bounds[0] = bounds[0].min(boundary[0]);
        bounds[1] = bounds[1].min(boundary[1]);
        bounds[2] = bounds[2].max(boundary[0] + boundary[2]);
        bounds[3] = bounds[3].max(boundary[1] + boundary[3]);
    }
    let boundary = [
        bounds[0],
        bounds[1],
        bounds[2] - bounds[0],
        bounds[3] - bounds[1],
    ];
    let matrix = [
        a,
        b,
        c,
        d,
        base_origin[0] - bounds[0],
        base_origin[1] - bounds[1],
    ];
    if boundary
        .iter()
        .chain(matrix.iter())
        .any(|value| !value.is_finite())
    {
        return Err(invalid("short replacement has nonfinite merged geometry"));
    }
    merged_xml(
        first,
        runs[0].1,
        replacement,
        boundary,
        matrix,
        &glyphs,
        &positions,
    )
}

fn same_style(first: Node<'_, '_>, other: Node<'_, '_>) -> bool {
    let included = |attribute: &Attribute<'_, '_>| {
        attribute.namespace().is_some() || !matches!(attribute.name(), "ID" | "Boundary" | "CTM")
    };
    first.attributes().filter(included).count() == other.attributes().filter(included).count()
        && first.attributes().filter(included).all(|attribute| {
            other.attributes().any(|candidate| {
                candidate.namespace() == attribute.namespace()
                    && candidate.name() == attribute.name()
                    && candidate.value() == attribute.value()
            })
        })
}

fn solid_paint<'a, 'input>(object: Node<'a, 'input>) -> Result<Vec<&'input str>, OfdError> {
    if object.attribute("Stroke") == Some("true")
        || object
            .attribute("Alpha")
            .is_some_and(|value| value != "255")
        || object
            .attribute("BlendMode")
            .is_some_and(|value| !value.eq_ignore_ascii_case("Normal"))
    {
        return Err(invalid(
            "short replacement requires opaque, normally blended fill text",
        ));
    }
    let mut paint = Vec::new();
    for child in object.children().filter(Node::is_element) {
        if is_ofd_element(child, "TextCode") || is_ofd_element(child, "CGTransform") {
            continue;
        }
        if !(is_ofd_element(child, "FillColor") || is_ofd_element(child, "StrokeColor"))
            || child.children().any(|node| node.is_element())
        {
            return Err(invalid(
                "short replacement does not support text clips, actions or gradient paints",
            ));
        }
        paint.push(&object.document().input_text()[child.range()]);
    }
    Ok(paint)
}

fn placement(object: Node<'_, '_>) -> Result<([f64; 6], [f64; 4]), OfdError> {
    let boundary = object
        .attribute("Boundary")
        .and_then(parse_box)
        .filter(|values| values[2] > 0.0 && values[3] > 0.0)
        .ok_or_else(|| invalid("short replacement has invalid text boundary"))?;
    let matrix = object
        .attribute("CTM")
        .map(parse_ctm)
        .unwrap_or(Some([1., 0., 0., 1., 0., 0.]))
        .ok_or_else(|| invalid("short replacement has invalid text matrix"))?;
    Ok((matrix, boundary))
}

fn glyph_bounds(
    face: &ttf_parser::Face<'_>,
    glyph: u32,
    linear: [f64; 4],
    cff_matrices: Option<&crate::pdf_import::cff_font::GlyphMatrices>,
    budget: &mut usize,
) -> Result<Option<[f64; 4]>, OfdError> {
    let glyph =
        u16::try_from(glyph).map_err(|_| invalid("short replacement has invalid glyph ID"))?;
    if glyph >= face.number_of_glyphs() {
        return Err(invalid("short replacement has invalid glyph ID"));
    }
    let [a, b, c, d, e, f] = match cff_matrices {
        Some(matrices) => matrices.for_glyph(glyph)?,
        None => {
            let scale = 1.0 / f64::from(face.units_per_em());
            [scale, 0., 0., scale, 0., 0.]
        }
    };
    let transform = [
        linear[0] * a + linear[2] * b,
        linear[1] * a + linear[3] * b,
        linear[0] * c + linear[2] * d,
        linear[1] * c + linear[3] * d,
        linear[0] * e + linear[2] * f,
        linear[1] * e + linear[3] * f,
    ];
    if transform.iter().any(|value| !value.is_finite()) {
        return Err(invalid(
            "short replacement has nonfinite glyph outline geometry",
        ));
    }
    let mut outline = OutlineBounds {
        transform,
        bounds: None,
        budget,
        limited: false,
        invalid: false,
    };
    let painted = face
        .outline_glyph(ttf_parser::GlyphId(glyph), &mut outline)
        .is_some();
    if outline.limited {
        return Err(OfdError::Limit(
            "PDF ActualText exceeds 500000 outline points".into(),
        ));
    }
    if outline.invalid {
        return Err(invalid("short replacement has nonfinite outline points"));
    }
    Ok(if painted { outline.bounds } else { None })
}

fn check_boundary(
    bounds: Option<[f64; 4]>,
    boundary: [f64; 4],
    origin: [f64; 2],
) -> Result<(), OfdError> {
    let Some([min_x, min_y, max_x, max_y]) = bounds else {
        return Ok(());
    };
    let points = [
        min_x + origin[0],
        min_y + origin[1],
        max_x + origin[0],
        max_y + origin[1],
    ];
    let [left, top, width, height] = boundary;
    if points.iter().any(|value| !value.is_finite())
        || points[0] < left - 1e-9
        || points[1] < top - 1e-9
        || points[2] > left + width + 1e-9
        || points[3] > top + height + 1e-9
    {
        return Err(invalid(
            "short replacement would change a glyph's boundary clipping",
        ));
    }
    Ok(())
}

struct OutlineBounds<'a> {
    transform: [f64; 6],
    bounds: Option<[f64; 4]>,
    budget: &'a mut usize,
    limited: bool,
    invalid: bool,
}

impl OutlineBounds<'_> {
    fn point(&mut self, x: f32, y: f32) {
        let Some(remaining) = self.budget.checked_sub(1) else {
            self.limited = true;
            return;
        };
        *self.budget = remaining;
        let [a, b, c, d, e, f] = self.transform;
        let px = a * f64::from(x) + c * f64::from(y) + e;
        let py = b * f64::from(x) + d * f64::from(y) + f;
        // Bezier curves lie within their control-point hull. Checking every
        // point ensures enlarging the shared boundary cannot expose clipped ink.
        if !px.is_finite() || !py.is_finite() {
            self.invalid = true;
            return;
        }
        self.bounds = Some(match self.bounds {
            Some([min_x, min_y, max_x, max_y]) => {
                [min_x.min(px), min_y.min(py), max_x.max(px), max_y.max(py)]
            }
            None => [px, py, px, py],
        });
    }
}

impl ttf_parser::OutlineBuilder for OutlineBounds<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.point(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.point(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.point(x1, y1);
        self.point(x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.point(x1, y1);
        self.point(x2, y2);
        self.point(x, y);
    }
    fn close(&mut self) {}
}

fn merged_xml(
    first: Node<'_, '_>,
    code: Node<'_, '_>,
    replacement: &str,
    boundary: [f64; 4],
    matrix: [f64; 6],
    glyphs: &[u32],
    positions: &[[f64; 2]],
) -> Result<String, OfdError> {
    let source = first.document().input_text();
    let transforms = first
        .children()
        .filter(|node| is_ofd_element(*node, "CGTransform"))
        .collect::<Vec<_>>();
    let transform = transforms[0];
    let object_tag = qualified_tag(source, first)?;
    let prefix = object_tag
        .strip_suffix("TextObject")
        .ok_or_else(|| invalid("short replacement has invalid native text tag"))?;
    let tag = format!("{prefix}CGTransform");
    let glyph_tag = format!("{prefix}Glyphs");
    let list = glyphs
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    let mapping = format!("<{tag} CodePosition=\"0\" CodeCount=\"{}\" GlyphCount=\"{}\"><{glyph_tag}>{list}</{glyph_tag}></{tag}>", replacement.encode_utf16().count(), glyphs.len());
    let delta = |axis: usize| {
        positions
            .windows(2)
            .map(|pair| pair[1][axis] - pair[0][axis])
            .collect::<Vec<_>>()
    };
    let dx = delta(0);
    let dy = delta(1);
    if dx.iter().chain(dy.iter()).any(|value| !value.is_finite()) {
        return Err(invalid("short replacement has nonfinite glyph advances"));
    }
    let numbers = |values: &[f64]| {
        values
            .iter()
            .map(f64::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    };
    let code_tag = format!("{prefix}TextCode");
    let content = format!(
        "<{code_tag} X=\"{}\" Y=\"{}\" DeltaX=\"{}\" DeltaY=\"{}\">{}</{code_tag}>",
        positions[0][0],
        positions[0][1],
        numbers(&dx),
        numbers(&dy),
        crate::escape_text_code(replacement)
    );
    let mut edits = vec![(code.range(), content), (transform.range(), mapping)];
    for redundant in transforms.iter().skip(1) {
        edits.push((redundant.range(), String::new()));
    }
    for (name, value) in [("Boundary", numbers(&boundary)), ("CTM", numbers(&matrix))] {
        let value = format!("{name}=\"{value}\"");
        if let Some(attribute) = first
            .attributes()
            .find(|attribute| attribute.namespace().is_none() && attribute.name() == name)
        {
            edits.push((attribute.range(), value));
        } else {
            let at = first.range().start + 1 + qualified_tag(source, first)?.len();
            edits.push((at..at, format!(" {value}")));
        }
    }
    edits.sort_by_key(|(range, _)| range.start);
    let start = first.range().start;
    let mut result = source[first.range()].to_owned();
    for (range, value) in edits.into_iter().rev() {
        result.replace_range(range.start - start..range.end - start, &value);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face() -> ttf_parser::Face<'static> {
        ttf_parser::Face::parse(include_bytes!("../../../tests/fixtures/dejavu-sans.ttf"), 0)
            .unwrap()
    }

    #[test]
    fn outline_validation_stops_when_remaining_work_is_exhausted() {
        let face = face();
        let glyph = u32::from(face.glyph_index('A').unwrap().0);
        assert!(
            matches!(glyph_bounds(&face, glyph, [1., 0., 0., 1.], None, &mut 1), Err(OfdError::Limit(message)) if message.contains("outline points"))
        );
    }

    #[test]
    fn blank_glyphs_need_no_outline_work() {
        let face = face();
        let glyph = u32::from(face.glyph_index(' ').unwrap().0);
        assert_eq!(
            glyph_bounds(&face, glyph, [1., 0., 0., 1.], None, &mut 0).unwrap(),
            None
        );
    }
}
