use std::env;

use ofdrw::{FlowTextStyle, OfdDocument};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args().nth(1).ok_or("missing output path")?;
    let mut document = OfdDocument::new();
    document.add_flowed_text(
        0,
        " A \n B \n C \n",
        FlowTextStyle {
            page_size: [40.0, 22.0],
            margins: [5.0; 4],
            font_size: 5.0,
            line_height: 6.0,
            font_id: None,
            first_line_indent: 5.0,
            ..FlowTextStyle::default()
        },
    )?;
    document.save(output)?;
    Ok(())
}
