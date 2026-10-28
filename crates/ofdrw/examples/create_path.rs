use std::env;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args().nth(1).ok_or("missing output path")?;
    let mut document = OfdDocument::new();
    document.add_path_page(
        "M 0 0 L 30 0 L 30 20 L 0 20 C",
        [10.0, 10.0, 30.0, 20.0],
        210.0,
        297.0,
        true,
        false,
    )?;
    document.save(output)?;
    Ok(())
}
