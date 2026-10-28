use std::env;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let source = args.next().ok_or("missing source OFD path")?;
    let output = args.next().ok_or("missing output OFD path")?;
    let mut document = OfdDocument::open(source)?;
    document.duplicate_page(0)?;
    document.save(output)?;
    Ok(())
}
