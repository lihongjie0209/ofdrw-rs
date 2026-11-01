use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
struct Point {
    page: usize,
    y: f64,
}

impl Point {
    fn later(self, other: Self) -> Self {
        if self.page > other.page || (self.page == other.page && self.y >= other.y) {
            self
        } else {
            other
        }
    }
}

struct Fragment {
    page: usize,
    y: f64,
    height: f64,
}
struct NodePlan<'a> {
    layout: LayoutBox,
    x: f64,
    fragments: Vec<Fragment>,
    steps: Vec<Step<'a>>,
    end: Point,
}

enum Step<'a> {
    Node(Box<NodePlan<'a>>),
    Line {
        flow: &'a text_measurement::PreparedFlowText,
        index: usize,
        x: f64,
        at: Point,
    },
    Image {
        image: &'a TableCellImage,
        x: f64,
        at: Point,
    },
    Table {
        prepared: &'a PreparedNestedTable<'a>,
        style: &'a TableStyle,
        rows: std::ops::Range<usize>,
        x: f64,
        at: Point,
    },
}

struct Planner {
    top: f64,
    bottom: f64,
    fragments: usize,
}

pub(super) fn needs_dynamic(b: &PreparedContainer<'_>) -> bool {
    b.children.iter().any(|child| match &child.node {
        PreparedChild::Container(nested) => {
            nested.layout.clear != BoxClear::Both || needs_dynamic(nested)
        }
        PreparedChild::Table { style, .. } => style.repeat_header,
        PreparedChild::LineBreak | PreparedChild::PageAreaFill => true,
        _ => false,
    })
}

fn shares(row: &[&LayoutBox], b: &LayoutBox, width: f64) -> bool {
    box_layout::shares_row(
        row.iter().map(|b| (*b, outer_width(b))),
        (b, outer_width(b)),
        width,
    )
}

fn positions(row: &[&LayoutBox], width: f64) -> Vec<f64> {
    box_layout::row_positions(row.iter().map(|b| (*b, outer_width(b))), width, 0.)
}

fn rows(b: &PreparedContainer<'_>) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    let mut row = Vec::new();
    let mut layouts = Vec::new();
    for (index, child) in b.children.iter().enumerate() {
        if matches!(child.node, PreparedChild::LineBreak) {
            if !row.is_empty() {
                result.push(std::mem::take(&mut row));
                layouts.clear();
            }
            continue;
        }
        if let PreparedChild::Container(nested) = &child.node {
            if !row.is_empty()
                && (layouts.len() != row.len()
                    || !shares(&layouts, &nested.layout, b.layout.size[0]))
            {
                result.push(std::mem::take(&mut row));
                layouts.clear();
            }
            row.push(index);
            layouts.push(&nested.layout);
        } else {
            if !row.is_empty() {
                result.push(std::mem::take(&mut row));
                layouts.clear();
            }
            result.push(vec![index]);
        }
    }
    if !row.is_empty() {
        result.push(row);
    }
    result
}

fn intrinsic_height(b: &PreparedContainer<'_>) -> f64 {
    let mut content = 0.;
    let mut gap_pending = false;
    for row in rows(b) {
        if matches!(b.children[row[0]].node, PreparedChild::PageAreaFill) {
            gap_pending = false;
            continue;
        }
        if gap_pending {
            content += b.gap;
        }
        gap_pending = true;
        content += row
            .iter()
            .map(|index| match &b.children[*index].node {
                PreparedChild::Container(nested) => intrinsic_height(nested),
                PreparedChild::Text(flow) => flow.metrics.height,
                PreparedChild::Image(image) => image.height,
                PreparedChild::Spacer(height) => *height,
                PreparedChild::LineBreak | PreparedChild::PageAreaFill => 0.,
                PreparedChild::Table { prepared, .. } => prepared.height,
            })
            .fold(0., f64::max);
    }
    content
        + b.layout.margin[0]
        + b.layout.margin[2]
        + b.layout.border[0]
        + b.layout.border[2]
        + b.layout.padding[0]
        + b.layout.padding[2]
}

fn record(spans: &mut BTreeMap<usize, (f64, f64)>, at: Point, height: f64) {
    let span = spans.entry(at.page).or_insert((at.y, at.y));
    span.0 = span.0.min(at.y);
    span.1 = span.1.max(at.y + height);
}

