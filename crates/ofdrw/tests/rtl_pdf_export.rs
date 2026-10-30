mod common;
#[path = "common/pdf_text.rs"]
mod pdf_text;
use ofdrw::OfdDocument;

fn document(text: &str) -> OfdDocument {
    let mut doc = OfdDocument::new();
    let font = doc
        .add_embedded_font("DejaVu Sans", include_bytes!("fixtures/dejavu-sans.ttf"))
        .unwrap();
    doc.add_blank_page(120., 30.).unwrap();
    doc.add_shaped_text_object(0, text, [5., 5., 110., 20.], 6., &font.to_string())
        .unwrap();
    doc
}

fn plain(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !c.is_whitespace() && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .collect()
}

fn check_document(text: &str, doc: OfdDocument, mode: Option<bool>) {
    assert_eq!(doc.page_text(0).unwrap(), [text]);
    let svg = doc.page_svg(0).unwrap();
    assert!(svg.contains(text));
    assert!(!svg.contains("data-ofd-rtl-semantic"));
    let pdf = doc.to_pdf().unwrap();
    let decoded = pdf_text::extract(&pdf).replace('\n', "");
    let bidi = unicode_bidi::BidiInfo::new(text, Some(unicode_bidi::Level::ltr()));
    let expected = bidi.reorder_line(&bidi.paragraphs[0], 0..text.len());
    assert_eq!(decoded, expected, "{text}: native visual character order");
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(0).unwrap(), [text]);
    assert_eq!(
        pdf_text::extract(&reopened.to_pdf().unwrap()),
        pdf_text::extract(&pdf)
    );
    assert!(reopened.validate_references().unwrap().is_empty());
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("export.pdf");
    std::fs::write(&input, &pdf).unwrap();
    if let (Some(reader), Some(raw)) = (std::env::var_os("OFDRW_PDFTOTEXT"), mode) {
        let mut command = std::process::Command::new(reader);
        if raw {
            command.arg("-raw");
        }
        let result = command
            .args(["-enc", "UTF-8"])
            .arg(&input)
            .arg("-")
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        let actual = String::from_utf8(result.stdout).unwrap();
        assert_eq!(plain(&actual), plain(text), "{text}: Poppler {actual:?}");
    }
    if let Some(gs) = std::env::var_os("OFDRW_GHOSTSCRIPT") {
        let target = temp.path().join("export.png");
        let result = std::process::Command::new(gs)
            .args([
                "-q",
                "-dSAFER",
                "-dBATCH",
                "-dNOPAUSE",
                "-sDEVICE=png16m",
                "-r144",
                "-dGraphicsAlphaBits=1",
            ])
            .arg(format!("-sOutputFile={}", target.display()))
            .arg(&input)
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        let rendered = image::open(target).unwrap().to_rgb8();
        let original = image::load_from_memory(&doc.page_png(0, 144.).unwrap())
            .unwrap()
            .to_rgb8();
        common::assert_raster_matches(&rendered, &original, text);
    }
}
macro_rules! rtl_case {
    ($name:ident, $text:literal, $raw:expr) => {
        #[test]
        fn $name() {
            check_document($text, document($text), Some($raw));
        }
    };
}
rtl_case!(arabic_lam_alef, "سلام", false);
rtl_case!(lam_alef_only, "لا", false);
rtl_case!(lam_alef_hamza_above, "لأ", false);
rtl_case!(lam_alef_hamza_below, "لإ", false);
rtl_case!(lam_alef_madda, "لآ", false);
rtl_case!(arabic_words, "السلام عليكم", false);
rtl_case!(repeated_ligatures, "سلام لا سلام", false);
rtl_case!(latin_arabic_raw, "AسلامB", true);
rtl_case!(latin_hebrew_raw, "hello שלום world", true);

#[test]
fn numeric_mixed_runs_keep_unicode_and_original_appearance() {
    let text = "abc 123 אבג 456 xyz";
    check_document(text, document(text), None);
}

#[test]
fn scaled_rotated_semantics_keep_unicode_and_original_appearance() {
    let text = "سلام لا سلام";
    let mut doc = document(text);
    let path = doc.pages().unwrap()[0].path.clone();
    let xml = String::from_utf8(doc.entry(&path).unwrap().unwrap().to_vec()).unwrap();
    let xml = xml.replace(
        "Size=\"6\"",
        "Size=\"6\" HScale=\"1.3\" CharDirection=\"90\"",
    );
    doc.set_entry(&path, xml.into_bytes()).unwrap();
    check_document(text, doc, None);
}
