use std::{env, fs};

use ofdrw::{create_ses_seal, SesSealSpec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let output = args.next().ok_or("missing output seal path")?;
    let key = fs::read(args.next().ok_or("missing issuer private key path")?)?;
    let certificate = fs::read(args.next().ok_or("missing issuer certificate path")?)?;
    let picture = fs::read(args.next().ok_or("missing picture path")?)?;
    let version: u8 = args.next().ok_or("missing SES version")?.parse()?;
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    let spec = SesSealSpec {
        version,
        vendor_id: "ofdrw-rs".into(),
        seal_id: format!("rust-generated-seal-v{version}"),
        name: "Rust SES seal".into(),
        seal_type: 3,
        authorized_signer_certificates: vec![certificate.clone()],
        created_at: 1_600_000_000,
        valid_from: 1_600_000_000,
        valid_until: 1_900_000_000,
        picture_type: "PNG".into(),
        picture_width_mm: 40,
        picture_height_mm: 40,
    };
    fs::write(
        output,
        create_ses_seal(&key, &certificate, &picture, &spec)?,
    )?;
    Ok(())
}
