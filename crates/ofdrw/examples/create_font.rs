use std::env;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = env::args().nth(1).ok_or("missing source path")?;
    let output = env::args().nth(2).ok_or("missing output path")?;
    let source = OfdDocument::open(source)?;
    let font = source.font_data(0)?;
    let mut document = OfdDocument::new();
    let id = document.add_embedded_font("宋体", font)?;
    document.add_text_page_with_font("Rust embedded font", 210.0, 297.0, id)?;
    document.save(output)?;
    Ok(())
}
