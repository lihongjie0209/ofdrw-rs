use std::{env, fs};

use ofdrw::OfdDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = env::args().nth(1).ok_or("missing source OFD path")?;
    let key_path = env::args().nth(2).ok_or("missing raw SM2 key path")?;
    let output = env::args().nth(3).ok_or("missing output OFD path")?;
    let mut document = OfdDocument::open(source)?;
    let key = fs::read(key_path)?;
    document.add_digital_signature(&key, "2020-04-22 16:22:44")?;
    if env::args().nth(4).as_deref() == Some("twice") {
        document.add_digital_signature(&key, "2020-04-23 10:11:12")?;
    }
    document.save(output)?;
    Ok(())
}
