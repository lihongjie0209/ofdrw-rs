use std::{env, fs};

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let output = args.next().ok_or("missing output OFD path")?;
    let key = fs::read(args.next().ok_or("missing SM2 key path")?)?;
    let certificate = fs::read(args.next().ok_or("missing signer certificate path")?)?;
    let seal = fs::read(args.next().ok_or("missing signed seal path")?)?;
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    let mut document = OfdDocument::new();
    document.add_text_page("Rust SES signature", 210.0, 297.0)?;
    document.add_ses_signature_with_stamp(
        &key,
        &certificate,
        &seal,
        "2020-09-13 12:26:40",
        0,
        [10.0, 20.0, 40.0, 40.0],
    )?;
    document.save(output)?;
    Ok(())
}
