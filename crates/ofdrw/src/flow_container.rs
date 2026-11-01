use super::*;
mod pagination;

/// A vertical sequence of text, images, tables, spacing and nested decorated boxes.
///
/// Content height is automatic; `layout.size[0]` is the requested width.
/// Containers support Java-style floating rows at every nesting level.
/// Paragraph, image, spacer and table children occupy full rows; nested boxes
/// share rows according to their float direction, clear rules and widths.
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
    /// Rows split only between complete merged-row groups. Table geometry
    /// comes from `table`; `style` supplies text defaults and font selection.
    /// Repeated headers stay with the first complete merged body-row group.
    Table {
        table: Box<NestedTable>,
        style: Box<TableStyle>,
        align: FlowTextAlign,
    },
    Container(Box<FlowContainer>),
    Spacer {
        height: f64,
    },
    /// Finish a nonempty floating row; an empty row is ignored, like Java BR.
    LineBreak,
    /// Exhaust this page's remaining work area, like Java PageAreaFiller.
    /// The next content starts on a new page; trailing fills do not append one.
    PageAreaFill,
}

struct PreparedContainer<'a> {
    layout: LayoutBox,
    children: Vec<PlacedChild<'a>>,
    ranges: Vec<[f64; 2]>,
    gap: f64,
}

struct PlacedChild<'a> {
    id: usize,
    keep_paragraphs: bool,
    x: f64,
    y: f64,
    node: PreparedChild<'a>,
}

