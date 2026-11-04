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

fn evaluate_function(pdf: &lopdf::Document, reference: lopdf::ObjectId, input: f64) -> Vec<f64> {
    let dictionary = pdf.get_object(reference).unwrap().as_dict().unwrap();
    let numbers = |key: &[u8]| -> Vec<f64> {
        dictionary
            .get(key)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| f64::from(v.as_float().unwrap()))
            .collect()
    };
    let domain = numbers(b"Domain");
    let t = input.clamp(domain[0], domain[1]);
    match dictionary.get(b"FunctionType").unwrap().as_i64().unwrap() {
        2 => {
            let first = numbers(b"C0");
            let last = numbers(b"C1");
            let exponent = f64::from(dictionary.get(b"N").unwrap().as_float().unwrap());
            first
                .iter()
                .zip(last)
                .map(|(a, b)| a + t.powf(exponent) * (b - a))
                .collect()
        }
        3 => {
            let bounds = numbers(b"Bounds");
            let encode = numbers(b"Encode");
            let index = bounds.partition_point(|&bound| bound <= t);
            let left = if index == 0 {
                domain[0]
            } else {
                bounds[index - 1]
            };
            let right = bounds.get(index).copied().unwrap_or(domain[1]);
            let mapped = encode[2 * index]
                + (t - left) / (right - left) * (encode[2 * index + 1] - encode[2 * index]);
            let functions = dictionary.get(b"Functions").unwrap().as_array().unwrap();
            evaluate_function(pdf, functions[index].as_reference().unwrap(), mapped)
        }
        other => panic!("unexpected PDF function type {other}"),
    }
}

fn check_periodic(
    radial: bool,
    mapping: &str,
    alpha: bool,
    stroke: bool,
    pattern: bool,
    inner: bool,
) {
    check_periodic_stops(radial, mapping, alpha, stroke, pattern, inner, false);
}

