mod common;
use ofdrw::OfdDocument;
use std::path::Path;

fn hairline_coverage(image: &image::RgbImage) -> f64 {
    image
        .pixels()
        .map(|pixel| {
            pixel
                .0
                .iter()
                .map(|v| 1. - f64::from(*v) / 255.)
                .sum::<f64>()
                / 3.
        })
        .sum()
}

fn check_affine_stroke(selected: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdf-affine-strokes");
    let reference: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("reference.json")).unwrap()).unwrap();
    let case = reference["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"].as_str() == Some(selected))
        .expect("requested affine reference case is missing");
    let name = selected;
    let pdf = std::fs::read(root.join(case["file"].as_str().unwrap())).unwrap();
    let doc = OfdDocument::from_pdf_bytes(&pdf).unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(doc.page_paths(0).unwrap().len(), 1, "{name}");
    assert!(
        doc.page_images(0).unwrap().is_empty(),
        "{name} rasterized the page"
    );
    assert!(
        doc.page_paths(0).unwrap()[0].ctm.is_some(),
        "{name} lost the affine pen"
    );
    let expected_width = case["line_width_pt"].as_f64().unwrap() * 25.4 / 72.;
    let width = doc.page_object_stroke_styles(0).unwrap()[0]
        .style
        .line_width;
    assert!((width - expected_width).abs() < 1e-6, "{name}");
    assert!(doc.validate_references().unwrap().is_empty());
    for dpi in [72., 144.] {
        let actual = image::load_from_memory(&doc.page_png(0, dpi).unwrap())
            .unwrap()
            .to_rgb8();
        let expected =
            image::open(root.join(case["references"][format!("{dpi:.0}")].as_str().unwrap()))
                .unwrap()
                .to_rgb8();
        assert_eq!(actual.dimensions(), expected.dimensions(), "{name}");
        if expected_width > 0. {
            common::assert_raster_matches(&actual, &expected, name);
        } else {
            let ratio = hairline_coverage(&actual) / hairline_coverage(&expected);
            assert!(
                (ratio - 1.).abs() < 0.12,
                "{name} at {dpi} DPI: ink ratio {ratio}"
            );
            for (source, target) in [(&actual, &expected), (&expected, &actual)] {
                let mut ink = 0usize;
                let mut unmatched = 0usize;
                for y in 1..source.height() - 1 {
                    for x in 1..source.width() - 1 {
                        if source.get_pixel(x, y).0.iter().all(|v| *v >= 250) {
                            continue;
                        }
                        ink += 1;
                        if !(y - 1..=y + 1).any(|yy| {
                            (x - 1..=x + 1)
                                .any(|xx| target.get_pixel(xx, yy).0.iter().any(|v| *v < 250))
                        }) {
                            unmatched += 1;
                        }
                    }
                }
                assert!(
                    ink > 100 && unmatched * 100 < ink * 3,
                    "{name}: {unmatched}/{ink} pixels misplaced"
                );
            }
        }
    }
    let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    assert_eq!(
        doc.page_png(0, 144.).unwrap(),
        reopened.page_png(0, 144.).unwrap(),
        "{name}"
    );
    assert!(
        lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok(),
        "{name}"
    );
}

#[test]
fn scale_dash() {
    check_affine_stroke("scale-dash");
}

#[test]
fn shear_dash() {
    check_affine_stroke("shear-dash");
}

#[test]
fn reflection() {
    check_affine_stroke("reflection");
}

#[test]
fn changing_ctm() {
    check_affine_stroke("changing-ctm");
}

#[test]
fn fill_stroke_alpha() {
    check_affine_stroke("fill-stroke-alpha");
}

#[test]
fn pattern_clip() {
    check_affine_stroke("pattern-clip");
}

#[test]
fn hairline() {
    check_affine_stroke("hairline");
}

#[test]
fn restored_ctm() {
    check_affine_stroke("restored-ctm");
}

#[test]
fn solid_curve() {
    check_affine_stroke("solid-curve");
}
