use std::env;

use ofdrw::{OfdDocument, TableCell, TableStyle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args().nth(1).ok_or("missing output path")?;
    let mut document = OfdDocument::new();
    let style = TableStyle {
        page_size: [30.0, 23.0],
        margins: [2.0; 4],
        column_widths: vec![12.0, 12.0],
        row_height: 0.0,
        font_size: 3.0,
        line_height: 3.0,
        padding: 1.0,
        repeat_header: true,
        ..TableStyle::default()
    };
    let cell = |row, column, row_span, column_span, text: &str| TableCell {
        row,
        column,
        row_span,
        column_span,
        text: text.into(),
        background_color: None,
        text_color: None,
        border_width: None,
        border_color: None,
        padding: None,
        font_id: None,
        font_size: None,
        line_height: None,
        letter_spacing: None,
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
        align: ofdrw::FlowTextAlign::Left,
        vertical_align: ofdrw::TableVerticalAlign::Top,
        image: None,
        blocks: Vec::new(),
    };
    let cells = [
        cell(0, 0, 1, 1, "H1"),
        cell(0, 1, 1, 1, "H2"),
        cell(1, 0, 2, 1, "L1\nL2\nL3\nL4"),
        cell(1, 1, 1, 1, "Top"),
        cell(2, 1, 1, 1, "Low"),
        cell(3, 0, 1, 2, "Wide"),
    ];
    document.add_table_cells(0, 4, &cells, style)?;
    document.save(output)?;
    Ok(())
}
