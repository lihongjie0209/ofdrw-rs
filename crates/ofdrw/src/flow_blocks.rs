use super::*;

const MAX_FLOW_BLOCKS: usize = 10_000;
const MAX_FLOW_PAGES: usize = 10_000;

/// Shared page geometry, default typography, and spacing for ordered content.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlowDocumentStyle {
    /// Supplies page size, margins, an optional start, and default text settings.
    pub text: FlowTextStyle,
    /// Millimeters between adjacent content blocks; omitted at a new page top.
    pub block_gap: f64,
}

/// Optional paragraph overrides. Page geometry and continuation stay shared.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlowParagraphStyle {
    pub font_id: Option<String>,
    pub font_size: Option<f64>,
    pub line_height: Option<f64>,
    pub align: Option<FlowTextAlign>,
    pub paragraph_gap: Option<f64>,
    pub keep_paragraphs_together: Option<bool>,
    pub first_line_indent: Option<f64>,
    pub rest_line_indent: Option<f64>,
    pub first_line_indent_chars: Option<usize>,
    pub indent_each_paragraph: Option<bool>,
    pub shape: Option<bool>,
    pub language: Option<String>,
}

impl FlowParagraphStyle {
    fn apply(&self, base: &FlowTextStyle) -> FlowTextStyle {
        let mut style = base.clone();
        if let Some(size) = self.font_size {
            style.font_size = size;
            style.line_height = size + base.line_height - base.font_size;
        }
        macro_rules! override_value {
            ($($field:ident),* $(,)?) => { $(if let Some(value) = self.$field { style.$field = value; })* };
        }
        override_value!(
            line_height,
            align,
            paragraph_gap,
            keep_paragraphs_together,
            first_line_indent,
            rest_line_indent,
            first_line_indent_chars,
            indent_each_paragraph,
            shape
        );
        if let Some(font) = &self.font_id {
            style.font_id = Some(font.clone());
        }
        if self.shape == Some(false) {
            style.language = None;
        }
        if let Some(language) = &self.language {
            style.language = Some(language.clone());
        }
        style
    }
}

/// Ordered blocks in a vertical document flow. Measurements are millimeters.
#[derive(Debug, Clone, PartialEq)]
pub enum FlowBlock {
    /// Styled spans with optional paragraph typography overrides.
    Paragraph {
        spans: Vec<FlowSpan>,
        style: Option<Box<FlowParagraphStyle>>,
    },
    /// An indivisible image with explicit size and horizontal alignment.
    Image {
        image: TableCellImage,
        align: FlowTextAlign,
    },
    /// A paginated grid; nested tables inside its cells remain indivisible.
    Table {
        table: NestedTable,
        repeat_header: bool,
    },
    /// Explicit positive vertical space, which can span pages.
    Spacer { height: f64 },
    /// Append a blank page and continue there. Consecutive breaks retain blank pages.
    PageBreak,
}

impl FlowBlock {
    /// Construct a paragraph using the document's default text style.
    pub fn paragraph(text: impl Into<String>) -> Self {
        Self::Paragraph {
            spans: vec![FlowSpan::new(text)],
            style: None,
        }
    }
}

/// Used global page indexes and a position suitable for a subsequent flow call.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowLayoutResult {
    /// Includes pages consumed by spacers and explicit blank-page breaks.
    pub pages: Vec<usize>,
    /// Relative to the last page's canvas top; `None` if no page was selected.
    pub next: Option<FlowStart>,
}

pub(super) struct FlowPlacement {
    pub(super) pages: Vec<usize>,
    pub(super) cursor: FlowPageCursor,
}

impl FlowPageCursor {
    fn continuation(&self) -> Option<FlowStart> {
        self.page.map(|page_index| FlowStart {
            page_index,
            y: (self.y - self.origin[1]).min(self.page_size[1] - self.margins[2]),
        })
    }
}

fn extend_pages(pages: &mut Vec<usize>, used: Vec<usize>) {
    for page in used {
        if pages.last() != Some(&page) {
            pages.push(page);
        }
    }
}