fn check_periodic_stops(
    radial: bool,
    mapping: &str,
    alpha: bool,
    stroke: bool,
    pattern: bool,
    inner: bool,
    hard: bool,
) {
    let mut document = document(if inner { 1. } else { 0. }, alpha, stroke, pattern);
    let path = document.pages().unwrap()[0].path.clone();
    let mut source = std::str::from_utf8(document.entry(&path).unwrap().unwrap())
        .unwrap()
        .replace(
            "<ofd:RadialShd ",
            &format!("<ofd:RadialShd MapType=\"{mapping}\" "),
        )
        .replace("EndRadius=\"10\"", "EndRadius=\"3\"");
    if !radial {
        source = source
            .replace("RadialShd", "AxialShd")
            .replace("EndPoint=\"15 15\"", "EndPoint=\"20 15\"");
    }
    if hard {
        source = source.replace("Position=\"0\"", "Position=\"0.2\"")
            .replace("<ofd:Segment Position=\"1\">", "<ofd:Segment Position=\"0.5\"><ofd:Color Value=\"0 255 0\"/></ofd:Segment><ofd:Segment Position=\"0.5\"><ofd:Color Value=\"255 255 0\"/></ofd:Segment><ofd:Segment Position=\"0.8\">");
    }
    if inner {
        source = source.replace("EndRadius=\"3\"", "EndRadius=\"6\"");
    }
    document.set_entry(&path, source.into_bytes()).unwrap();
    let document = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert!(document.validate_references().unwrap().is_empty());
    let bytes = document.to_pdf().unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let mut shadings = 0;
    for object in pdf.objects.values() {
        if let Ok(stream) = object.as_stream() {
            assert_ne!(
                stream
                    .dict
                    .get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .ok(),
                Some(b"Image".as_slice()),
                "periodic gradients must remain vector shadings"
            );
        }
        if let Ok(dictionary) = object.as_dict() {
            if dictionary
                .get(b"ShadingType")
                .and_then(lopdf::Object::as_i64)
                .ok()
                == Some(if radial { 3 } else { 2 })
            {
                let domain = dictionary.get(b"Domain").unwrap().as_array().unwrap();
                assert_eq!(domain.len(), 2);
                assert!(domain[1].as_float().unwrap() > 1.);
                let function = dictionary.get(b"Function").unwrap().as_reference().unwrap();
                for t in [-2.25_f64, -1.75, -0.25, 0., 0.25, 0.75, 1., 1.25, 3.75] {
                    if t < f64::from(domain[0].as_float().unwrap())
                        || t >= f64::from(domain[1].as_float().unwrap())
                    {
                        continue;
                    }
                    let phase = if mapping == "Repeat" {
                        t.rem_euclid(1.)
                    } else {
                        1. - (t.rem_euclid(2.) - 1.).abs()
                    };
                    let values = evaluate_function(&pdf, function, t);
                    let expected = if values.len() == 1 {
                        vec![(64. + (220. - 64.) * phase) / 255.]
                    } else if hard {
                        if phase < 0.2 {
                            vec![1., 0., 0.]
                        } else if phase < 0.5 {
                            let t = (phase - 0.2) / 0.3;
                            vec![1. - t, t, 0.]
                        } else if phase < 0.8 {
                            let t = (phase - 0.5) / 0.3;
                            vec![1. - t, 1. - t, t]
                        } else {
                            vec![0., 0., 1.]
                        }
                    } else {
                        vec![1. - phase, 0., phase]
                    };
                    for (actual, expected) in values.iter().zip(expected) {
                        assert!(
                            (actual - expected).abs() < 1e-5,
                            "{mapping} parameter {t}: {values:?}"
                        );
                    }
                }
                shadings += 1;
            }
        }
    }
    assert_eq!(shadings, if alpha { 2 } else { 1 });
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("gradient.pdf");
    std::fs::write(&input, &bytes).unwrap();
    for dpi in if hard { [406.4, 812.8] } else { [203.2, 406.4] } {
        let expected = image::load_from_memory(&document.page_png(0, dpi).unwrap())
            .unwrap()
            .to_rgb8();
        let output = temp.path().join("gradient");
        let label = format!("radial={radial} {mapping} alpha={alpha} stroke={stroke} pattern={pattern} inner={inner} dpi={dpi}");
        if let Some(program) = std::env::var_os("OFDRW_PDFTOPPM") {
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
            common::assert_raster_matches(&actual, &expected, &format!("Poppler {label}"));
        }
        if let Some(program) = std::env::var_os("OFDRW_GHOSTSCRIPT") {
            let status = std::process::Command::new(&program)
                .args([
                    "-q",
                    "-dSAFER",
                    "-dBATCH",
                    "-dNOPAUSE",
                    "-sDEVICE=png16m",
                    "-dTextAlphaBits=4",
                    "-dGraphicsAlphaBits=4",
                    "-dGridFitTT=0",
                ])
                .arg(format!("-r{dpi}"))
                .arg(format!(
                    "-sOutputFile={}",
                    output.with_extension("png").display()
                ))
                .arg(&input)
                .output()
                .unwrap();
            assert!(status.status.success(), "{status:?}");
            let actual = image::open(output.with_extension("png")).unwrap().to_rgb8();
            common::assert_raster_matches(&actual, &expected, &format!("Ghostscript {label}"));
        }
    }
}

