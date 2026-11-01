use super::*;

/// A styled text box whose content height is measured automatically.
///
/// The layout size's first component is the requested content width. Its
/// second component is replaced by measured text height. Typography supplies
/// font selection and wrapping; page geometry and continuation come from the
/// shared `BoxLayoutStyle`. Decorations and floating rules use `LayoutBox`.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBox {
    pub layout: LayoutBox,
    pub spans: Vec<FlowSpan>,
    pub style: FlowTextStyle,
}

pub(super) struct FlowLineDraw {
    pub(super) page_index: usize,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) height: f64,
    pub(super) baseline: f64,
}

impl OfdDocument {
    /// Measure, paginate and draw styled text boxes atomically.
    ///
    /// Each line, including its leading, remains on a single page. Paragraph
    /// keep rules protect complete paragraphs instead. Widths are clamped
    /// before wrapping, and decorations precede text painting. Floating columns
    /// retain independent continuation positions. All font, shaping and span
    /// decoration behavior is shared with `add_flowed_spans`.
    ///
    /// The returned fragments describe measured text height and decorations;
    /// glyph ink or underline overhang may exceed logical line bounds. This
    /// API accepts text children only; it does not create arbitrary nested DOM
    /// containers. Text style page geometry and start positions are ignored.
    ///
    /// ```
    /// use ofdrw::{BoxLayoutStyle, FlowSpan, FlowTextStyle, LayoutBox, OfdDocument, TextBox};
    /// let mut document = OfdDocument::new();
    /// let result = document.add_text_boxes(0, &[TextBox {
    ///     layout: LayoutBox { size: [40., 0.], padding: [2.; 4],
    ///         border: [0.2; 4], ..Default::default() },
    ///     spans: vec![FlowSpan::new("A measured text box")],
    ///     style: FlowTextStyle::default(),
    /// }], BoxLayoutStyle::default())?;
    /// assert_eq!(result.pages, [0]);
    /// assert!(!document.page_text(0)?.is_empty());
    /// # Ok::<(), ofdrw::OfdError>(())
    /// ```
    pub fn add_text_boxes(
        &mut self,
        document_index: usize,
        boxes: &[TextBox],
        style: BoxLayoutStyle,
    ) -> Result<BoxLayoutResult, OfdError> {
        if boxes.len() > 10_000 {
            return Err(OfdError::Limit("too many text boxes".into()));
        }
        let mut prepared = Vec::with_capacity(boxes.len());
        let mut items = Vec::with_capacity(boxes.len());
        let mut ranges = Vec::with_capacity(boxes.len());
        for (index, b) in boxes.iter().enumerate() {
            if !b.layout.size[0].is_finite() || b.layout.size[0] <= 0. {
                return Err(OfdError::Invalid(
                    "text box width must be positive and finite".into(),
                ));
            }
            let horizontal: f64 = [1, 3]
                .into_iter()
                .map(|side| b.layout.margin[side] + b.layout.border[side] + b.layout.padding[side])
                .sum();
            let width = b.layout.size[0]
                .min(style.page_size[0] - style.margins[1] - style.margins[3] - horizontal);
            let flow =
                self.prepare_measured_flow_text(document_index, &b.spans, width, &b.style)?;
            let metrics = &flow.metrics;
            let mut policy = BoxContentRanges::from_text(index, metrics);
            if b.style.keep_paragraphs_together {
                let mut paragraphs: Vec<[f64; 2]> = Vec::new();
                for line in &metrics.lines {
                    if paragraphs.is_empty() || line.paragraph_start {
                        paragraphs.push([line.y, line.y + line.height]);
                    } else if let Some(last) = paragraphs.last_mut() {
                        last[1] = line.y + line.height;
                    }
                }
                policy.ranges = paragraphs;
            }
            let mut layout = b.layout.clone();
            layout.size = [width, metrics.height];
            items.push(BoxLayoutItem::Box(layout));
            ranges.push(policy);
            prepared.push(flow);
        }
        let mut staged = self.clone();
        let result =
            staged.add_box_layout_with_content_ranges(document_index, &items, &ranges, style)?;
        let mut placements = vec![Vec::new(); boxes.len()];
        for placement in &result.placements {
            placements[placement.source_index].push(placement);
        }
        for (index, b) in boxes.iter().enumerate() {
            if b.layout.placeholder {
                continue;
            }
            let p = &prepared[index];
            let content_top = b.layout.margin[0] + b.layout.border[0] + b.layout.padding[0];
            let mut fragments = placements[index].iter();
            let mut fragment = fragments.next();
            for (line_index, line) in p.metrics.lines.iter().enumerate() {
                let end = line.y + line.height;
                let tolerance = end.abs().max(1.) * 1e-12;
                let (placement, content_offset) = loop {
                    let placement = fragment
                        .ok_or_else(|| OfdError::Invalid("text line has no box fragment".into()))?;
                    let offset = (placement.source_offset - content_top).max(0.);
                    if placement.content_box[3] > 0.
                        && end <= offset + placement.content_box[3] + tolerance
                    {
                        if line.y + tolerance < offset {
                            return Err(OfdError::Invalid(
                                "text line crosses a box fragment".into(),
                            ));
                        }
                        break (*placement, offset);
                    }
                    fragment = fragments.next();
                };
                let draw = FlowLineDraw {
                    page_index: placement.page_index,
                    x: placement.content_box[0] + line.x,
                    y: placement.content_box[1] + line.y - content_offset,
                    height: line.height,
                    baseline: line.baseline - line.y,
                };
                match &p.lines {
                    text_measurement::PreparedFlowLines::Plain(lines) => staged
                        .draw_plain_flow_line(&lines[line_index], &p.preparation.spans, draw)?,
                    text_measurement::PreparedFlowLines::Shaped(shaped) => staged
                        .draw_shaped_flow_line(
                            &shaped.lines[line_index],
                            &shaped.group_widths[line_index],
                            &p.preparation.spans,
                            draw,
                        )?,
                }
            }
        }
        *self = staged;
        Ok(result)
    }

