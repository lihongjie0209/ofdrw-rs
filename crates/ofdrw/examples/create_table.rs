use std::path::PathBuf;

use ofdrw::{OfdDocument, TableStyle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(std::env::args().nth(1).ok_or("expected output OFD path")?);
    let mut document = OfdDocument::new();
    let style = TableStyle {
        page_size: [30.0, 26.0],
        margins: [2.0; 4],
        column_widths: vec![12.0, 12.0],
        row_height: 8.0,
        font_size: 3.0,
        line_height: 3.0,
        padding: 1.0,
        repeat_header: true,
        ..TableStyle::default()
    };
    let rows = vec![
        vec!["H1".into(), "H2".into()],
        vec!["A".into(), "B".into()],
        vec!["C".into(), "D".into()],
    ];
    document.add_table(0, &rows, style)?;
    let auto_style = TableStyle {
        page_size: [30.0, 26.0],
        margins: [2.0; 4],
        column_widths: vec![12.0, 12.0],
        row_height: 0.0,
        font_size: 3.0,
        line_height: 3.0,
        padding: 1.0,
        repeat_header: true,
        ..TableStyle::default()
    };
    let auto_rows = vec![
        vec!["H1".into(), "H2".into()],
        vec!["A\nA2".into(), "".into()],
        vec!["B\nB2\nB3".into(), "".into()],
    ];
    document.add_table(0, &auto_rows, auto_style)?;
    document.save(path)?;
    Ok(())
}