impl Planner {
    fn advance(&self, at: &mut Point) -> Result<(), OfdError> {
        at.page += 1;
        at.y = self.top;
        if at.page > 10_000 {
            return Err(OfdError::Limit(
                "container pagination exceeds page limits".into(),
            ));
        }
        Ok(())
    }

    fn unit(&self, at: &mut Point, height: f64) -> Result<Point, OfdError> {
        if !height.is_finite() || height < 0. || height > self.bottom - self.top + 1e-9 {
            return Err(OfdError::Invalid(
                "container content unit exceeds page height".into(),
            ));
        }
        if at.y + height > self.bottom + 1e-9 {
            self.advance(at)?;
        }
        let start = *at;
        at.y += height;
        Ok(start)
    }

    fn space(
        &self,
        at: &mut Point,
        mut height: f64,
        spans: &mut BTreeMap<usize, (f64, f64)>,
    ) -> Result<(), OfdError> {
        if !height.is_finite() || height < 0. {
            return Err(OfdError::Invalid("container spacing overflows".into()));
        }
        if height / (self.bottom - self.top) > 10_000. {
            return Err(OfdError::Limit(
                "container spacing exceeds page limits".into(),
            ));
        }
        while height > 0. {
            let available = (self.bottom - at.y).max(0.);
            if available == 0. {
                self.advance(at)?;
                continue;
            }
            let consumed = height.min(available);
            record(spans, *at, consumed);
            at.y += consumed;
            height -= consumed;
        }
        Ok(())
    }

