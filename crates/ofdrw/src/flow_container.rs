use super::*;

/// A vertical sequence of text, images, spacing and nested decorated boxes.
///
/// Content height is automatic; `layout.size[0]` is the requested width.
/// Root containers support ordinary box floating rows. Nested containers are
/// vertical children; their float direction controls horizontal alignment and
/// their clear rule must be `Both`. Internal floating rows are separate work.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowContainer {
    pub layout: LayoutBox,
    pub children: Vec<FlowContainerChild>,
    pub gap: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FlowContainerChild {
    Paragraph {
        spans: Vec<FlowSpan>,
        style: Box<FlowTextStyle>,
    },
    Image {
        image: TableCellImage,
        align: FlowTextAlign,
    },
    Container(Box<FlowContainer>),
    Spacer {
        height: f64,
    },
}

struct PreparedContainer<'a> {
    layout: LayoutBox,
    children: Vec<PlacedChild<'a>>,
    ranges: Vec<[f64; 2]>,
}

struct PlacedChild<'a> {
    id: usize,
    x: f64,
    y: f64,
    node: PreparedChild<'a>,
}

enum PreparedChild<'a> {
    Text(text_measurement::PreparedFlowText),
    Image(&'a TableCellImage),
    Container(Box<PreparedContainer<'a>>),
}

fn expected_units(b: &PreparedContainer<'_>) -> usize {
    if b.layout.placeholder {
        return 0;
    }
    b.children
        .iter()
        .map(|child| match &child.node {
            PreparedChild::Text(flow) => flow.metrics.lines.len(),
            PreparedChild::Image(_) => 1,
            PreparedChild::Container(nested) => expected_units(nested),
        })
        .sum()
}

fn outer_height(b: &LayoutBox) -> f64 {
    b.size[1] + b.margin[0] + b.margin[2] + b.border[0] + b.border[2] + b.padding[0] + b.padding[2]
}

fn outer_width(b: &LayoutBox) -> f64 {
    b.size[0] + b.margin[1] + b.margin[3] + b.border[1] + b.border[3] + b.padding[1] + b.padding[3]
}

fn aligned_x(width: f64, child_width: f64, align: FlowTextAlign) -> f64 {
    match align {
        FlowTextAlign::Left => 0.,
        FlowTextAlign::Center => (width - child_width) / 2.,
        FlowTextAlign::Right => width - child_width,
    }
}

impl OfdDocument {
    /// Draw nested vertical mixed-content containers with automatic page cuts.
    ///
    /// Text lines, kept paragraphs, images and complete nested boxes remain
    /// indivisible. Nested decoration bands are consumed once across pages.
    /// Widths are clamped before text measurement. All changes are atomic.
    /// Up to 10,000 total nodes and eight nested container levels are accepted.
    /// Returned placements describe root containers; child objects use the
    /// corresponding nested coordinates. Nested floating rows are unsupported.
    ///
    /// ```
    /// use ofdrw::{BoxLayoutStyle, FlowContainer, FlowContainerChild, FlowSpan,
    ///     FlowTextStyle, LayoutBox, OfdDocument};
    /// let mut document = OfdDocument::new();
    /// let result = document.add_flow_containers(0, &[FlowContainer {
    ///     layout: LayoutBox { size: [40., 0.], padding: [1.; 4], ..Default::default() },
    ///     children: vec![FlowContainerChild::Paragraph {
    ///         spans: vec![FlowSpan::new("Hello")], style: Box::new(FlowTextStyle::default()),
    ///     }], gap: 1.,
    /// }], BoxLayoutStyle::default())?;
    /// assert_eq!(result.pages, [0]);
    /// assert_eq!(document.page_text(0)?, ["Hello"]);
    /// # Ok::<(), ofdrw::OfdError>(())
    /// ```
    pub fn add_flow_containers(
        &mut self,
        document_index: usize,
        containers: &[FlowContainer],
        style: BoxLayoutStyle,
    ) -> Result<BoxLayoutResult, OfdError> {
        if containers.len() > 10_000 {
            return Err(OfdError::Limit("too many flow containers".into()));
        }
        let available_width = style.page_size[0] - style.margins[1] - style.margins[3];
        let mut count = 0;
        let prepared = containers
            .iter()
            .map(|b| self.prepare_container(document_index, b, available_width, 0, &mut count))
            .collect::<Result<Vec<_>, _>>()?;
        let items = prepared
            .iter()
            .map(|b| BoxLayoutItem::Box(b.layout.clone()))
            .collect::<Vec<_>>();
        let ranges = prepared
            .iter()
            .enumerate()
            .map(|(source_index, b)| BoxContentRanges {
                source_index,
                ranges: b.ranges.clone(),
            })
            .collect::<Vec<_>>();
        let mut staged = self.clone();
        let result =
            staged.add_box_layout_with_content_ranges(document_index, &items, &ranges, style)?;
        let mut drawn = HashSet::new();
        for placement in &result.placements {
            staged.draw_container_children(
                &prepared[placement.source_index],
                placement,
                &mut drawn,
            )?;
        }
        if drawn.len() != prepared.iter().map(expected_units).sum::<usize>() {
            return Err(OfdError::Invalid(
                "flow container content was not completely placed".into(),
            ));
        }
        *self = staged;
        Ok(result)
    }

    fn prepare_container<'a>(
        &self,
        document_index: usize,
        b: &'a FlowContainer,
        available: f64,
        depth: usize,
        count: &mut usize,
    ) -> Result<PreparedContainer<'a>, OfdError> {
        *count += 1;
        if depth >= 8 || *count > 10_000 {
            return Err(OfdError::Limit(
                "flow container depth or node limit exceeded".into(),
            ));
        }
        if !b.gap.is_finite()
            || b.gap < 0.
            || !b.layout.size[0].is_finite()
            || b.layout.size[0] <= 0.
            || b.layout
                .margin
                .into_iter()
                .chain(b.layout.border)
                .chain(b.layout.padding)
                .any(|v| !v.is_finite() || v < 0.)
            || b.layout.offset.iter().any(|v| !v.is_finite())
        {
            return Err(OfdError::Invalid(
                "flow container geometry is invalid".into(),
            ));
        }
        if depth > 0 && b.layout.clear != BoxClear::Both {
            return Err(OfdError::Invalid(
                "nested vertical containers require clear Both".into(),
            ));
        }
        let mut layout = b.layout.clone();
        let horizontal = layout.margin[1]
            + layout.margin[3]
            + layout.border[1]
            + layout.border[3]
            + layout.padding[1]
            + layout.padding[3];
        layout.size[0] = layout.size[0].min(available - horizontal);
        let width = layout.size[0];
        if !width.is_finite() || width <= 0. {
            return Err(OfdError::Invalid(
                "flow container has no content width".into(),
            ));
        }
        let mut children = Vec::new();
        let mut ranges = Vec::new();
        let mut y = 0.;
        for (index, child) in b.children.iter().enumerate() {
            *count += 1;
            if *count > 10_000 {
                return Err(OfdError::Limit("too many flow container children".into()));
            }
            if index != 0 {
                y += b.gap;
            }
            let id = *count;
            let (x, height, node, protected) = match child {
                FlowContainerChild::Paragraph { spans, style } => {
                    let flow =
                        self.prepare_measured_flow_text(document_index, spans, width, style)?;
                    let mut protected = BoxContentRanges::from_text(0, &flow.metrics).ranges;
                    if style.keep_paragraphs_together {
                        let mut groups: Vec<[f64; 2]> = Vec::new();
                        for line in &flow.metrics.lines {
                            if groups.is_empty() || line.paragraph_start {
                                groups.push([line.y, line.y + line.height]);
                            } else if let Some(last) = groups.last_mut() {
                                last[1] = line.y + line.height;
                            }
                        }
                        protected = groups;
                    }
                    (
                        0.,
                        flow.metrics.height,
                        Some(PreparedChild::Text(flow)),
                        protected,
                    )
                }
                FlowContainerChild::Image { image, align } => {
                    if [image.width, image.height]
                        .iter()
                        .any(|v| !v.is_finite() || *v <= 0.)
                        || image.width > width
                    {
                        return Err(OfdError::Invalid(
                            "flow container image does not fit its width".into(),
                        ));
                    }
                    checked_image_format(&image.data, &image.format)?;
                    (
                        aligned_x(width, image.width, *align),
                        image.height,
                        Some(PreparedChild::Image(image)),
                        vec![[0., image.height]],
                    )
                }
                FlowContainerChild::Container(child) => {
                    let prepared =
                        self.prepare_container(document_index, child, width, depth + 1, count)?;
                    let height = outer_height(&prepared.layout);
                    let x = aligned_x(
                        width,
                        outer_width(&prepared.layout),
                        match prepared.layout.float {
                            BoxFloat::Left => FlowTextAlign::Left,
                            BoxFloat::Right => FlowTextAlign::Right,
                            BoxFloat::Center => FlowTextAlign::Center,
                        },
                    );
                    let top = prepared.layout.margin[0]
                        + prepared.layout.border[0]
                        + prepared.layout.padding[0];
                    let protected = if prepared.layout.integrity && height > 0. {
                        vec![[0., height]]
                    } else {
                        prepared
                            .ranges
                            .iter()
                            .map(|[start, end]| [start + top, end + top])
                            .collect()
                    };
                    (
                        x,
                        height,
                        Some(PreparedChild::Container(Box::new(prepared))),
                        protected,
                    )
                }
                FlowContainerChild::Spacer { height } => {
                    if !height.is_finite() || *height < 0. {
                        return Err(OfdError::Invalid("flow container spacer is invalid".into()));
                    }
                    (0., *height, None, Vec::new())
                }
            };
            ranges.extend(
                protected
                    .into_iter()
                    .map(|[start, end]| [start + y, end + y]),
            );
            if let Some(node) = node {
                children.push(PlacedChild { id, x, y, node });
            }
            y += height;
            if !y.is_finite() {
                return Err(OfdError::Invalid("flow container height overflows".into()));
            }
        }
        layout.size[1] = y;
        Ok(PreparedContainer {
            layout,
            children,
            ranges,
        })
    }

    fn draw_container_children(
        &mut self,
        b: &PreparedContainer<'_>,
        p: &BoxPlacement,
        drawn: &mut HashSet<(usize, usize)>,
    ) -> Result<(), OfdError> {
        if b.layout.placeholder || p.content_box[3] == 0. {
            return Ok(());
        }
        let top = b.layout.margin[0] + b.layout.border[0] + b.layout.padding[0];
        let start = (p.source_offset - top).max(0.);
        let end = start + p.content_box[3];
        for child in &b.children {
            match &child.node {
                PreparedChild::Text(flow) => {
                    for (index, line) in flow.metrics.lines.iter().enumerate() {
                        let y = child.y + line.y;
                        let tolerance = (y + line.height).abs().max(1.) * 1e-12;
                        if y + tolerance < start || y + line.height > end + tolerance {
                            continue;
                        }
                        if !drawn.insert((child.id, index)) {
                            return Err(OfdError::Invalid(
                                "flow container line was placed more than once".into(),
                            ));
                        }
                        let draw = text_box::FlowLineDraw {
                            page_index: p.page_index,
                            x: p.content_box[0] + child.x + line.x,
                            y: p.content_box[1] + y - start,
                            height: line.height,
                            baseline: line.baseline - line.y,
                        };
                        match &flow.lines {
                            text_measurement::PreparedFlowLines::Plain(lines) => self
                                .draw_plain_flow_line(
                                    &lines[index],
                                    &flow.preparation.spans,
                                    draw,
                                )?,
                            text_measurement::PreparedFlowLines::Shaped(lines) => self
                                .draw_shaped_flow_line(
                                    &lines.lines[index],
                                    &lines.group_widths[index],
                                    &flow.preparation.spans,
                                    draw,
                                )?,
                        }
                    }
                }
                PreparedChild::Image(image) => {
                    if child.y >= start && child.y + image.height <= end + 1e-9 {
                        if !drawn.insert((child.id, 0)) {
                            return Err(OfdError::Invalid(
                                "flow container image was placed more than once".into(),
                            ));
                        }
                        self.add_image_object(
                            p.page_index,
                            &image.data,
                            &image.format,
                            [
                                p.content_box[0] + child.x,
                                p.content_box[1] + child.y - start,
                                image.width,
                                image.height,
                            ],
                        )?;
                    }
                }
                PreparedChild::Container(nested) => {
                    let from = start.max(child.y);
                    let to = end.min(child.y + outer_height(&nested.layout));
                    if to <= from {
                        continue;
                    }
                    let placement = box_layout::nested_fragment(
                        &nested.layout,
                        p.page_index,
                        p.content_box[0] + child.x,
                        p.content_box[1] + from - start,
                        from - child.y,
                        to - from,
                    )?;
                    if !nested.layout.placeholder {
                        self.draw_box_fragment(&nested.layout, &placement)?;
                    }
                    self.draw_container_children(nested, &placement, drawn)?;
                }
            }
        }
        Ok(())
    }
}
