use super::*;

/// Horizontal placement within a shared layout row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxFloat {
    #[default]
    Left,
    Right,
    Center,
}

/// Row sharing rules corresponding to Java OFDRW's `Clear` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxClear {
    None,
    Left,
    Right,
    #[default]
    Both,
}

/// Painting layer for box decoration and content, ordered by the OFD page model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxLayer {
    Background,
    #[default]
    Body,
    Foreground,
}

impl BoxLayer {
    pub(super) fn ofd_type(self) -> &'static str {
        match self {
            Self::Background => "Background",
            Self::Body => "Body",
            Self::Foreground => "Foreground",
        }
    }
}

/// A measured box. Dimensions and top/right/bottom/left edges are millimeters.
///
/// `size` describes the content. Width is reduced to fit the full content area,
/// before row grouping. Relative `offset` changes drawing coordinates without
/// changing occupied space. A placeholder occupies space without drawing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutBox {
    pub size: [f64; 2],
    pub margin: [f64; 4],
    pub padding: [f64; 4],
    pub border: [f64; 4],
    pub background_color: Option<[u8; 3]>,
    pub border_color: [u8; 3],
    /// Java-style dash arguments: one length repeats as dash/gap; even-length
    /// sequences have zero offset; odd sequences begin with the offset.
    /// Empty sequences select a solid border. Up to 256 positive lengths.
    pub border_dash: Option<Vec<f64>>,
    pub alpha: Option<u8>,
    pub float: BoxFloat,
    pub clear: BoxClear,
    /// Move the complete box to a new page instead of splitting it vertically.
    pub integrity: bool,
    pub offset: [f64; 2],
    pub placeholder: bool,
    /// Painting layer. None inherits the enclosing container, or Body at the root.
    pub layer: Option<BoxLayer>,
}

/// Measured boxes and explicit row/page boundaries.
#[derive(Debug, Clone, PartialEq)]
pub enum BoxLayoutItem {
    Box(LayoutBox),
    /// Finish the current row. An empty row is ignored.
    LineBreak,
    /// Append a blank page and continue there, retaining explicit blank pages.
    PageBreak,
}

/// Page geometry for box layout; an existing page may be selected by `start`.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxLayoutStyle {
    pub page_size: [f64; 2],
    pub margins: [f64; 4],
    pub start: Option<FlowStart>,
}

impl Default for BoxLayoutStyle {
    fn default() -> Self {
        let text = FlowTextStyle::default();
        Self {
            page_size: text.page_size,
            margins: text.margins,
            start: None,
        }
    }
}

/// A vertical fragment of one source box, including nondrawing margin fragments.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxPlacement {
    pub source_index: usize,
    pub page_index: usize,
    /// Space occupied before applying the relative offset.
    pub allocation: [f64; 4],
    /// Drawing rectangles include the relative offset and page canvas origin.
    pub border_box: [f64; 4],
    pub padding_box: [f64; 4],
    pub content_box: [f64; 4],
    /// Effective top/right/bottom/left border widths after fragmentation.
    pub borders: [f64; 4],
    /// Millimeters consumed from the original outer box before this fragment.
    pub source_offset: f64,
    pub first: bool,
    pub last: bool,
}

/// Global page indexes, box fragments, and the next available vertical position.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxLayoutResult {
    pub pages: Vec<usize>,
    pub placements: Vec<BoxPlacement>,
    pub next: Option<FlowStart>,
}

/// Indivisible vertical content ranges for one box, in content coordinates.
///
/// Ranges must be positive, ordered, disjoint and inside the box's content
/// height. A page break inside a range moves that range to the next page.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxContentRanges {
    pub source_index: usize,
    pub ranges: Vec<[f64; 2]>,
}

impl BoxContentRanges {
    /// Protect each measured text line, including its leading, from page cuts.
    pub fn from_text(source_index: usize, metrics: &FlowTextMetrics) -> Self {
        Self {
            source_index,
            ranges: metrics
                .lines
                .iter()
                .map(|line| [line.y, line.y + line.height])
                .collect(),
        }
    }
}

struct PreparedBox<'a> {
    source_index: usize,
    source: &'a LayoutBox,
    width: f64,
    height: f64,
    content_width: f64,
    ranges: &'a [[f64; 2]],
}

