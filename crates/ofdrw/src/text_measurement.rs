use super::*;

/// One wrapped line in millimeters, relative to the paragraph's top-left corner.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowLineMetrics {
    pub text: String,
    /// Horizontal position including indentation and alignment.
    pub x: f64,
    pub y: f64,
    /// Logical advance width; this is not a glyph ink bounding box.
    /// Natural zero-advance glyphs can produce zero width without empty text.
    pub width: f64,
    pub height: f64,
    /// Baseline position measured from the paragraph's top.
    pub baseline: f64,
    pub paragraph_start: bool,
}

/// Unpaginated logical text dimensions, including blank lines and paragraph gaps.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowTextMetrics {
    /// Largest line advance, excluding indentation and alignment offsets.
    pub width: f64,
    pub height: f64,
    pub lines: Vec<FlowLineMetrics>,
}

pub(super) struct PreparedSpans {
    pub(super) spans: Vec<PreparedFlowSpan>,
    fonts: Vec<FontInfo>,
    pub(super) first_line_indent: f64,
    leading_placeholder: bool,
}

impl PreparedSpans {
    pub(super) fn indents(&self, style: &FlowTextStyle) -> FlowIndents {
        FlowIndents {
            first: self.first_line_indent,
            rest: style.rest_line_indent,
            initial: self.leading_placeholder.then_some(0.),
        }
    }
}

pub(super) struct ShapedLines {
    pub(super) lines: Vec<ShapedStyledLine>,
    pub(super) group_widths: Vec<Vec<f64>>,
}

pub(super) enum PreparedFlowLines {
    Plain(Vec<StyledFlowLine>),
    Shaped(ShapedLines),
}

pub(super) struct PreparedFlowText {
    pub(super) metrics: FlowTextMetrics,
    pub(super) preparation: PreparedSpans,
    pub(super) lines: PreparedFlowLines,
}

impl OfdDocument {
    /// Measure styled or shaped flowing text without changing the document.
    ///
    /// Uses exactly the font selection, kerning, tracking, wrapping, grapheme
    /// grouping and shaping used by `add_flowed_spans`. `content_width` is the
    /// available line width in millimeters. Page size, margins, start position
    /// and page keep rules are ignored: the result describes one continuous
    /// paragraph before pagination. Interior blank lines occupy line height;
    /// terminal empty paragraphs are discarded, matching the renderer.
    ///
    /// No pages, objects or resource IDs are allocated. Returned line positions
    /// include indentation, alignment and paragraph spacing; height includes
    /// the last line's leading. These are layout metrics, not painted bounds.
    ///
    /// ```
    /// use ofdrw::{FlowSpan, FlowTextStyle, OfdDocument};
    /// let document = OfdDocument::new();
    /// let metrics = document.measure_flowed_spans(0,
    ///     &[FlowSpan::new("First line\nSecond line")], 40., &FlowTextStyle::default())?;
    /// assert!(metrics.lines.len() >= 2);
    /// assert!(document.pages()?.is_empty());
    /// # Ok::<(), ofdrw::OfdError>(())
    /// ```
    pub fn measure_flowed_spans(
        &self,
        document_index: usize,
        spans: &[FlowSpan],
        content_width: f64,
        style: &FlowTextStyle,
    ) -> Result<FlowTextMetrics, OfdError> {
        Ok(self
            .prepare_measured_flow_text(document_index, spans, content_width, style)?
            .metrics)
    }

