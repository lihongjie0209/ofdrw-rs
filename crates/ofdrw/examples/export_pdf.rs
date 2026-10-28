use std::error::Error;
use std::path::PathBuf;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let source = args
        .next()
        .ok_or("usage: export_pdf INPUT.ofd OUTPUT.pdf")?;
    let target = args
        .next()
        .ok_or("usage: export_pdf INPUT.ofd OUTPUT.pdf")?;
    if args.next().is_some() {
        return Err("usage: export_pdf INPUT.ofd OUTPUT.pdf".into());
    }
    let document = OfdDocument::open(PathBuf::from(source))?;
    std::fs::write(PathBuf::from(target), document.to_pdf()?)?;
    Ok(())
}