impl PreparedBox<'_> {
    fn fragment(&self, page: usize, x: f64, y: f64, offset: f64, height: f64) -> BoxPlacement {
        let b = self.source;
        let bands = [
            b.margin[0],
            b.border[0],
            b.padding[0],
            b.size[1],
            b.padding[2],
            b.border[2],
            b.margin[2],
        ];
        let mut parts = [0.; 7];
        let mut start = 0.;
        for (part, band) in parts.iter_mut().zip(bands) {
            let end = start + band;
            *part = (end.min(offset + height) - start.max(offset)).max(0.);
            start = end;
        }
        let bx = x + b.offset[0] + b.margin[3];
        let by = y + b.offset[1] + parts[0];
        let bw = self.width - b.margin[1] - b.margin[3];
        let bh: f64 = parts[1..6].iter().sum();
        let px = bx + b.border[3];
        let py = by + parts[1];
        let pw = self.content_width + b.padding[1] + b.padding[3];
        let ph: f64 = parts[2..5].iter().sum();
        BoxPlacement {
            source_index: self.source_index,
            page_index: page,
            allocation: [x, y, self.width, height],
            border_box: [bx, by, bw, bh],
            padding_box: [px, py, pw, ph],
            content_box: [
                px + b.padding[3],
                py + parts[2],
                self.content_width,
                parts[3],
            ],
            borders: [parts[1], b.border[1], parts[5], b.border[3]],
            source_offset: offset,
            first: offset == 0.,
            last: offset + height >= self.height,
        }
    }
}

fn exclusive(b: &LayoutBox) -> bool {
    b.clear == BoxClear::Both
        || matches!(
            (b.float, b.clear),
            (BoxFloat::Left, BoxClear::Right) | (BoxFloat::Right, BoxClear::Left)
        )
}

pub(super) fn shares_row<'a>(
    mut row: impl Iterator<Item = (&'a LayoutBox, f64)> + Clone,
    item: (&LayoutBox, f64),
    width: f64,
) -> bool {
    let Some((first, _)) = row.clone().next() else {
        return true;
    };
    let b = item.0;
    if exclusive(b)
        || exclusive(first)
        || row.clone().map(|(_, width)| width).sum::<f64>() + item.1 > width
    {
        return false;
    }
    if first.float == BoxFloat::Center {
        b.float == BoxFloat::Center
            && b.clear != BoxClear::Left
            && row.last().is_none_or(|(b, _)| b.clear != BoxClear::Right)
    } else {
        b.float != BoxFloat::Center
            && !row.any(|(prior, _)| {
                prior.float == b.float
                    && matches!(
                        (b.float, b.clear),
                        (BoxFloat::Left, BoxClear::Left) | (BoxFloat::Right, BoxClear::Right)
                    )
            })
    }
}

pub(super) fn row_positions<'a>(
    row: impl Iterator<Item = (&'a LayoutBox, f64)> + Clone,
    width: f64,
    start_x: f64,
) -> Vec<f64> {
    let centered = row
        .clone()
        .next()
        .is_some_and(|(b, _)| b.float == BoxFloat::Center);
    let mut left = start_x;
    let mut right = left + width;
    if centered {
        left += (width - row.clone().map(|(_, width)| width).sum::<f64>()) / 2.;
    }
    row.map(|(b, width)| {
        if b.float == BoxFloat::Right && !centered {
            right -= width;
            right
        } else {
            let x = left;
            left += width;
            x
        }
    })
    .collect()
}

