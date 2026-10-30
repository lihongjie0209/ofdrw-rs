use std::{io::Write, process::Command};

use ofdrw::{OfdDocument, OfdError};

fn pdf(objects: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        writeln!(&mut bytes, "{} 0 obj", index + 1).unwrap();
        bytes.extend_from_slice(object);
        bytes.extend_from_slice(b"\nendobj\n");
    }
    let xref = bytes.len();
    write!(
        &mut bytes,
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    )
    .unwrap();
    for offset in offsets {
        writeln!(&mut bytes, "{offset:010} 00000 n ").unwrap();
    }
    write!(
        &mut bytes,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    )
    .unwrap();
    bytes
}

fn nested_pdf(depth: usize) -> Vec<u8> {
    let mut catalog = b"<< /Type /Catalog /Pages 2 0 R /Unused ".to_vec();
    catalog.extend(std::iter::repeat_n(b'[', depth));
    catalog.push(b'0');
    catalog.extend(std::iter::repeat_n(b']', depth));
    catalog.extend_from_slice(b" >>");
    pdf(&[
        catalog,
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] /Resources << >> >>".to_vec(),
    ])
}

#[test]
fn deeply_nested_pdf_objects_return_an_error_without_aborting_the_host() {
    // A stack-overflow abort cannot be caught by catch_unwind. A subprocess
    // makes the same regression observable without killing the test runner.
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "pdf_nesting_probe", "--nocapture"])
        .env("OFDRW_PDF_DEPTH_PROBE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "probe exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn pdf_nesting_probe() {
    if std::env::var_os("OFDRW_PDF_DEPTH_PROBE").is_none() {
        return;
    }
    assert_eq!(
        OfdDocument::from_pdf_bytes(&nested_pdf(8))
            .unwrap()
            .pages()
            .unwrap()
            .len(),
        1
    );
    assert!(matches!(
        OfdDocument::from_pdf_bytes(&nested_pdf(12_000)),
        Err(OfdError::Invalid(_))
    ));
}

#[test]
fn page_decompression_budget_is_shared_by_all_content_streams() {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let chunk = vec![b' '; 1024 * 1024];
    for _ in 0..33 {
        encoder.write_all(&chunk).unwrap();
    }
    let compressed = encoder.finish().unwrap();
    let mut stream = format!(
        "<< /Length {} /Filter /FlateDecode >>\nstream\n",
        compressed.len()
    )
    .into_bytes();
    stream.extend(compressed);
    stream.extend_from_slice(b"\nendstream");
    let source = pdf(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] /Resources << >> /Contents [4 0 R 4 0 R] >>".to_vec(),
        stream,
    ]);
    assert!(source.len() < 200_000);
    assert!(
        matches!(OfdDocument::from_pdf_bytes(&source), Err(OfdError::Limit(message)) if message.contains("page content"))
    );
}
