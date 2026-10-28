use std::env;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or("missing output path")?;
    let mut document = OfdDocument::new();
    document.add_text_page("base", 80.0, 50.0)?;
    document.add_text_stamp_annotation(0, "Rust stamp", [10.0, 20.0, 60.0, 10.0], 5.0)?;
    document.save(path)?;
    Ok(())
}
