mod common;
#[path = "common/pdf_text.rs"]
mod pdf_text;
use ofdrw::OfdDocument;
use std::path::Path;

fn coverage(image: &image::RgbImage) -> f64 {
    image
        .pixels()
        .map(|p| p.0.iter().map(|v| 1. - f64::from(*v) / 255.).sum::<f64>() / 3.)
        .sum()
}

fn check(name: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdf-text-affine");
    let reference: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("reference.json")).unwrap()).unwrap();
    let case = reference["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap();
    let doc = OfdDocument::from_pdf_bytes(
        &std::fs::read(root.join(case["file"].as_str().unwrap())).unwrap(),
    )
    .unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(
        doc.page_text(0).unwrap().join(""),
        case["text"].as_str().unwrap()
    );
    assert!(
        doc.page_images(0).unwrap().is_empty(),
        "{name} rasterized text"
    );
    assert!(
        !doc.page_paths(0).unwrap().is_empty(),
        "{name} omitted vector pen outlines"
    );
    assert!(doc.validate_references().unwrap().is_empty());
    let matrix: Vec<f64> = case["global_matrix"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let expected_pen = [matrix[0], -matrix[1], -matrix[2], matrix[3], 0., 0.];
    let styles = doc.page_object_stroke_styles(0).unwrap();
    for path in doc.page_paths(0).unwrap() {
        let style = styles.iter().find(|style| style.id == path.id).unwrap();
        if !style.stroke_enabled {
            continue;
        }
        assert!(
            (style.style.line_width - case["line_width_pt"].as_f64().unwrap() * 25.4 / 72.).abs()
                < 1e-6
        );
        if name == "dashed" {
            let dash = style.style.dash_sequence.as_ref().unwrap();
            assert_eq!(dash.len(), 2);
            for (actual, expected) in dash.iter().zip([3., 2.]) {
                assert!((actual - expected * 25.4 / 72.).abs() < 1e-7);
            }
            assert!((style.style.dash_offset - 25.4 / 72.).abs() < 1e-7);
        }
        if case["line_width_pt"].as_f64().unwrap() > 0. {
            let actual = path.ctm.unwrap_or([1., 0., 0., 1., 0., 0.]);
            for (a, b) in actual.into_iter().zip(expected_pen) {
                assert!((a - b).abs() < 1e-7, "{name}: pen matrix");
            }
        }
    }

    for dpi in [72., 288., 576.] {
        let actual = image::load_from_memory(&doc.page_png(0, dpi).unwrap())
            .unwrap()
            .to_rgb8();
        let expected =
            image::open(root.join(case["references"][format!("{dpi:.0}")].as_str().unwrap()))
                .unwrap()
                .to_rgb8();
        if name.ends_with("hairline") || dpi == 72. {
            if name.ends_with("hairline") {
                let ratio = coverage(&actual) / coverage(&expected);
                assert!(
                    (ratio - 1.).abs() < 0.12,
                    "{name} at {dpi} DPI: coverage ratio {ratio}"
                );
            }
            for (source, target) in [(&actual, &expected), (&expected, &actual)] {
                let mut ink = 0usize;
                let mut unmatched = 0usize;
                let mut ink_energy = 0.;
                let mut unmatched_energy = 0.;
                for y in 1..source.height() - 1 {
                    for x in 1..source.width() - 1 {
                        if source.get_pixel(x, y).0.iter().all(|v| *v >= 250) {
                            continue;
                        }
                        ink += 1;
                        let energy = source
                            .get_pixel(x, y)
                            .0
                            .iter()
                            .map(|v| 1. - f64::from(*v) / 255.)
                            .sum::<f64>()
                            / 3.;
                        ink_energy += energy;
                        if !(y - 1..=y + 1).any(|yy| {
                            (x - 1..=x + 1)
                                .any(|xx| target.get_pixel(xx, yy).0.iter().any(|v| *v < 250))
                        }) {
                            unmatched += 1;
                            unmatched_energy += energy;
                        }
                    }
                }
                // Subpixel hairline fringes carry partial coverage; compare
                // misplaced ink energy against the aliased one-pixel reference.
                let misplaced = if name.ends_with("hairline") {
                    unmatched_energy / ink_energy
                } else {
                    unmatched as f64 / ink as f64
                };
                assert!(
                    ink > 100 && misplaced < 0.03,
                    "{name}, {dpi}: {unmatched}/{ink} pixels misplaced"
                );
            }
        } else {
            common::assert_raster_matches(&actual, &expected, name);
        }
    }
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_text(0).unwrap(), doc.page_text(0).unwrap());
    assert_eq!(
        reopened.page_png(0, 144.).unwrap(),
        doc.page_png(0, 144.).unwrap()
    );
    let exported = reopened.to_pdf().unwrap();
    assert!(lopdf::Document::load_mem(&exported).is_ok());
    let extracted = pdf_text::extract(&exported);
    assert!(
        extracted
            .split_whitespace()
            .collect::<String>()
            .contains(case["text"].as_str().unwrap()),
        "{name} lost PDF text semantics: {extracted:?}"
    );
}

