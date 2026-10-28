use std::error::Error;
use std::fs;

use ofdrw::{OfdDocument, PasswordRecipient};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let source = args.next().ok_or("usage: encrypt_password SOURCE OUTPUT")?;
    let output = args.next().ok_or("missing OUTPUT")?;
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let document = OfdDocument::from_bytes(&fs::read(source)?)?;
    let encrypted = document.encrypt_with_passwords(&[
        PasswordRecipient {
            username: "alice".into(),
            password: "first".into(),
            user_type: None,
        },
        PasswordRecipient {
            username: "bob".into(),
            password: "second".into(),
            user_type: None,
        },
    ])?;
    fs::write(output, encrypted.to_bytes()?)?;
    Ok(())
}
