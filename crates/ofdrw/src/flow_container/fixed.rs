use super::*;

/// A mixed-content container positioned on one existing page.
///
/// Position is the outer allocation's top-left in OFD page coordinates, like
/// `add_text_object`. Positive layout heights are retained; zero measures the
/// content height. Content may extend beyond the box or physical page.
#[derive(Debug, Clone, PartialEq)]
pub struct FixedContainer {
    pub position: [f64; 2],
    pub container: FlowContainer,
}

struct Plan<'a> {
    layout: LayoutBox,
    x: f64,
    y: f64,
    steps: Vec<Step<'a>>,
}

enum Step<'a> {
    Node(Box<Plan<'a>>),
    Text {
        flow: &'a text_measurement::PreparedFlowText,
        x: f64,
        y: f64,
    },
    Image {
        image: &'a TableCellImage,
        x: f64,
        y: f64,
    },
    Table {
        prepared: &'a PreparedNestedTable<'a>,
        style: &'a TableStyle,
        x: f64,
        y: f64,
    },
}

impl OfdDocument {
    /// Draw mixed containers at absolute positions without adding pages.
    ///
    /// Root widths are preserved. Nested floating rows share the flow engine's
    /// clear and alignment rules. Positive content heights control allocation
    /// and decoration; children may overflow them. Zero heights are measured.
    /// Text lines, images and full tables are painted on the selected page.
    /// Each root's decoration and children precede the next root's painting.
    /// Relative offsets and inherited painting layers retain their usual rules.
    /// Page-area fillers are ignored, like Java fixed-page rendering. Empty
    /// line breaks are ignored. Invalid later content rolls back the whole call.
    /// The node and nesting limits are the same as `add_flow_containers`.
    ///
    /// ```
    /// use ofdrw::{FixedContainer, FlowContainer, FlowContainerChild, FlowSpan,
    ///     FlowTextStyle, LayoutBox, OfdDocument};
    /// let mut document = OfdDocument::new();
    /// document.add_blank_page(80., 60.)?;
    /// let placed = document.add_fixed_containers(0, &[FixedContainer {
    ///     position: [10., 20.],
    ///     container: FlowContainer {
    ///         layout: LayoutBox { size: [40., 0.], ..Default::default() },
    ///         children: vec![FlowContainerChild::Paragraph {
    ///             spans: vec![FlowSpan::new("Hello")],
    ///             style: Box::new(FlowTextStyle::default()),
    ///         }], gap: 0.,
    ///     },
    /// }])?;
    /// assert_eq!(placed[0].content_box[..2], [10., 20.]);
    /// assert_eq!(document.pages()?.len(), 1);
    /// # Ok::<(), ofdrw::OfdError>(())
    /// ```
    pub fn add_fixed_containers(
        &mut self,
        page_index: usize,
        containers: &[FixedContainer],
    ) -> Result<Vec<BoxPlacement>, OfdError> {
        let pages = self.pages()?;
        let page = pages.get(page_index).ok_or(OfdError::PageIndex {
            index: page_index,
            count: pages.len(),
        })?;
        self.ensure_unsigned_page(page)?;
        if containers.len() > 10_000 {
            return Err(OfdError::Limit("too many fixed containers".into()));
        }
        let mut count = 0;
        let prepared = containers
            .iter()
            .map(|source| {
                if source.position.iter().any(|value| !value.is_finite()) {
                    return Err(OfdError::Invalid(
                        "fixed container position is invalid".into(),
                    ));
                }
                // Absolute roots retain their requested width, independently of the canvas.
                self.prepare_container(
                    page.document_index,
                    &source.container,
                    f64::INFINITY,
                    0,
                    &mut count,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let plans = prepared
            .iter()
            .zip(containers)
            .map(|(source, fixed)| Plan::new(source, fixed.position[0], fixed.position[1]))
            .collect::<Result<Vec<_>, _>>()?;
        let placements = plans
            .iter()
            .enumerate()
            .map(|(index, plan)| {
                let mut placement = plan.placement(page_index, [0., 0.])?;
                placement.source_index = index;
                Ok(placement)
            })
            .collect::<Result<Vec<_>, OfdError>>()?;
        let mut staged = self.clone();
        for plan in &plans {
            plan.paint(&mut staged, page_index, [0., 0.])?;
        }
        *self = staged;
        Ok(placements)
    }
}

impl<'a> Plan<'a> {
    fn new(source: &'a PreparedContainer<'a>, x: f64, y: f64) -> Result<Self, OfdError> {
        if !source.declared_height.is_finite() || source.declared_height < 0. {
            return Err(OfdError::Invalid(
                "fixed container height is invalid".into(),
            ));
        }
        let b = &source.layout;
        let content_x = x + b.margin[3] + b.border[3] + b.padding[3];
        let content_y = y + b.margin[0] + b.border[0] + b.padding[0];
        if !content_x.is_finite() || !content_y.is_finite() {
            return Err(OfdError::Invalid("fixed container origin overflows".into()));
        }
        let mut steps = Vec::new();
        let mut height = 0.;
        for (row_index, row) in pagination::fixed_rows(source).iter().enumerate() {
            if row_index != 0 {
                height += source.gap;
            }
            let row_y = content_y + height;
            let layouts = row
                .iter()
                .filter_map(|index| match &source.children[*index].node {
                    PreparedChild::Container(nested) => Some(&nested.layout),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let xs = pagination::positions(&layouts, b.size[0]);
            let mut row_height: f64 = 0.;
            for (column, index) in row.iter().enumerate() {
                let child = &source.children[*index];
                let child_height = match &child.node {
                    PreparedChild::Container(nested) => {
                        let plan = Self::new(nested, content_x + xs[column], row_y)?;
                        let height = outer_height(&plan.layout);
                        steps.push(Step::Node(Box::new(plan)));
                        height
                    }
                    PreparedChild::Text(flow) => {
                        steps.push(Step::Text {
                            flow,
                            x: content_x + child.x,
                            y: row_y,
                        });
                        flow.metrics.height
                    }
                    PreparedChild::Image(image) => {
                        steps.push(Step::Image {
                            image,
                            x: content_x + child.x,
                            y: row_y,
                        });
                        image.height
                    }
                    PreparedChild::Table {
                        prepared, style, ..
                    } => {
                        steps.push(Step::Table {
                            prepared,
                            style,
                            x: content_x + child.x,
                            y: row_y,
                        });
                        prepared.height
                    }
                    PreparedChild::Spacer(height) => *height,
                    PreparedChild::LineBreak | PreparedChild::PageAreaFill => 0.,
                };
                row_height = row_height.max(child_height);
            }
            height += row_height;
            if !row_y.is_finite() || !height.is_finite() {
                return Err(OfdError::Invalid(
                    "fixed container content overflows".into(),
                ));
            }
        }
        let mut layout = b.clone();
        layout.size[1] = if source.declared_height > 0. {
            source.declared_height
        } else {
            height
        };
        let plan = Self {
            layout,
            x,
            y,
            steps,
        };
        plan.placement(0, [0., 0.])?;
        Ok(plan)
    }

    fn placement(&self, page: usize, inherited: [f64; 2]) -> Result<BoxPlacement, OfdError> {
        box_layout::nested_fragment(
            &self.layout,
            page,
            self.x + inherited[0],
            self.y + inherited[1],
            0.,
            outer_height(&self.layout),
        )
    }

    fn paint(
        &self,
        document: &mut OfdDocument,
        page: usize,
        inherited: [f64; 2],
    ) -> Result<(), OfdError> {
        if self.layout.placeholder {
            return Ok(());
        }
        document.with_layout_layer(self.layout.layer, |document| {
            let placement = self.placement(page, inherited)?;
            document.draw_box_fragment(&self.layout, &placement)?;
            let offset = [
                inherited[0] + self.layout.offset[0],
                inherited[1] + self.layout.offset[1],
            ];
            for step in &self.steps {
                match step {
                    Step::Node(nested) => nested.paint(document, page, offset)?,
                    Step::Text { flow, x, y } => {
                        for (index, line) in flow.metrics.lines.iter().enumerate() {
                            let [x, y, _, _] = shifted_bounds(
                                *x + line.x,
                                *y + line.y,
                                line.width.max(0.001),
                                line.height,
                                offset,
                            )?;
                            let draw = text_box::FlowLineDraw {
                                page_index: page,
                                x,
                                y,
                                height: line.height,
                                baseline: line.baseline - line.y,
                            };
                            match &flow.lines {
                                text_measurement::PreparedFlowLines::Plain(lines) => document
                                    .draw_plain_flow_line(
                                        &lines[index],
                                        &flow.preparation.spans,
                                        draw,
                                    )?,
                                text_measurement::PreparedFlowLines::Shaped(lines) => document
                                    .draw_shaped_flow_line(
                                        &lines.lines[index],
                                        &lines.group_widths[index],
                                        &flow.preparation.spans,
                                        draw,
                                    )?,
                            }
                        }
                    }
                    Step::Image { image, x, y } => {
                        let boundary = shifted_bounds(*x, *y, image.width, image.height, offset)?;
                        document.add_image_object(page, &image.data, &image.format, boundary)?;
                    }
                    Step::Table {
                        prepared,
                        style,
                        x,
                        y,
                    } => {
                        let [x, y, _, _] = shifted_bounds(
                            *x,
                            *y,
                            prepared.table.column_widths.iter().sum(),
                            prepared.height,
                            offset,
                        )?;
                        draw_nested_table(document, page, prepared, x, y, style)?;
                    }
                }
            }
            Ok(())
        })
    }
}

fn shifted_bounds(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    offset: [f64; 2],
) -> Result<[f64; 4], OfdError> {
    let result = [x + offset[0], y + offset[1], width, height];
    if result.iter().any(|value| !value.is_finite())
        || !(result[0] + width).is_finite()
        || !(result[1] + height).is_finite()
    {
        return Err(OfdError::Invalid(
            "fixed container drawing bounds overflow".into(),
        ));
    }
    Ok(result)
}
