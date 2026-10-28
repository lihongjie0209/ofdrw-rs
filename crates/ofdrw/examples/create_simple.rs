use std::env;

use ofdrw::{AttachmentMetadata, DestinationInfo, OfdDocument, PathStyle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or("missing output path")?;
    let mut document = OfdDocument::new();
    document.add_text_page("你好，Rust OFD!", 210.0, 297.0)?;
    document.add_text_object(0, "positioned by Rust", [30.0, 40.0, 70.0, 8.0], 5.0, None)?;
    document.add_path_object(
        0,
        "M 0 0 L 20 10",
        [30.0, 60.0, 20.0, 10.0],
        PathStyle::default(),
    )?;
    let object_id = document.page_objects(0)?[0]
        .id
        .as_deref()
        .ok_or("page text object has no ID")?
        .to_owned();
    document.add_page_object_uri_action(0, &object_id, "https://example.org/rust?x=1&y=2")?;
    document.add_text_watermark(0, "Rust watermark", [30.0, 70.0, 60.0, 10.0], 5.0)?;
    let page_id = document.pages()?[0].id.clone();
    document.add_page_object_goto_action(
        0,
        &object_id,
        DestinationInfo {
            page_id: page_id.clone(),
            kind: "XYZ".into(),
            left: Some(12.5),
            top: Some(21.0),
            right: None,
            bottom: None,
            zoom: Some(1.5),
        },
    )?;
    let outline_root = document.add_outline(
        0,
        "Rust outline",
        None,
        Some(DestinationInfo {
            page_id: page_id.clone(),
            kind: "XYZ".into(),
            left: Some(10.0),
            top: Some(10.0),
            right: None,
            bottom: None,
            zoom: None,
        }),
        true,
    )?;
    document.add_outline(0, "Rust child", Some(outline_root), None, true)?;
    document.add_bookmark(
        0,
        "Rust bookmark",
        DestinationInfo {
            page_id,
            kind: "XYZ".into(),
            left: Some(10.0),
            top: Some(10.0),
            right: None,
            bottom: None,
            zoom: None,
        },
    )?;
    document.add_page_object_bookmark_action(0, &object_id, "Rust bookmark")?;
    document.add_attachment("note.txt", b"hello")?;
    document.set_attachment_metadata(
        0,
        AttachmentMetadata {
            format: Some("text/plain".into()),
            creation_date: Some("2024-02-29T12:30:59".into()),
            mod_date: Some("2024-03-01T08:00:00".into()),
            visible: false,
            usage: Some("source".into()),
        },
    )?;
    document.replace_attachment_data(0, b"hello updated")?;
    document.rename_attachment(0, "renamed note.txt")?;
    document.set_document_custom_data(0, "Rust key", "value & <OFD>")?;
    document.add_document_keyword(0, "Rust & OFD")?;
    document.save(path)?;
    Ok(())
}