    fn node<'a>(
        &mut self,
        b: &'a PreparedContainer<'a>,
        x: f64,
        mut at: Point,
    ) -> Result<NodePlan<'a>, OfdError> {
        let original_start = at;
        if b.layout.integrity {
            let height = intrinsic_height(b);
            if height > self.bottom - self.top + 1e-9 || !height.is_finite() {
                return Err(OfdError::Invalid(
                    "complete container exceeds page height".into(),
                ));
            }
            if at.y + height > self.bottom + 1e-9 {
                self.advance(&mut at)?;
            }
        }
        let mut spans = BTreeMap::new();
        let mut steps = Vec::new();
        self.space(
            &mut at,
            b.layout.margin[0] + b.layout.border[0] + b.layout.padding[0],
            &mut spans,
        )?;
        let content_x = x + b.layout.margin[3] + b.layout.border[3] + b.layout.padding[3];
        let mut gap_pending = false;
        for row in rows(b) {
            let fill = matches!(b.children[row[0]].node, PreparedChild::PageAreaFill);
            if gap_pending && !fill {
                self.space(&mut at, b.gap, &mut spans)?;
            }
            gap_pending = !fill;
            let row_start = at;
            let mut end = at;
            let mut row_spans = BTreeMap::new();
            let layouts = row
                .iter()
                .filter_map(|index| match &b.children[*index].node {
                    PreparedChild::Container(nested) => Some(&nested.layout),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let xs = positions(&layouts, b.layout.size[0]);
            for (column, index) in row.iter().enumerate() {
                let child = &b.children[*index];
                let mut cursor = row_start;
                match &child.node {
                    PreparedChild::Container(nested) => {
                        let plan = self.node(nested, content_x + xs[column], cursor)?;
                        for fragment in &plan.fragments {
                            if fragment.height == 0. {
                                continue;
                            }
                            record(
                                &mut row_spans,
                                Point {
                                    page: fragment.page,
                                    y: fragment.y,
                                },
                                fragment.height,
                            );
                        }
                        cursor = plan.end;
                        steps.push(Step::Node(Box::new(plan)));
                    }
                    PreparedChild::LineBreak => {}
                    PreparedChild::PageAreaFill => {
                        if cursor.y >= self.bottom {
                            self.advance(&mut cursor)?;
                        }
                        let remaining = (self.bottom - cursor.y).max(0.);
                        self.space(&mut cursor, remaining, &mut row_spans)?;
                    }
                    PreparedChild::Spacer(height) => {
                        self.space(&mut cursor, *height, &mut row_spans)?
                    }
                    PreparedChild::Text(flow) => {
                        let lines = &flow.metrics.lines;
                        let mut index = 0;
                        let mut prior_end = 0.;
                        while index < lines.len() {
                            self.space(
                                &mut cursor,
                                (lines[index].y - prior_end).max(0.),
                                &mut row_spans,
                            )?;
                            let mut stop = index + 1;
                            if child.keep_paragraphs {
                                while stop < lines.len() && !lines[stop].paragraph_start {
                                    stop += 1;
                                }
                            }
                            let height =
                                lines[stop - 1].y + lines[stop - 1].height - lines[index].y;
                            let start = self.unit(&mut cursor, height)?;
                            record(&mut row_spans, start, height);
                            for line_index in index..stop {
                                steps.push(Step::Line {
                                    flow,
                                    index: line_index,
                                    x: content_x + child.x + lines[line_index].x,
                                    at: Point {
                                        page: start.page,
                                        y: start.y + lines[line_index].y - lines[index].y,
                                    },
                                });
                            }
                            prior_end = lines[stop - 1].y + lines[stop - 1].height;
                            index = stop;
                        }
                    }
                    PreparedChild::Image(image) => {
                        let start = self.unit(&mut cursor, image.height)?;
                        record(&mut row_spans, start, image.height);
                        steps.push(Step::Image {
                            image,
                            x: content_x + child.x,
                            at: start,
                        });
                    }
                    PreparedChild::Table {
                        prepared,
                        style,
                        groups,
                    } => {
                        let repeated = style.repeat_header && groups.len() > 1;
                        if style.repeat_header && groups[0].rows != (0..1) {
                            return Err(OfdError::Invalid(
                                "table header cannot span later rows".into(),
                            ));
                        }
                        for (index, group) in groups.iter().enumerate() {
                            let height = group.end - group.start;
                            if index == 0 && repeated {
                                let pair = height + (groups[1].end - groups[1].start);
                                if pair > self.bottom - self.top + 1e-9 {
                                    return Err(OfdError::Invalid(
                                        "table header and body group exceed page height".into(),
                                    ));
                                }
                                if cursor.y + pair > self.bottom + 1e-9 {
                                    self.advance(&mut cursor)?;
                                }
                            } else if index > 0
                                && repeated
                                && cursor.y + height > self.bottom + 1e-9
                            {
                                let header = &groups[0];
                                let header_height = header.end - header.start;
                                if header_height + height > self.bottom - self.top + 1e-9 {
                                    return Err(OfdError::Invalid(
                                        "table header and body group exceed page height".into(),
                                    ));
                                }
                                self.advance(&mut cursor)?;
                                let start = self.unit(&mut cursor, header_height)?;
                                record(&mut row_spans, start, header_height);
                                steps.push(Step::Table {
                                    prepared,
                                    style,
                                    rows: header.rows.clone(),
                                    x: content_x + child.x,
                                    at: start,
                                });
                            }
                            let start = self.unit(&mut cursor, height)?;
                            record(&mut row_spans, start, height);
                            steps.push(Step::Table {
                                prepared,
                                style,
                                rows: group.rows.clone(),
                                x: content_x + child.x,
                                at: start,
                            });
                        }
                    }
                }
                end = end.later(cursor);
            }
            for (page, (start, mut stop)) in row_spans {
                if row.len() > 1 && page < end.page {
                    stop = self.bottom;
                }
                record(&mut spans, Point { page, y: start }, stop - start);
            }
            at = end;
        }
        self.space(
            &mut at,
            b.layout.padding[2] + b.layout.border[2] + b.layout.margin[2],
            &mut spans,
        )?;
        if spans.is_empty() {
            record(&mut spans, original_start.later(at), 0.);
        }
        let fragments = spans
            .into_iter()
            .map(|(page, (y, end))| Fragment {
                page,
                y,
                height: end - y,
            })
            .collect::<Vec<_>>();
        if b.layout.integrity && fragments.len() > 1 {
            return Err(OfdError::Invalid(
                "complete container would span multiple pages".into(),
            ));
        }
        self.fragments += fragments.len();
        if self.fragments > 100_000 {
            return Err(OfdError::Limit(
                "too many nested container fragments".into(),
            ));
        }
        let mut layout = b.layout.clone();
        let allocated = fragments
            .iter()
            .map(|fragment| fragment.height)
            .sum::<f64>();
        layout.size[1] = (allocated
            - layout.margin[0]
            - layout.margin[2]
            - layout.border[0]
            - layout.border[2]
            - layout.padding[0]
            - layout.padding[2])
            .max(0.);
        if !layout.size[1].is_finite() {
            return Err(OfdError::Invalid("container height overflows".into()));
        }
        Ok(NodePlan {
            layout,
            x,
            fragments,
            steps,
            end: at,
        })
    }
}

