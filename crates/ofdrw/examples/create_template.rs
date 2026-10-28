use std::path::PathBuf;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(std::env::args().nth(1).ok_or("expected output OFD path")?);
    let mut document = OfdDocument::new();
    document.add_text_page("shared template", 30.0, 20.0)?;
    document.add_blank_page(30.0, 20.0)?;
    document.promote_page_to_template(0, &[1], "Background")?;
    document.save(path)?;
    Ok(())
}
