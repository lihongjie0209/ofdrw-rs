use std::env;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let output = args.next().ok_or("missing new document output path")?;
    let source = args.next().ok_or("missing existing document input path")?;
    let existing_output = args.next().ok_or("missing existing document output path")?;

    let mut document = OfdDocument::new();
    document.add_blank_page(40.0, 30.0)?;
    document.save(output)?;

    let mut existing = OfdDocument::open(source)?;
    existing.append_blank_page(0, 40.0, 30.0)?;
    existing.save(existing_output)?;
    Ok(())
}