impl OfdDocument {
    /// Lay out paragraphs, images, tables, spacers, and page breaks in order.
    ///
    /// Content continues from `style.text.start`, or starts on a new page.
    /// Paragraphs and tables paginate using their normal wrapping rules; images
    /// must fit a complete content area. Returns the position after the final
    /// block, including blank text lines and explicit space. The complete call
    /// is atomic: an invalid later block leaves the document unchanged.
    ///
    /// At most 10,000 blocks and 10,000 used pages are accepted per call.
    ///
    /// ```
    /// use ofdrw::{FlowBlock, FlowDocumentStyle, OfdDocument};
    /// let mut document = OfdDocument::new();
    /// let mut style = FlowDocumentStyle::default();
    /// let result = document.add_flow_blocks(0, &[
    ///     FlowBlock::paragraph("First paragraph"),
    ///     FlowBlock::Spacer { height: 4. },
    ///     FlowBlock::paragraph("Second paragraph"),
    /// ], style.clone())?;
    /// style.text.start = result.next;
    /// document.add_flow_blocks(0, &[FlowBlock::paragraph("Continued")], style)?;
    /// # Ok::<(), ofdrw::OfdError>(())
    /// ```
    pub fn add_flow_blocks(
        &mut self,
        document_index: usize,
        blocks: &[FlowBlock],
        style: FlowDocumentStyle,
    ) -> Result<FlowLayoutResult, OfdError> {
        if blocks.len() > MAX_FLOW_BLOCKS {
            return Err(OfdError::Limit("too many flow blocks".into()));
        }
        let [width, height] = style.text.page_size;
        let [top, right, bottom, left] = style.text.margins;
        let content_width = width - left - right;
        let content_height = height - top - bottom;
        if style
            .text
            .page_size
            .into_iter()
            .chain(style.text.margins)
            .chain([style.block_gap, content_width, content_height])
            .any(|v| !v.is_finite())
            || width <= 0.
            || height <= 0.
            || style.text.margins.iter().any(|v| *v < 0.)
            || content_width <= 0.
            || content_height <= 0.
            || style.block_gap < 0.
        {
            return Err(OfdError::Invalid(
                "flow document geometry is invalid".into(),
            ));
        }
        if document_index >= self.doc_roots()?.len() {
            return Err(OfdError::Invalid("document index is out of bounds".into()));
        }
        self.ensure_unsigned_document(document_index)?;
        let mut cursor = FlowPageCursor::new(
            self,
            document_index,
            style.text.page_size,
            style.text.margins,
            style.text.start,
        )?;
        let mut staged = self.clone();
        let mut pages = Vec::new();
        let mut has_content = false;
        for block in blocks {
            let gap = if has_content { style.block_gap } else { 0. };
            match block {
                FlowBlock::Paragraph {
                    spans,
                    style: paragraph,
                } => {
                    // The formatter reports only pages receiving actual lines.
                    cursor.place(&mut staged, document_index, 0., gap, &mut Vec::new())?;
                    let mut text = paragraph.as_deref().map_or_else(
                        || style.text.clone(),
                        |paragraph| paragraph.apply(&style.text),
                    );
                    text.start = cursor.continuation();
                    let placement =
                        staged.add_flowed_spans_in_place(document_index, spans, text)?;
                    extend_pages(&mut pages, placement.pages);
                    cursor = placement.cursor;
                    has_content = true;
                }
                FlowBlock::Image { image, align } => {
                    if !image.width.is_finite()
                        || !image.height.is_finite()
                        || image.width <= 0.
                        || image.height <= 0.
                        || image.width > content_width
                        || image.height > content_height
                    {
                        return Err(OfdError::Invalid(
                            "flow image exceeds the content area".into(),
                        ));
                    }
                    let (page, origin_x, y) =
                        cursor.place(&mut staged, document_index, image.height, gap, &mut pages)?;
                    let free = content_width - image.width;
                    let offset = match align {
                        FlowTextAlign::Left => 0.,
                        FlowTextAlign::Center => free / 2.,
                        FlowTextAlign::Right => free,
                    };
                    staged.add_image_object(
                        page,
                        &image.data,
                        &image.format,
                        [origin_x + left + offset, y, image.width, image.height],
                    )?;
                    cursor.y += image.height;
                    has_content = true;
                }
                FlowBlock::Table {
                    table,
                    repeat_header,
                } => {
                    cursor.place(&mut staged, document_index, 0., gap, &mut Vec::new())?;
                    let table_style = TableStyle {
                        start: cursor.continuation(),
                        page_size: style.text.page_size,
                        margins: style.text.margins,
                        column_widths: table.column_widths.clone(),
                        row_height: table.row_height,
                        font_size: style.text.font_size,
                        line_height: style.text.line_height,
                        padding: table.padding,
                        border_width: table.border_width,
                        border_color: table.border_color,
                        font_id: style.text.font_id.clone(),
                        repeat_header: *repeat_header,
                    };
                    let placement = staged.add_table_cells_in_place(
                        document_index,
                        table.row_count,
                        &table.cells,
                        table_style,
                    )?;
                    extend_pages(&mut pages, placement.pages);
                    cursor = placement.cursor;
                    has_content = true;
                }
                FlowBlock::Spacer { height } => {
                    if !height.is_finite() || *height <= 0. {
                        return Err(OfdError::Invalid("flow spacer height is invalid".into()));
                    }
                    if (*height / content_height).ceil() > MAX_FLOW_PAGES as f64 {
                        return Err(OfdError::Limit("flow spacer spans too many pages".into()));
                    }
                    let mut remaining = *height;
                    while remaining > 0. {
                        cursor.place(&mut staged, document_index, 0., 0., &mut Vec::new())?;
                        let available =
                            cursor.origin[1] + style.text.page_size[1] - bottom - cursor.y;
                        if available <= 0. {
                            cursor.page = None;
                            continue;
                        }
                        let consumed = remaining.min(available);
                        cursor.place(&mut staged, document_index, consumed, 0., &mut pages)?;
                        cursor.y += consumed;
                        remaining -= consumed;
                        if pages.len() > MAX_FLOW_PAGES {
                            return Err(OfdError::Limit("flow spans too many pages".into()));
                        }
                    }
                }
                FlowBlock::PageBreak => {
                    cursor.page = None;
                    cursor.place(&mut staged, document_index, 0., 0., &mut pages)?;
                    has_content = false;
                }
            }
            if pages.len() > MAX_FLOW_PAGES {
                return Err(OfdError::Limit("flow spans too many pages".into()));
            }
        }
        let result = FlowLayoutResult {
            pages,
            next: cursor.continuation(),
        };
        *self = staged;
        Ok(result)
    }
}