fn can_share(row: &[PreparedBox<'_>], item: &PreparedBox<'_>, width: f64) -> bool {
    shares_row(
        row.iter().map(|b| (b.source, b.width)),
        (item.source, item.width),
        width,
    )
}

impl OfdDocument {
    /// Arrange measured boxes in Java-style floating rows and draw decorations.
    ///
    /// Left and right boxes share a row when their clear rules and widths allow
    /// it. Centered boxes form separate centered groups. Row height is the
    /// largest outer height. Splittable rows retain column positions across
    /// pages; cut edges do not repeat top/bottom margin, padding, or borders.
    /// Complete boxes move to the next page when needed. Oversized complete
    /// boxes fail rather than being silently dropped. All changes are atomic.
    /// Calls accept at most 10,000 items, 10,000 used pages, and 100,000 fragments.
    ///
    /// This API places measured rectangles and paints backgrounds and borders.
    /// Returned content rectangles provide coordinates for subsequent content
    /// placement; text measurement and child containers are separate concerns.
    ///
    /// ```
    /// use ofdrw::{BoxLayoutItem, BoxLayoutStyle, LayoutBox, OfdDocument};
    /// let mut document = OfdDocument::new();
    /// let result = document.add_box_layout(0, &[BoxLayoutItem::Box(LayoutBox {
    ///     size: [40., 10.], padding: [2.; 4], border: [0.2; 4],
    ///     background_color: Some([240, 240, 255]), ..Default::default()
    /// })], BoxLayoutStyle::default())?;
    /// let placed = &result.placements[0];
    /// document.add_text_object(placed.page_index, "Hello", placed.content_box, 5., None)?;
    /// # Ok::<(), ofdrw::OfdError>(())
    /// ```
    pub fn add_box_layout(
        &mut self,
        document_index: usize,
        items: &[BoxLayoutItem],
        style: BoxLayoutStyle,
    ) -> Result<BoxLayoutResult, OfdError> {
        self.add_box_layout_with_content_ranges(document_index, items, &[], style)
    }

    /// Lay out boxes while keeping specified content ranges on a single page.
    ///
    /// Ranges are relative to content tops, before margins, borders, padding
    /// and drawing offsets. Columns can consume different source heights at a
    /// page boundary; their remaining content retains its horizontal position.
    /// Unused space below a protected range is left blank. A range taller than
    /// a full page content area is rejected. All changes remain atomic.
    pub fn add_box_layout_with_content_ranges(
        &mut self,
        document_index: usize,
        items: &[BoxLayoutItem],
        content_ranges: &[BoxContentRanges],
        style: BoxLayoutStyle,
    ) -> Result<BoxLayoutResult, OfdError> {
        if items.len() > 10_000 {
            return Err(OfdError::Limit("too many layout boxes".into()));
        }
        let [pw, ph] = style.page_size;
        let [top, right, bottom, left] = style.margins;
        let width = pw - left - right;
        let height = ph - top - bottom;
        if style
            .page_size
            .into_iter()
            .chain(style.margins)
            .chain([width, height])
            .any(|v| !v.is_finite())
            || pw <= 0.
            || ph <= 0.
            || style.margins.iter().any(|v| *v < 0.)
            || width <= 0.
            || height <= 0.
        {
            return Err(OfdError::Invalid(
                "box layout page geometry is invalid".into(),
            ));
        }
        if document_index >= self.doc_roots()?.len() {
            return Err(OfdError::Invalid("document index is out of bounds".into()));
        }
        self.ensure_unsigned_document(document_index)?;
        let mut cursor = FlowPageCursor::new(
            self,
            document_index,
            style.page_size,
            style.margins,
            style.start,
        )?;
        let mut ranges_by_item = vec![None; items.len()];
        let mut range_count = 0usize;
        for policy in content_ranges {
            let Some(BoxLayoutItem::Box(b)) = items.get(policy.source_index) else {
                return Err(OfdError::Invalid(
                    "content ranges must reference a box".into(),
                ));
            };
            if ranges_by_item[policy.source_index].is_some() {
                return Err(OfdError::Invalid("duplicate box content ranges".into()));
            }
            range_count = range_count.saturating_add(policy.ranges.len());
            if range_count > 100_000 {
                return Err(OfdError::Limit("too many box content ranges".into()));
            }
            let mut end = 0.;
            for &[start, stop] in &policy.ranges {
                if !start.is_finite()
                    || !stop.is_finite()
                    || start < end
                    || stop <= start
                    || stop > b.size[1]
                    || stop - start > height
                {
                    return Err(OfdError::Invalid(
                        "box content ranges are invalid or too tall".into(),
                    ));
                }
                end = stop;
            }
            ranges_by_item[policy.source_index] = Some(policy.ranges.as_slice());
        }
        let mut prepared = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            let BoxLayoutItem::Box(b) = item else {
                prepared.push(None);
                continue;
            };
            validate_border_dash(b.border_dash.as_deref())?;
            if b.size
                .into_iter()
                .chain(b.margin)
                .chain(b.padding)
                .chain(b.border)
                .any(|v| !v.is_finite() || v < 0.)
                || b.offset.iter().any(|v| !v.is_finite())
            {
                return Err(OfdError::Invalid("layout box geometry is invalid".into()));
            }
            let horizontal =
                b.margin[1] + b.margin[3] + b.padding[1] + b.padding[3] + b.border[1] + b.border[3];
            let content_width = b.size[0].min(width - horizontal);
            let outer_width = content_width + horizontal;
            let outer_height = b.size[1]
                + b.margin[0]
                + b.margin[2]
                + b.padding[0]
                + b.padding[2]
                + b.border[0]
                + b.border[2];
            if content_width < 0.
                || ![outer_width, outer_height].iter().all(|v| v.is_finite())
                || (b.integrity && outer_height > height)
            {
                return Err(OfdError::Invalid(
                    "layout box does not fit the content area".into(),
                ));
            }
            if (outer_height / height).ceil() > 10_000. {
                return Err(OfdError::Limit("layout box spans too many pages".into()));
            }
            prepared.push(Some(PreparedBox {
                source_index: index,
                source: b,
                width: outer_width,
                height: outer_height,
                content_width,
                ranges: ranges_by_item[index].unwrap_or(&[]),
            }));
        }
        let mut staged = self.clone();
        let mut pages = Vec::new();
        let mut placements = Vec::new();
        let mut row = Vec::new();
        for (item, b) in items.iter().zip(prepared) {
            if let Some(b) = b {
                if !can_share(&row, &b, width) {
                    staged.draw_box_row(
                        document_index,
                        &row,
                        &mut cursor,
                        &mut pages,
                        &mut placements,
                    )?;
                    row.clear();
                }
                row.push(b);
            } else {
                staged.draw_box_row(
                    document_index,
                    &row,
                    &mut cursor,
                    &mut pages,
                    &mut placements,
                )?;
                row.clear();
                if matches!(item, BoxLayoutItem::PageBreak) {
                    cursor.page = None;
                    cursor.place(&mut staged, document_index, 0., 0., &mut pages)?;
                }
            }
        }
        staged.draw_box_row(
            document_index,
            &row,
            &mut cursor,
            &mut pages,
            &mut placements,
        )?;
        if pages.len() > 10_000 {
            return Err(OfdError::Limit("box layout exceeds page limits".into()));
        }
        let result = BoxLayoutResult {
            pages,
            placements,
            next: cursor.page.map(|page_index| FlowStart {
                page_index,
                y: (cursor.y - cursor.origin[1]).min(ph - bottom),
            }),
        };
        *self = staged;
        Ok(result)
    }

    fn draw_box_row(
        &mut self,
        document_index: usize,
        row: &[PreparedBox<'_>],
        cursor: &mut FlowPageCursor,
        pages: &mut Vec<usize>,
        placements: &mut Vec<BoxPlacement>,
    ) -> Result<(), OfdError> {
        if row.is_empty() {
            return Ok(());
        }
        let width = cursor.page_size[0] - cursor.margins[1] - cursor.margins[3];
        let positions = row_positions(
            row.iter().map(|b| (b.source, b.width)),
            width,
            cursor.margins[3],
        );
        let mut offsets = vec![0.; row.len()];
        let mut done = vec![false; row.len()];
        while done.iter().any(|v| !v) {
            cursor.place(self, document_index, 0., 0., &mut Vec::new())?;
            let available =
                (cursor.origin[1] + cursor.page_size[1] - cursor.margins[2] - cursor.y).max(0.);
            let remaining = row
                .iter()
                .zip(&offsets)
                .zip(&done)
                .filter(|(_, done)| !**done)
                .map(|((b, offset), _)| b.height - offset)
                .fold(0., f64::max);
            let splittable = row
                .iter()
                .zip(&done)
                .any(|(b, done)| !done && !b.source.integrity);
            let required = if remaining <= available || !splittable {
                remaining
            } else {
                available
            };
            if required == 0. && remaining > 0. {
                cursor.page = None;
                continue;
            }
            let consumptions: Vec<f64> = row
                .iter()
                .enumerate()
                .map(|(index, b)| {
                    if done[index] || (b.source.integrity && b.height - offsets[index] > required) {
                        return 0.;
                    }
                    let mut consumed = (b.height - offsets[index]).min(required);
                    let content_top = b.source.margin[0] + b.source.border[0] + b.source.padding[0];
                    let cut = offsets[index] + consumed - content_top;
                    let range_index = b.ranges.partition_point(|range| range[1] <= cut);
                    if let Some(&[start, end]) = b.ranges.get(range_index) {
                        if start < cut && cut < end {
                            consumed = (content_top + start - offsets[index]).max(0.);
                        }
                    }
                    consumed
                })
                .collect();
            if remaining > 0. && consumptions.iter().all(|value| *value == 0.) {
                cursor.page = None;
                continue;
            }
            let (page, origin_x, y) = cursor.place(self, document_index, required, 0., pages)?;
            for (index, b) in row.iter().enumerate() {
                if done[index] {
                    continue;
                }
                let remaining = b.height - offsets[index];
                if b.source.integrity && remaining > required {
                    continue;
                }
                let consumed = consumptions[index];
                if consumed == 0. && remaining > 0. {
                    continue;
                }
                let placement = b.fragment(
                    page,
                    origin_x + positions[index],
                    y,
                    offsets[index],
                    consumed,
                );
                if placement
                    .allocation
                    .iter()
                    .chain(&placement.border_box)
                    .chain(&placement.padding_box)
                    .chain(&placement.content_box)
                    .any(|v| !v.is_finite())
                {
                    return Err(OfdError::Invalid("layout box coordinates overflow".into()));
                }
                for [x, y, w, h] in [
                    placement.border_box,
                    placement.padding_box,
                    placement.content_box,
                ] {
                    if !(x + w).is_finite() || !(y + h).is_finite() {
                        return Err(OfdError::Invalid("layout box bounds overflow".into()));
                    }
                }
                if !b.source.placeholder {
                    self.draw_box_fragment(b.source, &placement)?;
                }
                placements.push(placement);
                offsets[index] += consumed;
                done[index] = consumed >= remaining;
            }
            cursor.y += required;
            if pages.len() > 10_000 || placements.len() > 100_000 {
                return Err(OfdError::Limit(
                    "box layout exceeds page or fragment limits".into(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn with_layout_layer<T>(
        &mut self,
        layer: Option<BoxLayer>,
        draw: impl FnOnce(&mut Self) -> Result<T, OfdError>,
    ) -> Result<T, OfdError> {
        let previous = self.layout_layer;
        self.layout_layer = layer.or(previous);
        let result = draw(self);
        self.layout_layer = previous;
        result
    }

    pub(super) fn draw_box_fragment(
        &mut self,
        b: &LayoutBox,
        p: &BoxPlacement,
    ) -> Result<(), OfdError> {
        self.with_layout_layer(b.layer, |document| {
            document.draw_box_fragment_in_layer(b, p)
        })
    }

    fn draw_box_fragment_in_layer(
        &mut self,
        b: &LayoutBox,
        p: &BoxPlacement,
    ) -> Result<(), OfdError> {
        let [x, y, w, h] = p.padding_box;
        if let Some(color) = b.background_color.filter(|_| w > 0. && h > 0.) {
            self.add_path_object(
                p.page_index,
                &format!("M 0 0 L {w} 0 L {w} {h} L 0 {h} C"),
                [x, y, w, h],
                PathStyle {
                    fill: true,
                    stroke: false,
                    fill_color: Some(color),
                    alpha: b.alpha,
                    ..Default::default()
                },
            )?;
        }
        let [x, y, w, h] = p.border_box;
        if w <= 0. || h <= 0. {
            return Ok(());
        }
        if let Some(dash) = b.border_dash.as_deref().filter(|dash| !dash.is_empty()) {
            return self.draw_dashed_box_border(b, p, dash);
        }
        // Filled edge strips retain independent widths, including partially split borders.
        for (edge, [ex, ey, ew, eh]) in [
            [x, y, w, p.borders[0]],
            [x + w - p.borders[1], y, p.borders[1], h],
            [x, y + h - p.borders[2], w, p.borders[2]],
            [x, y, p.borders[3], h],
        ]
        .into_iter()
        .enumerate()
        {
            if ew <= 0. || eh <= 0. {
                continue;
            }
            // Give horizontal edges the corners once, avoiding repeated alpha blending.
            let (ey, eh) = if edge == 1 || edge == 3 {
                (
                    ey + p.borders[0],
                    (eh - p.borders[0] - p.borders[2]).max(0.),
                )
            } else {
                (ey, eh)
            };
            if eh <= 0. {
                continue;
            }
            self.add_path_object(
                p.page_index,
                &format!("M 0 0 L {ew} 0 L {ew} {eh} L 0 {eh} C"),
                [ex, ey, ew, eh],
                PathStyle {
                    fill: true,
                    stroke: false,
                    fill_color: Some(b.border_color),
                    alpha: b.alpha,
                    ..Default::default()
                },
            )?;
        }
        Ok(())
    }

    fn draw_dashed_box_border(
        &mut self,
        b: &LayoutBox,
        p: &BoxPlacement,
        arguments: &[f64],
    ) -> Result<(), OfdError> {
        let (offset, pattern) = if arguments.len() == 1 {
            (0., vec![arguments[0]; 2])
        } else if arguments.len() % 2 == 1 {
            // Java omits offsets below 0.000001 mm.
            (
                if arguments[0] >= 1e-6 {
                    arguments[0]
                } else {
                    0.
                },
                arguments[1..].to_vec(),
            )
        } else {
            (0., arguments.to_vec())
        };
        let style = |width| PathStyle {
            fill: false,
            stroke: true,
            line_width: width,
            dash_sequence: Some(pattern.clone()),
            dash_offset: (offset != 0.).then_some(offset),
            stroke_color: Some(b.border_color),
            alpha: b.alpha,
            ..Default::default()
        };
        let [x, y, w, h] = p.border_box;
        let [top, right, bottom, left] = p.borders;
        if top > 0. && p.borders.iter().all(|width| *width == top) {
            let half = top / 2.;
            let (right, bottom) = (w - half, h - half);
            self.add_path_object(
                p.page_index,
                &format!("M {half} {half} L {right} {half} L {right} {bottom} L {half} {bottom} C"),
                [x, y, w, h],
                style(top),
            )?;
            return Ok(());
        }
        // Match Java's separate-edge geometry: horizontal edges own the corners.
        for (boundary, width, horizontal) in [
            ([x, y, w, top], top, true),
            ([x, y + h - bottom, w, bottom], bottom, true),
            ([x, y + top, left, (h - top - bottom).max(0.)], left, false),
            (
                [x + w - right, y + top, right, (h - top - bottom).max(0.)],
                right,
                false,
            ),
        ] {
            if width <= 0. || boundary[2] <= 0. || boundary[3] <= 0. {
                continue;
            }
            let half = width / 2.;
            let commands = if horizontal {
                format!("M 0 {half} L {} {half}", boundary[2])
            } else {
                format!("M {half} 0 L {half} {}", boundary[3])
            };
            self.add_path_object(p.page_index, &commands, boundary, style(width))?;
        }
        Ok(())
    }
}

pub(super) fn nested_fragment(
    source: &LayoutBox,
    page: usize,
    x: f64,
    y: f64,
    offset: f64,
    height: f64,
) -> Result<BoxPlacement, OfdError> {
    let placement = PreparedBox {
        source_index: 0,
        source,
        width: source.size[0]
            + source.margin[1]
            + source.margin[3]
            + source.border[1]
            + source.border[3]
            + source.padding[1]
            + source.padding[3],
        height: source.size[1]
            + source.margin[0]
            + source.margin[2]
            + source.border[0]
            + source.border[2]
            + source.padding[0]
            + source.padding[2],
        content_width: source.size[0],
        ranges: &[],
    }
    .fragment(page, x, y, offset, height);
    if placement
        .allocation
        .iter()
        .chain(&placement.border_box)
        .chain(&placement.padding_box)
        .chain(&placement.content_box)
        .any(|v| !v.is_finite())
    {
        return Err(OfdError::Invalid("nested box coordinates overflow".into()));
    }
    for [x, y, w, h] in [
        placement.border_box,
        placement.padding_box,
        placement.content_box,
    ] {
        if !(x + w).is_finite() || !(y + h).is_finite() {
            return Err(OfdError::Invalid("nested box bounds overflow".into()));
        }
    }
    Ok(placement)
}

pub(super) fn validate_border_dash(dash: Option<&[f64]>) -> Result<(), OfdError> {
    let Some(dash) = dash.filter(|dash| !dash.is_empty()) else {
        return Ok(());
    };
    if dash.len() > 257 {
        return Err(OfdError::Limit(
            "box border dash sequence is too long".into(),
        ));
    }
    let pattern = if dash.len() > 1 && dash.len() % 2 == 1 {
        &dash[1..]
    } else {
        dash
    };
    if dash.iter().any(|value| !value.is_finite())
        || pattern.iter().any(|value| *value <= 0.)
        || !pattern.iter().sum::<f64>().is_finite()
        || (dash.len() == 1 && !(dash[0] * 2.).is_finite())
    {
        return Err(OfdError::Invalid(
            "box border dash arguments are invalid".into(),
        ));
    }
    Ok(())
}
