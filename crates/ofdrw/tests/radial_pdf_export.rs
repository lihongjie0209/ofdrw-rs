//! Preserve both circles of OFD radial shadings through SVG and native PDF.
mod common;

use ofdrw::OfdDocument;

fn document(radius: f64, alpha: bool, stroke: bool, pattern: bool) -> OfdDocument {
    let mut document = OfdDocument::new();
    document
        .add_path_page(
            "M 3 3 L 27 3 L 27 27 L 3 27 C",
            [0., 0., 30., 30.],
            30.,
            30.,
            !stroke,
            stroke,
        )
        .unwrap();
    let path = document.pages().unwrap()[0].path.clone();
    let original = std::str::from_utf8(document.entry(&path).unwrap().unwrap()).unwrap();
    let tag = if stroke { "StrokeColor" } else { "FillColor" };
    let (first_alpha, last_alpha) = if alpha { (64, 220) } else { (255, 255) };
    let shading = format!(
        "<ofd:RadialShd Extend=\"3\" StartPoint=\"14 15\" EndPoint=\"15 15\" \
         StartRadius=\"{radius}\" EndRadius=\"10\" Eccentricity=\"0.6\" Angle=\"25\">\
         <ofd:Segment Position=\"0\"><ofd:Color Value=\"255 0 0\" Alpha=\"{first_alpha}\"/></ofd:Segment>\
         <ofd:Segment Position=\"1\"><ofd:Color Value=\"0 0 255\" Alpha=\"{last_alpha}\"/></ofd:Segment>\
         </ofd:RadialShd>"
    );
    let paint = if pattern {
        format!(
            "<ofd:Pattern Width=\"30\" Height=\"30\"><ofd:CellContent>\
             <ofd:PathObject ID=\"40\" Boundary=\"0 0 30 30\" Fill=\"true\" Stroke=\"false\">\
             <ofd:FillColor>{shading}</ofd:FillColor>\
             <ofd:AbbreviatedData>M 0 0 L 30 0 L 30 30 L 0 30 C</ofd:AbbreviatedData>\
             </ofd:PathObject></ofd:CellContent></ofd:Pattern>"
        )
    } else {
        shading
    };
    let page = original
        .replace("<ofd:PathObject ", "<ofd:PathObject LineWidth=\"4\" ")
        .replace(
            "</ofd:PathObject>",
            &format!("<ofd:{tag}>{paint}</ofd:{tag}></ofd:PathObject>"),
        );
    document.set_entry(&path, page.into_bytes()).unwrap();
    if pattern {
        let root = document.document_info().unwrap()[0].root.clone();
        let source = std::str::from_utf8(document.entry(&root).unwrap().unwrap()).unwrap();
        let parsed = roxmltree::Document::parse(source).unwrap();
        let max = parsed
            .descendants()
            .find(|node| node.has_tag_name(("http://www.ofdspec.org/2016", "MaxUnitID")))
            .unwrap()
            .range();
        let mut source = source.to_owned();
        source.replace_range(max, "<ofd:MaxUnitID>40</ofd:MaxUnitID>");
        document.set_entry(&root, source.into_bytes()).unwrap();
    }
    assert!(document.validate_references().unwrap().is_empty());
    document
}

fn check(radius: f64, alpha: bool, stroke: bool, pattern: bool) {
    let document = document(radius, alpha, stroke, pattern);
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    let bytes = reopened.to_pdf().unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let mut shadings = 0;
    let mut opacity_shadings = 0;
    for object in pdf.objects.values() {
        let Ok(dictionary) = object.as_dict() else {
            continue;
        };
        if dictionary
            .get(b"ShadingType")
            .and_then(lopdf::Object::as_i64)
            .ok()
            != Some(3)
        {
            continue;
        }
        let coords = dictionary.get(b"Coords").unwrap().as_array().unwrap();
        let coords: Vec<_> = coords
            .iter()
            .map(|value| f64::from(value.as_float().unwrap()))
            .collect();
        // The rotated eccentric ellipse maps its start center into the
        // circular shading coordinate system; its two radii stay unchanged.
        for (actual, expected) in coords
            .iter()
            .zip([13.955348, 15.095756, radius, 15., 15., 10.])
        {
            assert!(
                (actual - expected).abs() < 1e-5,
                "shading coordinates {coords:?}"
            );
        }
        assert_eq!(coords.len(), 6);
        shadings += 1;
        let color_space = dictionary.get(b"ColorSpace").unwrap().as_array().unwrap();
        assert_eq!(color_space[0].as_name().unwrap(), b"ICCBased");
        let profile = pdf
            .get_object(color_space[1].as_reference().unwrap())
            .unwrap()
            .as_stream()
            .unwrap();
        opacity_shadings += usize::from(profile.dict.get(b"N").unwrap().as_i64().unwrap() == 1);
    }
    assert!(shadings > 0, "gradient was lost or rasterized");
    assert_eq!(
        opacity_shadings > 0,
        alpha,
        "opacity must use the same circles"
    );

    // Linux CI requires this independent PDF renderer. Other platforms still
    // verify every native shading dictionary, including the opacity soft mask.
    if let Some(program) = std::env::var_os("OFDRW_PDFTOPPM") {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("gradient.pdf");
        std::fs::write(&input, &bytes).unwrap();
        for dpi in [101.6, 203.2] {
            let expected = image::load_from_memory(&reopened.page_png(0, dpi).unwrap())
                .unwrap()
                .to_rgb8();
            let output = temp.path().join("gradient");
            let status = std::process::Command::new(&program)
                .args([
                    "-singlefile",
                    "-png",
                    "-scale-to-x",
                    &expected.width().to_string(),
                    "-scale-to-y",
                    &expected.height().to_string(),
                ])
                .arg(&input)
                .arg(&output)
                .output()
                .unwrap();
            assert!(status.status.success(), "{status:?}");
            let actual = image::open(output.with_extension("png")).unwrap().to_rgb8();
            common::assert_raster_matches(
                &actual,
                &expected,
                &format!(
                    "radius={radius}, alpha={alpha}, stroke={stroke}, pattern={pattern}, dpi={dpi}"
                ),
            );
        }
    }
}

#[test]
fn fill_preserves_start_circle() {
    check(4., false, false, false);
}

#[test]
fn transparent_fill_preserves_start_circle_in_soft_mask() {
    check(4., true, false, false);
}

#[test]
fn stroke_preserves_start_circle() {
    check(4., false, true, false);
}

#[test]
fn transparent_stroke_preserves_start_circle_in_soft_mask() {
    check(4., true, true, false);
}

#[test]
fn pattern_preserves_start_circle() {
    check(4., false, false, true);
}

#[test]
fn transparent_pattern_preserves_start_circle() {
    check(4., true, false, true);
}

#[test]
fn zero_start_radius_remains_supported() {
    check(0., false, false, false);
}
