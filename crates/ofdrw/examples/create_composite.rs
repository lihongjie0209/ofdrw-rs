use std::env;

use ofdrw::{OfdDocument, PathStyle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args().nth(1).ok_or("missing output path")?;
    let source = env::args().nth(2).ok_or("missing source image document")?;
    let mut document = OfdDocument::new();
    document.add_text_page("composite", 40.0, 40.0)?;
    document.add_composite_path_object(
        0,
        "M 0 0 L 10 0 L 10 10 L 0 10 C",
        [2.0, 3.0, 10.0, 10.0],
        PathStyle {
            fill: true,
            stroke: false,
            fill_color: Some([255, 0, 0]),
            ..Default::default()
        },
    )?;
    document.add_composite_text_object(
        0,
        "Rust composite text",
        [2.0, 16.0, 30.0, 8.0],
        5.0,
        Some("1"),
    )?;
    let image_document = OfdDocument::open(source)?;
    let image = image_document.page_image_data(0, 0)?;
    document.add_composite_image_object(0, image, "png", [35.0, 3.0, 20.0, 20.0])?;
    document.save(output)?;
    Ok(())
}
