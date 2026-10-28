use std::error::Error;
use std::fs;

use ofdrw::{CertificateRecipient, EncryptionRecipient, OfdDocument, PasswordRecipient};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let source = args
        .next()
        .ok_or("usage: encrypt_mixed SOURCE CERT_DER OUTPUT")?;
    let certificate = args.next().ok_or("missing CERT_DER")?;
    let output = args.next().ok_or("missing OUTPUT")?;
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let document = OfdDocument::from_bytes(&fs::read(source)?)?;
    let recipients = [
        EncryptionRecipient::Password(PasswordRecipient {
            username: "rust-password-user".into(),
            password: "rust-mixed-pass".into(),
            user_type: None,
        }),
        EncryptionRecipient::Certificate(CertificateRecipient {
            username: "rust-cert-user".into(),
            certificate_der: fs::read(certificate)?,
            user_type: None,
        }),
    ];
    let encrypted = document.encrypt_with_recipients(&recipients)?;
    fs::write(output, encrypted.to_bytes()?)?;
    Ok(())
}
