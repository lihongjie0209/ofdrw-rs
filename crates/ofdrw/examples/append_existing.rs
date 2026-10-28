use std::env;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = env::args().nth(1).ok_or("missing source path")?;
    let output = env::args().nth(2).ok_or("missing output path")?;
    let image_source = env::args().nth(3).ok_or("missing image source path")?;
    let font_source = env::args().nth(4).ok_or("missing font source path")?;
    let mut document = OfdDocument::open(source)?;
    let original_id = document
        .page_text_objects(0)?
        .first()
        .and_then(|object| object.id.as_deref())
        .ok_or("original text object has no ID")?
        .to_owned();
    document.remove_page_object(0, &original_id)?;
    let font = OfdDocument::open(font_source)?;
    let font_id =
        document.add_embedded_font_to_document(0, "Rust embedded font", font.font_data(0)?)?;
    document.append_text_page_with_font(0, "Rust added page", 210.0, 297.0, font_id)?;
    document.add_text_object(
        1,
        "Rust font on existing document",
        [20.0, 35.0, 100.0, 10.0],
        5.0,
        Some(&font_id.to_string()),
    )?;
    document.append_path_page(
        0,
        "M 0 0 L 30 0 L 30 20 C",
        [10.0, 10.0, 30.0, 20.0],
        [210.0, 297.0],
        true,
        false,
    )?;
    let image = OfdDocument::open(image_source)?;
    document.append_image_page(0, image.page_image_data(0, 0)?, "png", [80.0, 60.0])?;
    document.add_image_object(
        0,
        image.page_image_data(0, 0)?,
        "png",
        [10.0, 40.0, 20.0, 20.0],
    )?;
    document.add_attachment_to_document(0, "rust-note.txt", b"Rust existing attachment")?;
    document.save(output)?;
    Ok(())
}
