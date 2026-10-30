mod common;
use ofdrw::OfdDocument;

fn check(name: &str, source: &[u8], reference: &[u8]) {
    let document =
        OfdDocument::from_pdf_bytes(source).unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(
        document.pages().unwrap().len(),
        1,
        "{name}: temporary page leaked"
    );
    let png = document.page_png(0, 144.).unwrap();
    let actual = image::load_from_memory(&png).unwrap().to_rgb8();
    let expected = image::load_from_memory(reference).unwrap().to_rgb8();
    if matches!(name, "empty" | "hidden") {
        assert_eq!(actual, expected);
        assert!(document.page_objects(0).unwrap().is_empty());
    } else {
        common::assert_raster_matches(&actual, &expected, name);
    }
    if name == "text" {
        assert_eq!(document.page_text(0).unwrap(), ["A"]);
    }
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_png(0, 144.).unwrap(), png, "{name}");
    assert!(reopened.validate_references().unwrap().is_empty(), "{name}");
    let pdf = reopened.to_pdf().unwrap();
    assert!(lopdf::Document::load_mem(&pdf).is_ok());
    if let Some(gs) = std::env::var_os("OFDRW_GHOSTSCRIPT") {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("export.pdf");
        let target = temp.path().join("export.png");
        std::fs::write(&source, pdf).unwrap();
        let result = std::process::Command::new(gs)
            .args([
                "-q",
                "-dSAFER",
                "-dBATCH",
                "-dNOPAUSE",
                "-dUseFastColor=true",
                "-sDEVICE=png16m",
                "-r144",
                "-dGraphicsAlphaBits=1",
            ])
            .arg(format!("-sOutputFile={}", target.display()))
            .arg(source)
            .output()
            .unwrap();
        assert!(result.status.success(), "{name}: {:?}", result);
        let exported = image::open(target).unwrap().to_rgb8();
        if matches!(name, "empty" | "hidden") {
            assert_eq!(exported, expected);
        } else {
            common::assert_raster_matches(&exported, &expected, &format!("{name} PDF export"));
        }
    }
}

macro_rules! group_case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_bytes!(concat!("fixtures/isolated-group-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/isolated-group-", $name, ".png")),
            );
        }
    };
}
group_case!(opaque, "opaque");
group_case!(child_alpha, "child-alpha");
group_case!(group_alpha, "group-alpha");
group_case!(stroke_alpha, "stroke-alpha");
group_case!(parent_blend, "parent-blend");
group_case!(child_multiply, "child-multiply");
group_case!(child_screen, "child-screen");
group_case!(mixed_blends, "mixed-blends");
group_case!(bbox, "bbox");
group_case!(clip, "clip");
group_case!(matrix, "matrix");
group_case!(reflected, "reflected");
group_case!(rotated, "rotated");
group_case!(user_unit, "user-unit");
group_case!(nested, "nested");
group_case!(inherited_color, "inherited-color");
group_case!(image_alpha, "image-alpha");
group_case!(text, "text");
group_case!(gradient, "gradient");
group_case!(tiling, "tiling");
group_case!(pattern_cell, "pattern-cell");
group_case!(reuse, "reuse");
group_case!(ordinary_form, "ordinary-form");
group_case!(hidden, "hidden");
group_case!(indirect, "indirect");
group_case!(empty, "empty");
group_case!(text_clip, "text-clip");
group_case!(parent_stroke_alpha, "parent-stroke-alpha");

fn pixel(name: &str, x: u32, y: u32) -> [u8; 3] {
    let source = std::fs::read(format!(
        "{}/tests/fixtures/isolated-group-{name}.pdf",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let document = OfdDocument::from_pdf_bytes(&source).unwrap();
    image::load_from_memory(&document.page_png(0, 144.).unwrap())
        .unwrap()
        .to_rgb8()
        .get_pixel(x * 2, (64 - y) * 2)
        .0
}

fn assert_color(actual: [u8; 3], expected: [u8; 3]) {
    assert!(
        actual
            .into_iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff(b) <= 2),
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn group_opacity_applies_once_after_members_overlap() {
    assert_color(pixel("group-alpha", 40, 32), [128, 128, 128]);
}

#[test]
fn group_members_reset_parent_stroking_alpha() {
    assert_color(pixel("parent-stroke-alpha", 12, 24), [255, 102, 0]);
}

#[test]
fn internal_screen_blending_does_not_see_yellow_page_backdrop() {
    assert_color(pixel("child-screen", 16, 16), [255, 77, 0]);
}

#[test]
fn isolated_form_in_pattern_uses_transparent_backdrop() {
    assert_color(pixel("pattern-cell", 72, 40), [77, 77, 178]);
}

#[test]
fn exported_pdf_keeps_isolated_groups_as_vectors() {
    let source = include_bytes!("fixtures/isolated-group-child-screen.pdf");
    let document = OfdDocument::from_pdf_bytes(source).unwrap();
    let pdf = lopdf::Document::load_mem(&document.to_pdf().unwrap()).unwrap();
    assert!(pdf.objects.values().any(|object| {
        object
            .as_stream()
            .ok()
            .and_then(|stream| stream.dict.get(b"Group").ok())
            .and_then(|group| pdf.dereference(group).ok())
            .and_then(|(_, group)| group.as_dict().ok())
            .is_some_and(|group| {
                group.get(b"I").and_then(lopdf::Object::as_bool).ok() == Some(true)
            })
    }));
    assert!(!pdf.objects.values().any(|object| {
        object.as_stream().is_ok_and(|stream| {
            stream
                .dict
                .get(b"Subtype")
                .and_then(lopdf::Object::as_name)
                .ok()
                == Some(b"Image".as_slice())
        })
    }));
}