    pub(super) fn prepare_measured_flow_text(
        &self,
        document_index: usize,
        spans: &[FlowSpan],
        content_width: f64,
        style: &FlowTextStyle,
    ) -> Result<PreparedFlowText, OfdError> {
        let preparation = self.prepare_flow_spans(document_index, spans, style, content_width)?;
        let prepared = &preparation.spans;
        let leading = style.line_height - style.font_size;
        let flow_lines = if style.shape {
            PreparedFlowLines::Shaped(self.prepare_shaped_flow_lines(
                document_index,
                spans,
                style,
                content_width,
                &preparation,
                true,
            )?)
        } else {
            let paragraphs = styled_flow_paragraphs(spans, prepared);
            PreparedFlowLines::Plain(wrap_styled_flow_lines(
                &paragraphs,
                content_width,
                preparation.indents(style),
                prepared,
                style.indent_each_paragraph,
            )?)
        };
        let mut lines: Vec<FlowLineMetrics> = match &flow_lines {
            PreparedFlowLines::Shaped(shaped) => shaped
                .lines
                .iter()
                .map(|line| {
                    let baseline = if line.units.is_empty() {
                        style.font_size
                    } else {
                        line.units
                            .iter()
                            .map(|unit| prepared[unit.span_index].font_size)
                            .fold(0., f64::max)
                    };
                    FlowLineMetrics {
                        text: line
                            .units
                            .iter()
                            .filter(|u| prepared[u.span_index].placeholder_width.is_none())
                            .map(|u| u.text.as_str())
                            .collect(),
                        x: line.indent,
                        y: 0.,
                        width: line.width,
                        height: baseline + leading,
                        baseline,
                        paragraph_start: line.paragraph_start,
                    }
                })
                .collect(),
            PreparedFlowLines::Plain(plain) => plain
                .iter()
                .map(|line| {
                    let baseline = if line.glyphs.is_empty() {
                        style.font_size
                    } else {
                        line.glyphs
                            .iter()
                            .map(|glyph| prepared[glyph.span_index].font_size)
                            .fold(0., f64::max)
                    };
                    FlowLineMetrics {
                        text: line
                            .glyphs
                            .iter()
                            .filter(|g| prepared[g.span_index].placeholder_width.is_none())
                            .map(|g| g.ch)
                            .collect(),
                        x: line.indent,
                        y: 0.,
                        width: line.width,
                        height: baseline + leading,
                        baseline,
                        paragraph_start: line.paragraph_start,
                    }
                })
                .collect(),
        };
        if lines.is_empty() {
            return Err(OfdError::Invalid(
                "flowed spans have no rendered lines".into(),
            ));
        }
        let mut height = 0.;
        let mut width: f64 = 0.;
        for (index, line) in lines.iter_mut().enumerate() {
            if index != 0 && line.paragraph_start {
                height += style.paragraph_gap;
            }
            line.y = height;
            line.baseline += height;
            let free = (content_width - line.x - line.width).max(0.);
            line.x += match style.align {
                FlowTextAlign::Left => 0.,
                FlowTextAlign::Center => free / 2.,
                FlowTextAlign::Right => free,
            };
            height += line.height;
            width = width.max(line.width);
            if [
                line.x,
                line.y,
                line.width,
                line.height,
                line.baseline,
                height,
            ]
            .iter()
            .any(|v| !v.is_finite())
            {
                return Err(OfdError::Invalid(
                    "flowed text measurements overflow".into(),
                ));
            }
        }
        Ok(PreparedFlowText {
            metrics: FlowTextMetrics {
                width,
                height,
                lines,
            },
            preparation,
            lines: flow_lines,
        })
    }