enum PreparedChild<'a> {
    Text(text_measurement::PreparedFlowText),
    Image(&'a TableCellImage),
    Spacer(f64),
    LineBreak,
    PageAreaFill,
    Table {
        prepared: Box<PreparedNestedTable<'a>>,
        style: &'a TableStyle,
        groups: Vec<TableGroup>,
    },
    Container(Box<PreparedContainer<'a>>),
}

struct TableGroup {
    rows: std::ops::Range<usize>,
    start: f64,
    end: f64,
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
            PreparedChild::Spacer(_) | PreparedChild::LineBreak | PreparedChild::PageAreaFill => 0,
            PreparedChild::Table { groups, .. } => groups.len(),
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
    /// corresponding nested coordinates. Columns continue independently; table
    /// headers repeat when requested, expanding actual laid-out content height.
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
        if prepared.iter().any(pagination::needs_dynamic) {
            return pagination::draw_containers(self, document_index, &prepared, style);
        }
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
        box_layout::validate_border_dash(b.layout.border_dash.as_deref())?;
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
        let mut gap_pending = false;
        for child in &b.children {
            *count += 1;
            if *count > 10_000 {
                return Err(OfdError::Limit("too many flow container children".into()));
            }
            let control = matches!(
                child,
                FlowContainerChild::LineBreak | FlowContainerChild::PageAreaFill
            );
            if gap_pending && !control {
                y += b.gap;
            }
            if matches!(child, FlowContainerChild::PageAreaFill) {
                gap_pending = false;
            } else if !control {
                gap_pending = true;
            }
            let id = *count;
            let keep_paragraphs = matches!(child, FlowContainerChild::Paragraph { style, .. } if style.keep_paragraphs_together);
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
                FlowContainerChild::Table {
                    table,
                    style,
                    align,
                } => {
                    let (font, metrics) =
                        self.container_table_fonts(document_index, table, style)?;
                    let prepared =
                        prepare_nested_table(table, width, style, &metrics, font.as_deref(), 0)?;
                    let mut row_end = (1..=table.row_count).collect::<Vec<_>>();
                    for cell in &table.cells {
                        row_end[cell.row] = row_end[cell.row].max(cell.row + cell.row_span);
                    }
                    let mut groups = Vec::new();
                    let mut row = 0;
                    let mut offset = 0.;
                    while row < table.row_count {
                        let mut stop = row_end[row];
                        let mut scan = row + 1;
                        while scan < stop {
                            stop = stop.max(row_end[scan]);
                            scan += 1;
                        }
                        let end = offset + prepared.row_heights[row..stop].iter().sum::<f64>();
                        if !end.is_finite() || end <= offset {
                            return Err(OfdError::Invalid(
                                "container table row group has invalid height".into(),
                            ));
                        }
                        groups.push(TableGroup {
                            rows: row..stop,
                            start: offset,
                            end,
                        });
                        offset = end;
                        row = stop;
                    }
                    let table_width = table.column_widths.iter().sum::<f64>();
                    let protected = groups
                        .iter()
                        .map(|group| [group.start, group.end])
                        .collect();
                    (
                        aligned_x(width, table_width, *align),
                        offset,
                        Some(PreparedChild::Table {
                            prepared: Box::new(prepared),
                            style,
                            groups,
                        }),
                        protected,
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
                FlowContainerChild::LineBreak => {
                    (0., 0., Some(PreparedChild::LineBreak), Vec::new())
                }
                FlowContainerChild::PageAreaFill => {
                    (0., 0., Some(PreparedChild::PageAreaFill), Vec::new())
                }
                FlowContainerChild::Spacer { height } => {
                    if !height.is_finite() || *height < 0. {
                        return Err(OfdError::Invalid("flow container spacer is invalid".into()));
                    }
                    (
                        0.,
                        *height,
                        Some(PreparedChild::Spacer(*height)),
                        Vec::new(),
                    )
                }
            };
            ranges.extend(
                protected
                    .into_iter()
                    .map(|[start, end]| [start + y, end + y]),
            );
            if let Some(node) = node {
                children.push(PlacedChild {
                    id,
                    keep_paragraphs,
                    x,
                    y,
                    node,
                });
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
            gap: b.gap,
        })
    }

    fn container_table_fonts(
        &self,
        document_index: usize,
        table: &NestedTable,
        style: &TableStyle,
    ) -> Result<(Option<String>, HashMap<String, FlowFontMetrics>), OfdError> {
        if !style.font_size.is_finite()
            || style.font_size <= 0.
            || !style.line_height.is_finite()
            || style.line_height < style.font_size
        {
            return Err(OfdError::Invalid(
                "container table typography is invalid".into(),
            ));
        }
        let mut parts = Vec::new();
        collect_table_text(&table.cells, &mut parts, 0)?;
        if parts.iter().all(|text| text.is_empty()) {
            return Ok((None, HashMap::new()));
        }
        let fonts = self.fonts()?;
        let default = fonts
            .iter()
            .find(|font| {
                font.document_index == document_index
                    && style
                        .font_id
                        .as_ref()
                        .is_none_or(|id| resource_id_key(id) == resource_id_key(&font.id))
            })
            .ok_or_else(|| OfdError::Invalid("container table font is not defined".into()))?;
        let mut ids = HashSet::new();
        ids.insert(default.id.clone());
        collect_table_font_ids(&table.cells, &mut ids);
        let all_text = parts.join("\n");
        let mut metrics = HashMap::new();
        for id in ids {
            let font = fonts
                .iter()
                .find(|font| {
                    font.document_index == document_index
                        && resource_id_key(&font.id) == resource_id_key(&id)
                })
                .ok_or_else(|| {
                    OfdError::Invalid(format!("container table font {id} is not defined"))
                })?;
            metrics.insert(
                resource_id_key(&id),
                flow_font_metrics(self, font, &all_text)?,
            );
        }
        Ok((Some(default.id.clone()), metrics))
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
                PreparedChild::Spacer(_)
                | PreparedChild::LineBreak
                | PreparedChild::PageAreaFill => {}
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
                PreparedChild::Table {
                    prepared,
                    style,
                    groups,
                } => {
                    for (index, group) in groups.iter().enumerate() {
                        let from = child.y + group.start;
                        let to = child.y + group.end;
                        let tolerance = to.abs().max(1.) * 1e-12;
                        if from + tolerance < start || to > end + tolerance {
                            continue;
                        }
                        if !drawn.insert((child.id, index)) {
                            return Err(OfdError::Invalid(
                                "container table group was placed more than once".into(),
                            ));
                        }
                        draw_nested_table_rows(
                            self,
                            p.page_index,
                            prepared,
                            p.content_box[0] + child.x,
                            p.content_box[1] + from - start,
                            style,
                            group.rows.clone(),
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