macro_rules! periodic_cases {
    ($($name:ident: ($radial:expr, $mapping:expr, $alpha:expr, $stroke:expr, $pattern:expr, $inner:expr)),* $(,)?) => {
        $(#[test] fn $name() { check_periodic($radial, $mapping, $alpha, $stroke, $pattern, $inner); })*
    };
}

periodic_cases! {
    axial_repeat_opaque_fill: (false, "Repeat", false, false, false, false),
    axial_repeat_opaque_stroke: (false, "Repeat", false, true, false, false),
    axial_repeat_opaque_pattern: (false, "Repeat", false, false, true, false),
    axial_repeat_alpha_fill: (false, "Repeat", true, false, false, false),
    axial_repeat_alpha_stroke: (false, "Repeat", true, true, false, false),
    axial_repeat_alpha_pattern: (false, "Repeat", true, false, true, false),
    axial_reflect_opaque_fill: (false, "Reflect", false, false, false, false),
    axial_reflect_opaque_stroke: (false, "Reflect", false, true, false, false),
    axial_reflect_opaque_pattern: (false, "Reflect", false, false, true, false),
    axial_reflect_alpha_fill: (false, "Reflect", true, false, false, false),
    axial_reflect_alpha_stroke: (false, "Reflect", true, true, false, false),
    axial_reflect_alpha_pattern: (false, "Reflect", true, false, true, false),
    radial_repeat_opaque_fill: (true, "Repeat", false, false, false, false),
    radial_repeat_opaque_stroke: (true, "Repeat", false, true, false, false),
    radial_repeat_opaque_pattern: (true, "Repeat", false, false, true, false),
    radial_repeat_alpha_fill: (true, "Repeat", true, false, false, false),
    radial_repeat_alpha_stroke: (true, "Repeat", true, true, false, false),
    radial_repeat_alpha_pattern: (true, "Repeat", true, false, true, false),
    radial_reflect_opaque_fill: (true, "Reflect", false, false, false, false),
    radial_reflect_opaque_stroke: (true, "Reflect", false, true, false, false),
    radial_reflect_opaque_pattern: (true, "Reflect", false, false, true, false),
    radial_reflect_alpha_fill: (true, "Reflect", true, false, false, false),
    radial_reflect_alpha_stroke: (true, "Reflect", true, true, false, false),
    radial_reflect_alpha_pattern: (true, "Reflect", true, false, true, false),
    radial_repeat_inner_radius: (true, "Repeat", true, false, false, true),
    radial_reflect_inner_radius: (true, "Reflect", true, false, false, true),
}

#[test]
fn axial_repeat_hard_and_padded_stops() {
    check_periodic_stops(false, "Repeat", false, false, false, false, true);
}

#[test]
fn axial_reflect_hard_and_padded_stops() {
    check_periodic_stops(false, "Reflect", false, false, false, false, true);
}

#[test]
fn radial_repeat_hard_and_padded_stops() {
    check_periodic_stops(true, "Repeat", false, false, false, false, true);
}

#[test]
fn radial_reflect_hard_and_padded_stops() {
    check_periodic_stops(true, "Reflect", false, false, false, false, true);
}

#[test]
fn many_periods_reuse_a_logarithmic_function_tree() {
    for mapping in ["repeat", "reflect"] {
        let source = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="20"><defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" x2="0.0001" spreadMethod="{mapping}"><stop offset="0" stop-color="#f00"/><stop offset="1" stop-color="#00f"/></linearGradient></defs><path d="M0 0H100V20H0Z" fill="url(#g)"/></svg>"##
        );
        let tree =
            svg2pdf::usvg::Tree::from_str(&source, &svg2pdf::usvg::Options::default()).unwrap();
        let bytes = svg2pdf::to_pdf(
            &tree,
            svg2pdf::ConversionOptions::default(),
            svg2pdf::PageOptions::default(),
        )
        .unwrap();
        assert!(
            bytes.len() < 20_000,
            "one million periods must remain compact: {}",
            bytes.len()
        );
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let count = pdf
            .objects
            .values()
            .filter(|object| {
                object
                    .as_dict()
                    .is_ok_and(|dictionary| dictionary.has(b"FunctionType"))
            })
            .count();
        assert!(
            count < 30,
            "one million periods must share functions: {count}"
        );
        let shading = pdf
            .objects
            .values()
            .filter_map(|object| object.as_dict().ok())
            .find(|dict| dict.has(b"ShadingType"))
            .unwrap();
        let function = shading.get(b"Function").unwrap().as_reference().unwrap();
        for t in [0.25_f64, 1.25, 31.75, 65_535.25, 999_999.75] {
            let expected = if mapping == "repeat" {
                t.rem_euclid(1.)
            } else {
                1. - (t.rem_euclid(2.) - 1.).abs()
            };
            let color = evaluate_function(&pdf, function, t);
            assert!(
                (color[2] - expected).abs() < 1e-5,
                "{mapping} {t}: {color:?}"
            );
        }
    }
}
