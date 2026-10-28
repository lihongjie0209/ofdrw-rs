use std::{env, fs};

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = args.next().ok_or("missing input PDF path")?;
    let output = args.next().ok_or("missing output OFD path")?;
    let document = OfdDocument::from_pdf_bytes(&fs::read(input)?)?;
    document.save(output)?;
    Ok(())
}