fn translated_placements(
    plan: &NodePlan<'_>,
    pages: &[(usize, [f64; 2])],
    offset: [f64; 2],
) -> Result<Vec<BoxPlacement>, OfdError> {
    let mut consumed = 0.;
    let mut result = Vec::new();
    for (index, fragment) in plan.fragments.iter().enumerate() {
        let (page, origin) = pages[fragment.page];
        let mut placement = box_layout::nested_fragment(
            &plan.layout,
            page,
            plan.x + origin[0] + offset[0],
            fragment.y + origin[1] + offset[1],
            consumed,
            fragment.height,
        )?;
        placement.first = index == 0;
        placement.last = index + 1 == plan.fragments.len();
        consumed += fragment.height;
        result.push(placement);
    }
    Ok(result)
}

fn paint_children(
    document: &mut OfdDocument,
    plan: &NodePlan<'_>,
    pages: &[(usize, [f64; 2])],
    inherited: [f64; 2],
) -> Result<(), OfdError> {
    if plan.layout.placeholder {
        return Ok(());
    }
    let offset = [
        inherited[0] + plan.layout.offset[0],
        inherited[1] + plan.layout.offset[1],
    ];
    if offset.iter().any(|value| !value.is_finite()) {
        return Err(OfdError::Invalid("nested drawing offset overflows".into()));
    }
    for step in &plan.steps {
        match step {
            Step::Node(nested) => {
                let placements = translated_placements(nested, pages, offset)?;
                if !nested.layout.placeholder {
                    for placement in &placements {
                        document.draw_box_fragment(&nested.layout, placement)?;
                    }
                }
                paint_children(document, nested, pages, offset)?;
            }
            Step::Line { flow, index, x, at } => {
                let (page, origin) = pages[at.page];
                let line = &flow.metrics.lines[*index];
                let draw = text_box::FlowLineDraw {
                    page_index: page,
                    x: x + origin[0] + offset[0],
                    y: at.y + origin[1] + offset[1],
                    height: line.height,
                    baseline: line.baseline - line.y,
                };
                if !draw.x.is_finite()
                    || !draw.y.is_finite()
                    || !(draw.x + line.width).is_finite()
                    || !(draw.y + line.height).is_finite()
                {
                    return Err(OfdError::Invalid("nested text coordinates overflow".into()));
                }
                match &flow.lines {
                    text_measurement::PreparedFlowLines::Plain(lines) => document
                        .draw_plain_flow_line(&lines[*index], &flow.preparation.spans, draw)?,
                    text_measurement::PreparedFlowLines::Shaped(lines) => document
                        .draw_shaped_flow_line(
                            &lines.lines[*index],
                            &lines.group_widths[*index],
                            &flow.preparation.spans,
                            draw,
                        )?,
                }
            }
            Step::Image { image, x, at } => {
                let (page, origin) = pages[at.page];
                document.add_image_object(
                    page,
                    &image.data,
                    &image.format,
                    [
                        x + origin[0] + offset[0],
                        at.y + origin[1] + offset[1],
                        image.width,
                        image.height,
                    ],
                )?;
            }
            Step::Table {
                prepared,
                style,
                rows,
                x,
                at,
            } => {
                let (page, origin) = pages[at.page];
                draw_nested_table_rows(
                    document,
                    page,
                    prepared,
                    x + origin[0] + offset[0],
                    at.y + origin[1] + offset[1],
                    style,
                    rows.clone(),
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn draw_containers(
    document: &mut OfdDocument,
    document_index: usize,
    prepared: &[PreparedContainer<'_>],
    style: BoxLayoutStyle,
) -> Result<BoxLayoutResult, OfdError> {
    let width = style.page_size[0] - style.margins[1] - style.margins[3];
    let height = style.page_size[1] - style.margins[0] - style.margins[2];
    if style
        .page_size
        .into_iter()
        .chain(style.margins)
        .chain([width, height])
        .any(|v| !v.is_finite())
        || style.page_size.iter().any(|v| *v <= 0.)
        || style.margins.iter().any(|v| *v < 0.)
        || width <= 0.
        || height <= 0.
    {
        return Err(OfdError::Invalid(
            "container page geometry is invalid".into(),
        ));
    }
    if document_index >= document.doc_roots()?.len() {
        return Err(OfdError::Invalid("document index is out of bounds".into()));
    }
    document.ensure_unsigned_document(document_index)?;
    let cursor = FlowPageCursor::new(
        document,
        document_index,
        style.page_size,
        style.margins,
        style.start,
    )?;
    let mut planner = Planner {
        top: style.margins[0],
        bottom: style.page_size[1] - style.margins[2],
        fragments: 0,
    };
    let mut at = Point {
        page: 0,
        y: cursor.y - cursor.origin[1],
    };
    let mut root_rows = Vec::new();
    let mut row = Vec::new();
    let mut layouts = Vec::new();
    for (index, b) in prepared.iter().enumerate() {
        if !shares(&layouts, &b.layout, width) {
            root_rows.push(std::mem::take(&mut row));
            layouts.clear();
        }
        row.push(index);
        layouts.push(&b.layout);
    }
    if !row.is_empty() {
        root_rows.push(row);
    }
    let mut plans = Vec::new();
    for row in root_rows {
        let layouts = row
            .iter()
            .map(|index| &prepared[*index].layout)
            .collect::<Vec<_>>();
        let xs = positions(&layouts, width);
        let mut end = at;
        for (column, index) in row.into_iter().enumerate() {
            let plan = planner.node(&prepared[index], style.margins[3] + xs[column], at)?;
            end = end.later(plan.end);
            plans.push((index, plan));
        }
        at = end;
    }
    let used_ordinals = plans
        .iter()
        .flat_map(|(_, plan)| plan.fragments.iter().map(|fragment| fragment.page))
        .collect::<BTreeSet<_>>();
    if used_ordinals.len() > 10_000 {
        return Err(OfdError::Limit(
            "container pagination exceeds used-page limits".into(),
        ));
    }
    let max_page = plans
        .iter()
        .flat_map(|(_, plan)| plan.fragments.iter().map(|fragment| fragment.page))
        .max()
        .unwrap_or(0);
    let mut staged = document.clone();
    let mut page_map = Vec::new();
    for ordinal in 0..=max_page {
        if ordinal == 0 {
            if let Some(page) = cursor.page {
                page_map.push((page, cursor.origin));
                continue;
            }
        }
        staged.append_blank_page(document_index, style.page_size[0], style.page_size[1])?;
        let page = staged
            .pages()?
            .into_iter()
            .rfind(|page| page.document_index == document_index)
            .ok_or_else(|| OfdError::Invalid("new container page is missing".into()))?
            .index;
        page_map.push((page, [0., 0.]));
    }
    let mut placements = Vec::new();
    let mut used = BTreeSet::new();
    for (source_index, plan) in &plans {
        let translated = translated_placements(plan, &page_map, [0., 0.])?;
        for mut placement in translated {
            placement.source_index = *source_index;
            if !plan.layout.placeholder {
                staged.draw_box_fragment(&plan.layout, &placement)?;
            }
            used.insert(placement.page_index);
            placements.push(placement);
        }
    }
    for (_, plan) in &plans {
        paint_children(&mut staged, plan, &page_map, [0., 0.])?;
    }
    placements.sort_by_key(|placement| (placement.page_index, placement.source_index));
    let next = FlowStart {
        page_index: page_map[at.page].0,
        y: at.y.min(planner.bottom),
    };
    *document = staged;
    Ok(BoxLayoutResult {
        pages: used.into_iter().collect(),
        placements,
        next: Some(next),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_existing_start_page_does_not_consume_the_used_page_budget() {
        let source = PreparedContainer {
            layout: LayoutBox {
                size: [1., 10_000.],
                ..Default::default()
            },
            children: vec![PlacedChild {
                id: 1,
                keep_paragraphs: false,
                x: 0.,
                y: 0.,
                node: PreparedChild::Spacer(10_000.),
            }],
            ranges: Vec::new(),
            gap: 0.,
        };
        let mut planner = Planner {
            top: 0.,
            bottom: 1.,
            fragments: 0,
        };
        let plan = planner.node(&source, 0., Point { page: 0, y: 1. }).unwrap();
        assert_eq!(plan.fragments.len(), 10_000);
        assert_eq!(plan.fragments[0].page, 1);
        assert_eq!(plan.fragments.last().unwrap().page, 10_000);
        assert_eq!(
            plan.fragments
                .iter()
                .map(|fragment| fragment.height)
                .sum::<f64>(),
            10_000.
        );
    }
}