    pub(super) fn prepare_flow_spans(
        &self,
        document_index: usize,
        spans: &[FlowSpan],
        style: &FlowTextStyle,
        content_width: f64,
    ) -> Result<PreparedSpans, OfdError> {
        if spans.is_empty()
            || spans
                .iter()
                .all(|span| span.text.is_empty() && span.placeholder_width.is_none())
        {
            return Err(OfdError::Invalid("flowed spans are empty".into()));
        }
        if !style.shape
            && (style.language.is_some() || spans.iter().any(|span| span.language.is_some()))
        {
            return Err(OfdError::Invalid(
                "flowed span language requires shaping".into(),
            ));
        }
        if [
            content_width,
            style.font_size,
            style.line_height,
            style.paragraph_gap,
            style.first_line_indent,
            style.rest_line_indent,
        ]
        .iter()
        .any(|v| !v.is_finite())
            || content_width <= 0.
            || style.font_size <= 0.
            || style.line_height < style.font_size
            || style.paragraph_gap < 0.
            || style.first_line_indent < 0.
            || style.rest_line_indent < 0.
            || style.first_line_indent >= content_width
            || style.rest_line_indent >= content_width
        {
            return Err(OfdError::Invalid("flowed text geometry is invalid".into()));
        }
        let default_language = style
            .language
            .as_deref()
            .map(parse_shaping_language)
            .transpose()?;
        if document_index >= self.doc_roots()?.len() {
            return Err(OfdError::Invalid("document index is out of bounds".into()));
        }
        let fonts = self.fonts()?;
        let mut prepared = Vec::with_capacity(spans.len());
        for span in spans {
            let language = span
                .language
                .as_deref()
                .map(parse_shaping_language)
                .transpose()?
                .or_else(|| default_language.clone());
            let font_size = span.font_size.unwrap_or(style.font_size);
            if !font_size.is_finite() || font_size <= 0.0 {
                return Err(OfdError::Invalid("flowed span font size is invalid".into()));
            }
            if !span.letter_spacing.is_finite()
                || !span.underline_offset.is_finite()
                || !span.underline_width.is_finite()
                || span.underline_width < 0.0
                || (span.underline && !(font_size + span.underline_offset).is_finite())
                || span
                    .stroke_width
                    .is_some_and(|width| !width.is_finite() || width <= 0.0)
                || span
                    .weight
                    .is_some_and(|weight| !(100..=900).contains(&weight) || weight % 100 != 0)
            {
                return Err(OfdError::Invalid("flowed span style is invalid".into()));
            }
            if let Some(width) = span.placeholder_width {
                if !span.text.is_empty() || !width.is_finite() || width < 0. {
                    return Err(OfdError::Invalid(
                        "inline placeholder geometry is invalid".into(),
                    ));
                }
            }
            let (font_id, metrics) = if span.placeholder_width.is_some() {
                (String::new(), FlowFontMetrics::default())
            } else {
                let selected = span.font_id.as_ref().or(style.font_id.as_ref());
                let font = fonts
                    .iter()
                    .find(|font| {
                        font.document_index == document_index
                            && selected
                                .is_none_or(|id| resource_id_key(&font.id) == resource_id_key(id))
                    })
                    .ok_or_else(|| OfdError::Invalid("flowed span font is not defined".into()))?;
                let metrics = flow_font_metrics(self, font, &span.text)?;
                (font.id.clone(), metrics)
            };
            if !style.shape
                && span
                    .text
                    .chars()
                    .filter(|ch| !is_flow_hard_break(*ch))
                    .any(|ch| {
                        let advance = metrics.advance(ch);
                        let width = advance * font_size + span.letter_spacing;
                        // Combining glyphs may have zero advance; tracking must not
                        // cancel the advance of an otherwise advancing glyph.
                        !width.is_finite() || width < 0.0 || (width == 0.0 && advance != 0.0)
                    })
            {
                return Err(OfdError::Invalid(
                    "flowed span glyph advance is invalid".into(),
                ));
            }
            prepared.push(PreparedFlowSpan {
                font_id,
                placeholder_width: span.placeholder_width,
                font_size,
                language,
                fill_color: span.fill_color,
                fill: span.fill,
                stroke: span.stroke,
                stroke_color: span.stroke_color,
                stroke_width: span.stroke_width,
                bold: span.bold,
                weight: span.weight,
                italic: span.italic,
                letter_spacing: span.letter_spacing,
                underline: span.underline,
                underline_offset: span.underline_offset,
                underline_width: span.underline_width,
                keep_together: span.keep_together || span.placeholder_width.is_some(),
                metrics,
            });
        }
        let first_span_index = spans
            .iter()
            .position(|span| !span.text.is_empty() || span.placeholder_width.is_some())
            .expect("nonempty span was checked");
        let first_span = &prepared[first_span_index];
        let first_line_indent = effective_flow_indent(
            style,
            first_span.font_size,
            first_span.letter_spacing,
            content_width,
        )?;
        let leading_placeholder =
            prepared[first_span_index].placeholder_width.is_some() && first_line_indent > 0.;
        if leading_placeholder {
            prepared[first_span_index].placeholder_width = Some(first_line_indent);
        }
        if prepared.iter().any(|span| {
            span.placeholder_width
                .is_some_and(|width| width > content_width)
        }) {
            return Err(OfdError::Invalid(
                "inline placeholder exceeds available line width".into(),
            ));
        }
        Ok(PreparedSpans {
            spans: prepared,
            fonts,
            first_line_indent,
            leading_placeholder,
        })
    }

