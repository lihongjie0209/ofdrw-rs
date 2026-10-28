use std::env;

use ofdrw::{OfdDocument, PathStyle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args().nth(1).ok_or("missing output path")?;
    let mut document = OfdDocument::new();
    document.add_blank_page(40.0, 30.0)?;
    let original = document.add_path_object(
        0,
        "M 0 0 L 20 0",
        [5.0, 5.0, 20.0, 10.0],
        PathStyle {
            dash_sequence: Some(vec![1.0, 2.0, 3.0, 4.0]),
            ..PathStyle::default()
        },
    )?;
    document.duplicate_page_object(0, &original)?;
    document.save(output)?;
    Ok(())
}
