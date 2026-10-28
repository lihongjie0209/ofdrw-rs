use std::env;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args().nth(1).ok_or("missing output path")?;
    let mut document = OfdDocument::new();
    document.add_text_page("Rust version", 80.0, 50.0)?;
    document.add_version_snapshot(0, "rust-v1", Some("First version"))?;
    document.add_text_page("Live page", 80.0, 50.0)?;
    document.save(output)?;
    Ok(())
}
