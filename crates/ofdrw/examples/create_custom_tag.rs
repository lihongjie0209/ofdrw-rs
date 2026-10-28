use std::env;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or("missing output path")?;
    let mut document = OfdDocument::new();
    document.add_blank_page(40.0, 30.0)?;
    document.add_custom_tag_to_document(
        0,
        "invoice",
        Some("urn:example:invoice"),
        None,
        b"<Invoice Version=\"1\"/>",
    )?;
    document.update_custom_tag(0, "invoice-v2", Some("urn:example:invoice"), None)?;
    document.replace_custom_tag_data(0, b"<Invoice Version=\"2\"/>")?;
    document.save(path)?;
    Ok(())
}
