use std::{env, path::PathBuf};

use ofdrw::{FlowTextStyle, OfdDocument};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args().nth(1).ok_or("missing output path")?;
    let mode = env::args().nth(2);
    let arabic = matches!(mode.as_deref(), Some("rtl" | "mixed"));
    let font_name = if arabic {
        "DejaVu Sans"
    } else {
        "DejaVu Serif"
    };
    let text = match mode.as_deref() {
        Some("rtl") => "سلام",
        Some("mixed") => "AسلامB",
        _ => "AfiB",
    };
    let file = if arabic {
        "dejavu-sans.ttf"
    } else {
        "dejavu-serif.ttf"
    };
    let font = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(file),
    )?;
    let mut document = OfdDocument::new();
    let font_id = document.add_embedded_font(font_name, &font)?;
    if mode.as_deref() == Some("flow") {
        document.add_flowed_text(
            0,
            "fifi",
            FlowTextStyle {
                page_size: [14.5, 22.0],
                margins: [5.0; 4],
                font_size: 5.0,
                line_height: 6.0,
                font_id: Some(font_id.to_string()),
                shape: true,
                ..FlowTextStyle::default()
            },
        )?;
    } else {
        document.add_blank_page(40.0, 20.0)?;
        document.add_shaped_text_object(
            0,
            text,
            [5.0, 5.0, 30.0, 10.0],
            6.0,
            &font_id.to_string(),
        )?;
    }
    document.save(output)?;
    Ok(())
}
