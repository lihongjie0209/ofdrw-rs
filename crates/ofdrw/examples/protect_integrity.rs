use std::error::Error;
use std::fs;

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args().nth(1).ok_or("expected output OFD path")?;
    let key = include_bytes!("../tests/fixtures/user-private-key.bin");
    let certificate = include_bytes!("../tests/fixtures/user-cert.der");
    let mut document = OfdDocument::new();
    document.add_text_page("Rust integrity", 100.0, 60.0)?;
    document.protect_integrity_sm2(key, certificate)?;
    fs::write(output, document.to_bytes()?)?;
    Ok(())
}