#[test]
fn scale() {
    check("scale");
}
#[test]
fn shear() {
    check("shear");
}
#[test]
fn reflection() {
    check("reflection");
}
#[test]
fn text_matrix() {
    check("text-matrix");
}
#[test]
fn horizontal_scale() {
    check("horizontal-scale");
}
#[test]
fn fill_stroke() {
    check("fill-stroke");
}
#[test]
fn clip() {
    check("clip");
}
#[test]
fn hairline() {
    check("hairline");
}

#[test]
fn cff_shear() {
    check("cff-shear");
}

fn fallback_pdf(mode: u8) -> Vec<u8> {
    use lopdf::{dictionary, Object};
    let mut pdf = lopdf::Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font =
        pdf.add_object(dictionary! { "Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica" });
    let stream = pdf.add_object(lopdf::Stream::new(
        dictionary! {},
        format!("q 2 0.2 0.3 0.7 10 10 cm 1.5 w BT /F1 24 Tf {mode} Tr 10 60 Td (ABC) Tj ET Q")
            .into_bytes(),
    ));
    let page = pdf.add_object(dictionary! { "Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),200.into(),160.into()],
        "Resources"=>dictionary! { "Font"=>dictionary! {"F1"=>font}}, "Contents"=>stream });
    pdf.objects.insert(
        pages,
        Object::Dictionary(dictionary! {"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}),
    );
    let catalog = pdf.add_object(dictionary! { "Type"=>"Catalog", "Pages"=>pages });
    pdf.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn affine_text_strokes_support_unembedded_font_substitution_and_semantics() {
    let doc = OfdDocument::from_pdf_bytes(&fallback_pdf(1)).unwrap();
    assert_eq!(doc.page_text(0).unwrap().join(""), "ABC");
    assert!(doc.page_paths(0).unwrap().len() >= 3);
    assert!(doc.page_images(0).unwrap().is_empty());
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(
        doc.page_png(0, 144.).unwrap(),
        reopened.page_png(0, 144.).unwrap()
    );
    assert!(pdf_text::extract(&doc.to_pdf().unwrap())
        .split_whitespace()
        .collect::<String>()
        .contains("ABC"));
}

#[test]
fn nonpainting_pdf_text_remains_selectable_when_exported_again() {
    let doc = OfdDocument::from_pdf_bytes(&fallback_pdf(3)).unwrap();
    assert_eq!(doc.page_text(0).unwrap().join(""), "ABC");
    let image = image::load_from_memory(&doc.page_png(0, 72.).unwrap())
        .unwrap()
        .to_rgb8();
    assert!(image.pixels().all(|pixel| pixel.0 == [255; 3]));
    assert!(pdf_text::extract(&doc.to_pdf().unwrap())
        .split_whitespace()
        .collect::<String>()
        .contains("ABC"));
}

#[test]
fn dashed() {
    check("dashed");
}

#[test]
fn fill_stroke_clip() {
    check("fill-stroke-clip");
}

#[test]
fn uniform_hairline() {
    check("uniform-hairline");
}
