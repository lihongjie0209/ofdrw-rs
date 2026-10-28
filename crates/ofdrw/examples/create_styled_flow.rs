use std::env;

use ofdrw::{FlowSpan, FlowTextStyle, OfdDocument};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args().nth(1).ok_or("missing output path")?;
    let mut document = OfdDocument::new();
    let mut red = FlowSpan::new("AA");
    red.bold = true;
    red.weight = Some(600);
    red.fill_color = Some([255, 0, 0]);
    red.letter_spacing = 0.5;
    red.underline = true;
    red.underline_offset = 0.5;
    red.underline_width = 0.2;
    red.line_break_after = true;
    let mut blue = FlowSpan::new("B\nC");
    blue.font_size = Some(3.0);
    blue.italic = true;
    blue.fill = false;
    blue.stroke = true;
    blue.stroke_color = Some([0, 0, 255]);
    blue.stroke_width = Some(0.2);
    blue.keep_together = true;
    document.add_flowed_spans(
        0,
        &[red, blue],
        FlowTextStyle {
            page_size: [40.0, 16.0],
            margins: [5.0; 4],
            ..FlowTextStyle::default()
        },
    )?;
    document.save(output)?;
    Ok(())
}