    pub(super) fn draw_plain_flow_line(
        &mut self,
        line: &StyledFlowLine,
        prepared: &[PreparedFlowSpan],
        draw: FlowLineDraw,
    ) -> Result<(), OfdError> {
        let FlowLineDraw {
            page_index,
            mut x,
            y,
            height: line_height,
            baseline,
        } = draw;
        for group in line.glyphs.chunk_by(|a, b| a.span_index == b.span_index) {
            let span = &prepared[group[0].span_index];
            let text: String = group.iter().map(|glyph| glyph.ch).collect();
            let width: f64 = group.iter().map(|glyph| glyph.width).sum();
            self.add_flow_span_object(
                page_index,
                &text,
                [x, y, width.max(0.001), line_height],
                baseline,
                span,
                group,
            )?;
            if span.underline {
                let offset = baseline + span.underline_offset;
                let line_width = if span.underline_width == 0.0 {
                    span.font_size * 0.05
                } else {
                    span.underline_width
                };
                self.add_path_object(
                    page_index,
                    &format!("M 0 {offset} L {width} {offset}"),
                    [x, y, width.max(0.001), line_height],
                    PathStyle {
                        line_width,
                        stroke_color: span.fill_color,
                        ..PathStyle::default()
                    },
                )?;
            }
            x += width;
        }
        Ok(())
    }

    pub(super) fn draw_shaped_flow_line(
        &mut self,
        line: &ShapedStyledLine,
        widths: &[f64],
        prepared: &[PreparedFlowSpan],
        draw: FlowLineDraw,
    ) -> Result<(), OfdError> {
        let FlowLineDraw {
            page_index,
            mut x,
            y,
            height: line_height,
            baseline,
        } = draw;
        for (group, width) in line
            .units
            .chunk_by(|a, b| a.span_index == b.span_index)
            .zip(widths)
        {
            let span = &prepared[group[0].span_index];
            let text = group
                .iter()
                .map(|unit| unit.text.as_str())
                .collect::<String>();
            let width = *width;
            self.add_shaped_text_object_with_style(
                page_index,
                &text,
                [x, y, width.max(0.001), line_height],
                span.font_size,
                &span.font_id,
                ShapingContext {
                    style: Some((baseline, span)),
                    language: span.language.as_ref(),
                },
            )?;
            if span.underline {
                let underline_y = baseline + span.underline_offset;
                let line_width = if span.underline_width == 0.0 {
                    span.font_size * 0.05
                } else {
                    span.underline_width
                };
                self.add_path_object(
                    page_index,
                    &format!("M 0 {underline_y} L {width} {underline_y}"),
                    [x, y, width.max(0.001), line_height],
                    PathStyle {
                        line_width,
                        stroke_color: span.fill_color,
                        ..PathStyle::default()
                    },
                )?;
            }
            x += width;
        }
        Ok(())
    }
}
