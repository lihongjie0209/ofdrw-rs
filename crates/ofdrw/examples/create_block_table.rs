use std::env;

use ofdrw::{FlowTextAlign, OfdDocument, TableCell, TableCellBlock, TableCellImage, TableStyle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args().nth(1).ok_or("missing output path")?;
    let mut source = OfdDocument::new();
    source.add_blank_page(10.0, 10.0)?;
    let png = source.page_png(0, 25.4)?;
    let image = |width, height| {
        TableCellBlock::Image(TableCellImage {
            data: png.clone(),
            format: "png".into(),
            width,
            height,
        })
    };
    let cell = TableCell {
        row: 0,
        column: 0,
        row_span: 1,
        column_span: 1,
        text: String::new(),
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
        align: FlowTextAlign::Center,
        vertical_align: ofdrw::TableVerticalAlign::Top,
        image: None,
        blocks: vec![
            TableCellBlock::Text {
                text: "A".into(),
                color: None,
                align: None,
            },
            image(6.0, 4.0),
            TableCellBlock::Text {
                text: "B".into(),
                color: Some([4, 5, 6]),
                align: Some(FlowTextAlign::Right),
            },
            image(8.0, 5.0),
        ],
    };
    let style = TableStyle {
        page_size: [40.0, 40.0],
        margins: [2.0; 4],
        column_widths: vec![20.0],
        row_height: 0.0,
        font_size: 3.0,
        line_height: 3.0,
        padding: 1.0,
        ..TableStyle::default()
    };
    let mut document = OfdDocument::new();
    document.add_table_cells(0, 1, &[cell], style)?;
    document.save(output)?;
    Ok(())
}