    pub(super) fn prepare_shaped_flow_lines(
        &self,
        document_index: usize,
        spans: &[FlowSpan],
        style: &FlowTextStyle,
        content_width: f64,
        preparation: &PreparedSpans,
        validate_glyphs: bool,
    ) -> Result<ShapedLines, OfdError> {
        let prepared = &preparation.spans;
        let fonts = &preparation.fonts;

        let faces = prepared
            .iter()
            .map(|span| {
                if span.placeholder_width.is_some() {
                    return Ok(None);
                }
                let font = fonts
                    .iter()
                    .find(|font| {
                        font.document_index == document_index
                            && resource_id_key(&font.id) == resource_id_key(&span.font_id)
                    })
                    .ok_or_else(|| OfdError::Invalid("flowed span font is not defined".into()))?;
                let path = font.embedded_path.as_deref().ok_or_else(|| {
                    OfdError::Invalid("shaped flow requires an embedded font".into())
                })?;
                rustybuzz::Face::from_slice(self.required_entry(path)?, 0)
                    .map(Some)
                    .ok_or_else(|| OfdError::Invalid("shaped flow font is invalid".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let paragraphs = flow_styled_paragraph_chars(spans)
            .into_iter()
            .map(|characters| {
                let mut index = 0;
                let mut units = Vec::new();
                let mut kept_ranges = HashMap::<usize, (usize, usize)>::new();
                for next in flow_grapheme_ends(
                    characters
                        .iter()
                        .map(|&(ch, span)| (ch, prepared[span].placeholder_width.is_some())),
                ) {
                    let unit_index = units.len();
                    for &(_, span_index) in &characters[index..next] {
                        if prepared[span_index].keep_together {
                            kept_ranges
                                .entry(span_index)
                                .and_modify(|(_, last)| *last = unit_index)
                                .or_insert((unit_index, unit_index));
                        }
                    }
                    units.push(ShapedFlowUnit {
                        text: characters[index..next].iter().map(|(ch, _)| *ch).collect(),
                        span_index: characters[index].1,
                        keep_until: unit_index + 1,
                    });
                    index = next;
                }
                for (first, last) in kept_ranges.into_values() {
                    units[first].keep_until = units[first].keep_until.max(last + 1);
                }
                units
            })
            .collect::<Vec<_>>();
        let indents = preparation.indents(style);
        let mut lines = Vec::<ShapedStyledLine>::new();
        for (paragraph_index, paragraph) in paragraphs.iter().enumerate() {
            let paragraph_line_start = lines.len();
            if paragraph.is_empty() {
                lines.push(ShapedStyledLine {
                    units: Vec::new(),
                    width: 0.0,
                    paragraph_start: paragraph_index > 0,
                    indent: indents.for_paragraph_line(
                        paragraph_index,
                        paragraph_index == 0 || style.indent_each_paragraph,
                    ),
                });
                continue;
            }
            let content = paragraph
                .iter()
                .map(|unit| unit.text.as_str())
                .collect::<String>();
            let break_offsets = unicode_linebreak::linebreaks(&content)
                .map(|(offset, _)| offset)
                .collect::<Vec<_>>();
            let mut unit_end_bytes = Vec::with_capacity(paragraph.len() + 1);
            unit_end_bytes.push(0);
            for unit in paragraph {
                unit_end_bytes.push(unit_end_bytes.last().copied().unwrap() + unit.text.len());
            }
            let mut start = 0;
            let mut first_line = paragraph_index == 0 || style.indent_each_paragraph;
            while start < paragraph.len() {
                let indent = indents.for_paragraph_line(paragraph_index, first_line);
                let available = content_width - indent;
                let mut end = start;
                let mut width = 0.0;
                let mut last_break = None;
                let mut has_visible = false;
                while end < paragraph.len() {
                    let mut next = end + 1;
                    let mut cursor = end;
                    while cursor < next {
                        next = next.max(paragraph[cursor].keep_until);
                        cursor += 1;
                    }
                    let run_width = shaped_styled_width(&paragraph[end..next], prepared, &faces)?;
                    if run_width > content_width + 1e-9 {
                        return Err(OfdError::Invalid(
                            "shaped flowed span exceeds available line width".into(),
                        ));
                    }
                    let candidate = shaped_styled_width(&paragraph[start..next], prepared, &faces)?;
                    if candidate > available + 1e-9 {
                        if let Some((break_end, break_width)) = last_break {
                            end = break_end;
                            width = break_width;
                        }
                        break;
                    }
                    has_visible |= paragraph[end..next]
                        .iter()
                        .any(|unit| unit.text.chars().any(|ch| !ch.is_whitespace()));
                    end = next;
                    width = candidate;
                    if has_visible && break_offsets.binary_search(&unit_end_bytes[end]).is_ok() {
                        last_break = Some((end, width));
                    }
                }
                if end == start {
                    if first_line {
                        lines.push(ShapedStyledLine {
                            units: Vec::new(),
                            width: 0.0,
                            indent,
                            paragraph_start: paragraph_index > 0
                                && lines.len() == paragraph_line_start,
                        });
                        first_line = false;
                        continue;
                    }
                    return Err(OfdError::Invalid(
                        "shaped flowed span exceeds available line width".into(),
                    ));
                }
                lines.push(ShapedStyledLine {
                    units: paragraph[start..end].to_vec(),
                    width,
                    indent,
                    paragraph_start: paragraph_index > 0 && lines.len() == paragraph_line_start,
                });
                start = end;
                first_line = false;
            }
        }
        if lines.is_empty() {
            return Err(OfdError::Invalid(
                "flowed spans have no rendered lines".into(),
            ));
        }
        let group_widths = lines
            .iter()
            .map(|line| {
                line.units
                    .chunk_by(|a, b| a.span_index == b.span_index)
                    .map(|group| shaped_styled_width(group, prepared, &faces))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        if validate_glyphs {
            for line in &lines {
                for group in line.units.chunk_by(|a, b| a.span_index == b.span_index) {
                    let index = group[0].span_index;
                    let span = &prepared[index];
                    if span.placeholder_width.is_some() {
                        continue;
                    }
                    let text = group
                        .iter()
                        .map(|unit| unit.text.as_str())
                        .collect::<String>();
                    prepare_positioned_shaping(
                        faces[index].as_ref().ok_or_else(|| {
                            OfdError::Invalid("shaped flow font is not defined".into())
                        })?,
                        &text,
                        span.font_size,
                        span.language.as_ref(),
                        span.letter_spacing,
                    )?;
                }
            }
        }
        drop(faces);
        Ok(ShapedLines {
            lines,
            group_widths,
        })
    }
}

pub(super) struct PositionedShaping {
    pub(super) transforms: String,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) delta_x: String,
    pub(super) delta_y: String,
    pub(super) zero_advance_ink: Option<[f64; 4]>,
}

pub(super) fn include_zero_advance_ink(
    face: &rustybuzz::Face<'_>,
    glyph: u16,
    position: [f64; 2],
    scale: f64,
    ink: &mut Option<[f64; 4]>,
) -> Result<(), OfdError> {
    if let Some(bounds) = face.glyph_bounding_box(ttf_parser::GlyphId(glyph)) {
        let [x, y] = position;
        let bounds = [
            x + f64::from(bounds.x_min) * scale,
            y - f64::from(bounds.y_max) * scale,
            x + f64::from(bounds.x_max) * scale,
            y - f64::from(bounds.y_min) * scale,
        ];
        if bounds.iter().any(|value| !value.is_finite()) {
            return Err(OfdError::Invalid("shaped glyph ink bounds overflow".into()));
        }
        *ink = Some(match *ink {
            None => bounds,
            Some([l, t, r, b]) => [
                l.min(bounds[0]),
                t.min(bounds[1]),
                r.max(bounds[2]),
                b.max(bounds[3]),
            ],
        });
    }
    Ok(())
}

pub(super) fn prepare_positioned_shaping(
    face: &rustybuzz::Face<'_>,
    text: &str,
    size: f64,
    language: Option<&rustybuzz::Language>,
    tracking: f64,
) -> Result<PositionedShaping, OfdError> {
    Ok(if visual_shape_runs(text)?.len() > 1 {
        shape_bidirectional_text(face, text, size, language, tracking)?
    } else {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        if let Some(language) = language {
            buffer.set_language(language.clone());
        }
        let direction = buffer.direction();
        if direction != rustybuzz::Direction::LeftToRight
            && direction != rustybuzz::Direction::RightToLeft
        {
            return Err(OfdError::Invalid(
                "shaped text direction is unsupported".into(),
            ));
        }
        let shaped = rustybuzz::shape(face, &[], buffer);
        let glyphs = shaped.glyph_infos();
        let positions = shaped.glyph_positions();
        if glyphs.is_empty() || glyphs.len() > 10_000 {
            return Err(OfdError::Invalid(
                "shaped text glyph mapping is invalid".into(),
            ));
        }
        let mut transforms = String::new();
        if direction == rustybuzz::Direction::RightToLeft {
            let mapped = glyphs
                .iter()
                .map(|glyph| {
                    u16::try_from(glyph.glyph_id)
                        .ok()
                        .filter(|id| *id != 0)
                        .map(|id| id.to_string())
                        .ok_or_else(|| OfdError::Invalid("shaped text glyph ID is invalid".into()))
                })
                .collect::<Result<Vec<_>, _>>()?
                .join(" ");
            let code_count = text.encode_utf16().count();
            transforms.push_str(&format!("<ofd:CGTransform CodePosition=\"0\" CodeCount=\"{code_count}\" GlyphCount=\"{}\"><ofd:Glyphs>{mapped}</ofd:Glyphs></ofd:CGTransform>", glyphs.len()));
        } else {
            if glyphs[0].cluster != 0 {
                return Err(OfdError::Invalid(
                    "shaped text glyph mapping is invalid".into(),
                ));
            }
            let mut code_position = 0_usize;
            let mut start = 0_usize;
            while start < glyphs.len() {
                let cluster = glyphs[start].cluster as usize;
                let mut end = start + 1;
                while end < glyphs.len() && glyphs[end].cluster == glyphs[start].cluster {
                    end += 1;
                }
                let next_cluster = if end < glyphs.len() {
                    glyphs[end].cluster as usize
                } else {
                    text.len()
                };
                let code_count = text
                    .get(cluster..next_cluster)
                    .filter(|segment| !segment.is_empty())
                    .ok_or_else(|| OfdError::Invalid("shaped text cluster is invalid".into()))?
                    .encode_utf16()
                    .count();
                let mapped = glyphs[start..end]
                    .iter()
                    .map(|glyph| {
                        u16::try_from(glyph.glyph_id)
                            .ok()
                            .filter(|id| *id != 0)
                            .map(|id| id.to_string())
                            .ok_or_else(|| {
                                OfdError::Invalid("shaped text glyph ID is invalid".into())
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .join(" ");
                transforms.push_str(&format!("<ofd:CGTransform CodePosition=\"{code_position}\" CodeCount=\"{code_count}\" GlyphCount=\"{}\"><ofd:Glyphs>{mapped}</ofd:Glyphs></ofd:CGTransform>", end - start));
                code_position += code_count;
                start = end;
            }
            if code_position != text.encode_utf16().count() {
                return Err(OfdError::Invalid(
                    "shaped text code mapping is invalid".into(),
                ));
            }
        }
        let scale = size / f64::from(face.units_per_em());
        let mut zero_advance_ink = None;
        let mut pen_x = 0_i64;
        let mut pen_y = 0_i64;
        let mut glyph_positions = Vec::with_capacity(positions.len());
        for (index, position) in positions.iter().enumerate() {
            let x = (pen_x + i64::from(position.x_offset)) as f64 * scale;
            let y = size - (pen_y + i64::from(position.y_offset)) as f64 * scale;
            if !x.is_finite() || !y.is_finite() {
                return Err(OfdError::Invalid("shaped text position is invalid".into()));
            }
            if position.x_advance == 0 {
                let id = u16::try_from(glyphs[index].glyph_id)
                    .map_err(|_| OfdError::Invalid("shaped glyph ID is invalid".into()))?;
                include_zero_advance_ink(
                    face,
                    id,
                    [x + index as f64 * tracking, y],
                    scale,
                    &mut zero_advance_ink,
                )?;
            }
            glyph_positions.push((x, y));
            pen_x += i64::from(position.x_advance);
            pen_y += i64::from(position.y_advance);
        }
        let delta_x = glyph_positions
            .windows(2)
            .map(|pair| (pair[1].0 - pair[0].0).to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let delta_y = glyph_positions
            .windows(2)
            .map(|pair| (pair[1].1 - pair[0].1).to_string())
            .collect::<Vec<_>>()
            .join(" ");
        PositionedShaping {
            transforms,
            x: glyph_positions[0].0,
            y: glyph_positions[0].1,
            delta_x,
            delta_y,
            zero_advance_ink,
        }
    })
}
