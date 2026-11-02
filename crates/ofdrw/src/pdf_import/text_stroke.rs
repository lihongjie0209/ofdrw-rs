//! Preserve affine text pens as vector outlines with a separate semantic text run.
use super::*;
use std::fmt::Write;

pub(super) struct Run<'a> {
    pub object_id: &'a str,
    pub text: &'a str,
    pub glyphs: Option<&'a [u16]>,
    pub positions: &'a [f64],
    pub vertical: Option<&'a [f64]>,
}

pub(super) fn required(graphics: &GraphicsState, state: &TextState) -> bool {
    if !matches!(state.render_mode % 4, 1 | 2) || graphics.stroke_space.nonpainting() {
        return false;
    }
    let [a, b, c, d, _, _] = state.matrix;
    let x = a.hypot(b);
    let y = c.hypot(d);
    graphics.line_width == 0.
        || graphics.stroke_scale().is_err()
        || (x - y).abs() > 1e-6 * x.max(y)
        || (a * c + b * d).abs() > 1e-6 * x * y
        || state.h_scale.abs() != 1.
}

pub(super) fn paint(
    output: &mut OfdDocument,
    page: &PdfPageContext<'_>,
    graphics: &GraphicsState,
    state: &TextState,
    run: Run<'_>,
) -> Result<(), OfdError> {
    let object = output
        .page_text_objects(page.index)?
        .into_iter()
        .find(|object| object.id.as_deref() == Some(run.object_id))
        .ok_or_else(|| OfdError::Invalid("PDF semantic text object is missing".into()))?;
    let (document_index, _) = output.page_location(page.index)?;
    let font = output
        .fonts()?
        .into_iter()
        .find(|font| {
            font.document_index == document_index
                && object.font_id.as_ref().is_some_and(|id| {
                    crate::resource_id_key(id) == crate::resource_id_key(&font.id)
                })
        })
        .ok_or_else(|| OfdError::Invalid("PDF stroke font is missing".into()))?;
    let build = |data: &[u8], index| outlines(data, index, page, graphics, state, &run);
    let paths = if let Some(path) = font.embedded_path {
        build(output.required_entry(&path)?, 0)?
    } else {
        use resvg::usvg::fontdb::{Family, Query, Style, Weight};
        let database = crate::system_fonts();
        let id = database
            .query(&Query {
                families: &[Family::Name(&font.font_name), Family::SansSerif],
                weight: Weight(object.weight),
                style: if object.italic {
                    Style::Italic
                } else {
                    Style::Normal
                },
                ..Query::default()
            })
            .ok_or_else(|| OfdError::Invalid("no usable font for PDF text stroke".into()))?;
        database
            .with_face_data(id, build)
            .ok_or_else(|| OfdError::Invalid("PDF stroke fallback font has no data".into()))??
    };
    for commands in paths {
        let mut path = PathBuilder {
            commands,
            ..Default::default()
        };
        super::paint(
            output,
            page,
            &mut path,
            graphics,
            PdfPaint {
                fill: state.render_mode % 4 == 2,
                stroke: true,
                even_odd: false,
            },
            true,
        )?;
    }
    Ok(())
}

fn outlines(
    data: &[u8],
    index: u32,
    page: &PdfPageContext<'_>,
    graphics: &GraphicsState,
    state: &TextState,
    run: &Run<'_>,
) -> Result<Vec<String>, OfdError> {
    let wrapped;
    let face = match ttf_parser::Face::parse(data, index) {
        Ok(face) => face,
        Err(_) if ttf_parser::cff::Table::parse(data).is_some() => {
            wrapped = cff_font::wrap(data)?;
            ttf_parser::Face::parse(&wrapped, 0)
                .map_err(|_| OfdError::Invalid("invalid PDF CFF stroke font".into()))?
        }
        Err(_) => return Err(OfdError::Invalid("invalid PDF stroke font".into())),
    };
    let cff = face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"CFF "))
        .filter(|_| face.tables().glyf.is_none())
        .map(cff_font::GlyphMatrices::parse)
        .transpose()?;
    let glyphs = run.glyphs.map(<[u16]>::to_vec).unwrap_or_else(|| {
        run.text
            .chars()
            .map(|ch| face.glyph_index(ch).unwrap_or(ttf_parser::GlyphId(0)).0)
            .collect()
    });
    if glyphs.len() > 10_000 {
        return Err(OfdError::Limit(
            "PDF text stroke exceeds 10000 glyphs".into(),
        ));
    }
    if glyphs.len() != run.positions.len() {
        return Err(OfdError::Invalid(
            "PDF text stroke advances do not match glyphs".into(),
        ));
    }
    let [a, b, c, d, e, f] = graphics.ctm;
    let world = [
        a * MM_PER_PT,
        -b * MM_PER_PT,
        c * MM_PER_PT,
        -d * MM_PER_PT,
        (e - page.box_[0]) * MM_PER_PT,
        (page.box_[3] - f) * MM_PER_PT,
    ];
    let text = crate::multiply_matrix(world, state.matrix);
    let unit = 1. / f64::from(face.units_per_em());
    let mut paths = Vec::new();
    let mut total = 0usize;
    for (index, glyph) in glyphs.into_iter().enumerate() {
        if glyph >= face.number_of_glyphs() {
            return Err(OfdError::Invalid("PDF stroke glyph is out of range".into()));
        }
        let glyph_matrix = cff
            .as_ref()
            .map_or(Ok([unit, 0., 0., unit, 0., 0.]), |matrices| {
                matrices.for_glyph(glyph)
            })?;
        let local = [
            state.size_pt * state.h_scale,
            0.,
            0.,
            state.size_pt,
            run.positions[index] * state.h_scale,
            state.rise + run.vertical.map_or(0., |p| p[index]),
        ];
        let mut outline = Outline {
            matrix: crate::multiply_matrix(text, crate::multiply_matrix(local, glyph_matrix)),
            commands: String::new(),
            invalid: false,
        };
        if face
            .outline_glyph(ttf_parser::GlyphId(glyph), &mut outline)
            .is_none()
        {
            continue;
        }
        if outline.invalid || outline.commands.len() > 1_000_000 {
            return Err(OfdError::Limit(
                "PDF text stroke outline is invalid or too large".into(),
            ));
        }
        total += outline.commands.len();
        if total > 32_000_000 {
            return Err(OfdError::Limit(
                "PDF text stroke outlines exceed 32 MB".into(),
            ));
        }
        if !outline.commands.is_empty() {
            paths.push(outline.commands);
        }
    }
    Ok(paths)
}

struct Outline {
    matrix: [f64; 6],
    commands: String,
    invalid: bool,
}
impl Outline {
    fn point(&mut self, prefix: &str, x: f32, y: f32) {
        if self.invalid || self.commands.len() > 1_000_000 {
            return;
        }
        let [a, b, c, d, e, f] = self.matrix;
        let px = a * f64::from(x) + c * f64::from(y) + e;
        let py = b * f64::from(x) + d * f64::from(y) + f;
        if !px.is_finite() || !py.is_finite() {
            self.invalid = true;
            return;
        }
        // String formatting is infallible.
        let _ = write!(&mut self.commands, " {prefix} {px} {py}");
    }
}
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.point("M", x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.point("L", x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.point("Q", x1, y1);
        self.point("", x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.point("B", x1, y1);
        self.point("", x2, y2);
        self.point("", x, y);
    }
    fn close(&mut self) {
        if self.commands.len() <= 1_000_000 {
            self.commands.push_str(" C");
        }
    }
}
