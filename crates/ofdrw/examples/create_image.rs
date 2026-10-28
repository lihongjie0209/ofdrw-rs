use std::env;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = env::args().nth(1).ok_or("missing source path")?;
    let output = env::args().nth(2).ok_or("missing output path")?;
    let source = OfdDocument::open(source)?;
    let png = source.page_image_data(0, 0)?;
    let mut document = OfdDocument::new();
    document.add_image_page(png, "png", 210.0, 140.0)?;
    document.save(output)?;
    Ok(())
}
