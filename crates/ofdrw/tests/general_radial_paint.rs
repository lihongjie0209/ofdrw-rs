//! Paint OFD two-circle fields directly; the oracle is the original PDF,
//! rendered independently by Ghostscript, rather than our PDF importer.
mod common;
#[path = "common/pdf_text.rs"]
mod pdf_text;
use ofdrw::{GradientFillStyle, GradientKind, GradientMapType, GradientSegment, OfdDocument};
use std::path::Path;

fn check_pdf(bytes: &[u8], expected: &image::RgbImage, label: &str) {
    for (backend, program) in [
        ("Poppler", std::env::var_os("OFDRW_PDFTOPPM")),
        ("Ghostscript", std::env::var_os("OFDRW_GHOSTSCRIPT")),
    ] {
        let Some(program) = program else { continue };
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("radial.pdf");
        let output = temp.path().join("radial.png");
        std::fs::write(&source, bytes).unwrap();
        let mut command = std::process::Command::new(program);
        if backend == "Poppler" {
            command
                .args([
                    "-singlefile",
                    "-png",
                    "-scale-to-x",
                    &expected.width().to_string(),
                    "-scale-to-y",
                    &expected.height().to_string(),
                ])
                .arg(&source)
                .arg(output.with_extension(""));
        } else {
            command
                .args([
                    "-q",
                    "-dSAFER",
                    "-dBATCH",
                    "-dNOPAUSE",
                    "-sDEVICE=png16m",
                    "-r144",
                    "-dTextAlphaBits=4",
                    "-dGraphicsAlphaBits=4",
                    "-dGridFitTT=0",
                ])
                .arg(format!("-g{}x{}", expected.width(), expected.height()))
                .arg(format!("-sOutputFile={}", output.display()))
                .arg(&source);
        }
        let result = command.output().unwrap();
        assert!(result.status.success(), "{backend} {label}: {result:?}");
        let actual = image::open(output).unwrap().to_rgb8();
        if label == "both-zero" {
            assert_eq!(&actual, expected);
        } else {
            common::assert_raster_matches(&actual, expected, &format!("{backend} PDF {label}"));
        }
    }
}

fn document(name: &str) -> OfdDocument {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let pdf = lopdf::Document::load(root.join(format!("general-radial-{name}.pdf"))).unwrap();
    let shading = pdf.get_object((5, 0)).unwrap().as_dict().unwrap();
    let mut coordinates: Vec<_> = shading
        .get(b"Coords")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|value| f64::from(value.as_float().unwrap()))
        .collect();
    if name.starts_with("equal-") && name != "equal-both" {
        coordinates = vec![24., 32., 14., 72., 32., 14.];
    }
    if name == "ellipse" {
        coordinates[0] = 26.4;
        coordinates[1] = 36.1569219381653;
    }
    let extend = shading.get(b"Extend").unwrap().as_array().unwrap();
    let unit = 25.4 / 72.;
    let (width, height) = (96. * unit, 64. * unit);
    let style = GradientFillStyle {
        kind: GradientKind::Radial {
            start_radius: coordinates[2] * unit,
            end_radius: coordinates[5] * unit,
            eccentricity: if name == "ellipse" { 0.6 } else { 0. },
            angle: if name == "ellipse" { 30. } else { 0. },
        },
        start_point: [coordinates[0] * unit, (64. - coordinates[1]) * unit],
        end_point: [coordinates[3] * unit, (64. - coordinates[4]) * unit],
        segments: vec![
            GradientSegment {
                position: Some(0.),
                color: [255, 0, 0],
                alpha: None,
            },
            GradientSegment {
                position: Some(1.),
                color: [0, 0, 255],
                alpha: None,
            },
        ],
        map_type: match name {
            "equal-repeat" => GradientMapType::Repeat,
            "equal-reflect" => GradientMapType::Reflect,
            _ => GradientMapType::Direct,
        },
        map_unit: None,
        extend: if matches!(name, "equal-repeat" | "equal-reflect") {
            3
        } else {
            u8::from(extend[0].as_bool().unwrap()) + 2 * u8::from(extend[1].as_bool().unwrap())
        },
        fill_alpha: if name == "alpha" { Some(128) } else { None },
    };
    let mut document = OfdDocument::new();
    document
        .add_path_page(
            &format!("M 0 0 L {width} 0 L {width} {height} L 0 {height} C"),
            [0., 0., width, height],
            width,
            height,
            true,
            false,
        )
        .unwrap();
    let id = document.page_paths(0).unwrap()[0].id.clone().unwrap();
    document
        .set_page_object_gradient_fill(0, &id, &style)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    if matches!(name, "affine" | "reflected") {
        let [a, b, c, d, e, f]: [f64; 6] = if name == "affine" {
            [1., 0.2, 0.4, 0.8, -8., 0.]
        } else {
            [-1., 0.2, 0.3, 0.9, 84., 0.]
        };
        let [a, b, c, d, e, f] = [
            a,
            -b,
            -c,
            d,
            (64. * c + e) * unit,
            (64. * (1. - d) - f) * unit,
        ];
        let determinant = a * d - b * c;
        let points = [[0., 0.], [width, 0.], [width, height], [0., height]].map(|[x, y]| {
            [
                (d * (x - e) - c * (y - f)) / determinant,
                (-b * (x - e) + a * (y - f)) / determinant,
            ]
        });
        let commands = format!(
            "M {} {} L {} {} L {} {} L {} {} C",
            points[0][0],
            points[0][1],
            points[1][0],
            points[1][1],
            points[2][0],
            points[2][1],
            points[3][0],
            points[3][1]
        );
        let path = document.pages().unwrap()[0].path.clone();
        let source = std::str::from_utf8(document.entry(&path).unwrap().unwrap()).unwrap();
        let parsed = roxmltree::Document::parse(source).unwrap();
        let data = parsed
            .descendants()
            .find(|node| node.tag_name().name() == "AbbreviatedData")
            .unwrap()
            .range();
        let mut source = source.to_owned();
        source.replace_range(
            data,
            &format!("<ofd:AbbreviatedData>{commands}</ofd:AbbreviatedData>"),
        );
        let source = source.replace(
            "<ofd:PathObject ",
            &format!("<ofd:PathObject CTM=\"{a} {b} {c} {d} {e} {f}\" "),
        );
        document.set_entry(&path, source.into_bytes()).unwrap();
    }
    if name == "clipped" {
        document
            .set_page_object_clip_rects(
                0,
                &id,
                &[[20. * unit, 12. * unit, 58. * unit, 38. * unit]],
                true,
            )
            .unwrap();
    }
    document
}

fn check(name: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let document = document(name);
    let actual = image::load_from_memory(&document.page_png(0, 144.).unwrap())
        .unwrap()
        .to_rgb8();
    let expected = image::open(root.join(format!("general-radial-{name}.png")))
        .unwrap()
        .to_rgb8();
    if name == "both-zero" {
        assert_eq!(actual, expected);
    } else {
        common::assert_raster_matches(&actual, &expected, name);
    }
    assert!(document.validate_references().unwrap().is_empty());
    assert!(document.page_images(0).unwrap().is_empty());
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(
        reopened.page_png(0, 144.).unwrap(),
        document.page_png(0, 144.).unwrap()
    );
    let bytes = reopened.to_pdf().unwrap();
    assert!(lopdf::Document::load_mem(&bytes).is_ok());
    check_pdf(&bytes, &expected, name);
}

macro_rules! case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check($name);
        }
    };
}
case!(grow, "grow");
case!(shrink, "shrink");
case!(overlap, "overlap");
case!(equal, "equal");
case!(grow_start, "grow-start");
case!(grow_end, "grow-end");
case!(grow_both, "grow-both");
case!(shrink_start, "shrink-start");
case!(shrink_end, "shrink-end");
case!(shrink_both, "shrink-both");
case!(equal_both, "equal-both");
case!(point_start, "point-start");
case!(point_end, "point-end");
case!(both_zero, "both-zero");
case!(tangent, "tangent");
case!(near_tangent, "near-tangent");
case!(affine, "affine");
case!(reflected, "reflected");
case!(clipped, "clipped");
case!(alpha, "alpha");

case!(equal_repeat, "equal-repeat");
case!(equal_reflect, "equal-reflect");

fn check_colored_shape(mut shape: OfdDocument, name: &str) {
    let field = document("grow");
    let field_path = field.pages().unwrap()[0].path.clone();
    let source = std::str::from_utf8(field.entry(&field_path).unwrap().unwrap()).unwrap();
    let parsed = roxmltree::Document::parse(source).unwrap();
    let range = parsed
        .descendants()
        .find(|node| node.tag_name().name() == "FillColor")
        .unwrap()
        .range();
    let paint = source[range].replace("FillColor", name);
    let mask = image::load_from_memory(&shape.page_png(0, 144.).unwrap())
        .unwrap()
        .to_rgb8();
    let reference = image::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/general-radial-grow.png"),
    )
    .unwrap()
    .to_rgb8();
    let expected = image::RgbImage::from_fn(mask.width(), mask.height(), |x, y| {
        image::Rgb(std::array::from_fn(|channel| {
            let coverage = 1. - f64::from(mask.get_pixel(x, y)[channel]) / 255.;
            (255. + (f64::from(reference.get_pixel(x, y)[channel]) - 255.) * coverage).round() as u8
        }))
    });
    let path = shape.pages().unwrap()[0].path.clone();
    let source = std::str::from_utf8(shape.entry(&path).unwrap().unwrap()).unwrap();
    let parsed = roxmltree::Document::parse(source).unwrap();
    let mut updated = source.to_owned();
    if let Some(old) = parsed
        .descendants()
        .find(|node| node.tag_name().name() == name)
    {
        updated.replace_range(old.range(), &paint);
    } else {
        let tag = if shape.page_text(0).unwrap().is_empty() {
            "PathObject"
        } else {
            "TextObject"
        };
        updated = updated.replace(&format!("</ofd:{tag}>"), &format!("{paint}</ofd:{tag}>"));
    }
    shape.set_entry(&path, updated.into_bytes()).unwrap();
    let actual = image::load_from_memory(&shape.page_png(0, 144.).unwrap())
        .unwrap()
        .to_rgb8();
    common::assert_raster_matches(&actual, &expected, name);
    let reopened = OfdDocument::from_bytes(&shape.to_bytes().unwrap()).unwrap();
    assert_eq!(
        shape.page_png(0, 144.).unwrap(),
        reopened.page_png(0, 144.).unwrap()
    );
    assert!(reopened.validate_references().unwrap().is_empty());
    check_pdf(&reopened.to_pdf().unwrap(), &expected, name);
    if !shape.page_text(0).unwrap().is_empty() {
        assert_eq!(shape.page_text(0).unwrap().concat(), "MMMMMM");
        let text = pdf_text::extract(&reopened.to_pdf().unwrap());
        assert_eq!(
            text.split_whitespace().collect::<String>(),
            "MMMMMM",
            "PDF text: {text:?}"
        );
    }
}

#[test]
fn boundary_free_stroke_uses_the_field_without_filling_its_interior() {
    let unit = 25.4 / 72.;
    let mut shape = OfdDocument::new();
    shape
        .add_path_page(
            "M 5 5 L 29 5 L 29 17 L 5 17 C",
            [0., 0., 96. * unit, 64. * unit],
            96. * unit,
            64. * unit,
            false,
            true,
        )
        .unwrap();
    let path = shape.pages().unwrap()[0].path.clone();
    let source = std::str::from_utf8(shape.entry(&path).unwrap().unwrap()).unwrap();
    let parsed = roxmltree::Document::parse(source).unwrap();
    let attribute = parsed
        .descendants()
        .find(|node| node.tag_name().name() == "PathObject")
        .unwrap()
        .attribute_node("Boundary")
        .unwrap();
    let mut source = source.to_owned();
    source.replace_range(attribute.range(), "LineWidth=\"2\"");
    shape.set_entry(&path, source.into_bytes()).unwrap();
    check_colored_shape(shape, "StrokeColor");
}

#[test]
fn text_uses_the_field_without_losing_selectable_content() {
    let unit = 25.4 / 72.;
    let mut shape = OfdDocument::new();
    shape
        .add_text_page("MMMMMM", 96. * unit, 64. * unit)
        .unwrap();
    let path = shape.pages().unwrap()[0].path.clone();
    let source = std::str::from_utf8(shape.entry(&path).unwrap().unwrap()).unwrap();
    let parsed = roxmltree::Document::parse(source).unwrap();
    let attribute = parsed
        .descendants()
        .find(|node| node.tag_name().name() == "TextCode")
        .unwrap()
        .attribute_node("Y")
        .unwrap();
    let x = parsed
        .descendants()
        .find(|node| node.tag_name().name() == "TextCode")
        .unwrap()
        .attribute_node("X")
        .unwrap()
        .range();
    let boundary = parsed
        .descendants()
        .find(|node| node.tag_name().name() == "TextObject")
        .unwrap()
        .attribute_node("Boundary")
        .unwrap()
        .range();
    let mut source = source.to_owned();
    source.replace_range(attribute.range(), "Y=\"12\"");
    source.replace_range(x, "X=\"3\"");
    source.replace_range(
        boundary,
        &format!("Boundary=\"0 0 {} {}\"", 96. * unit, 64. * unit),
    );
    shape.set_entry(&path, source.into_bytes()).unwrap();
    check_colored_shape(shape, "FillColor");
}

#[test]
fn pattern_and_composite_magnification_preserve_field_resolution() {
    use base64::Engine;
    let unit = 25.4 / 72.;
    let (width, height) = (96. * unit, 64. * unit);
    let reference = image::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/general-radial-grow.png"),
    )
    .unwrap()
    .to_rgb8();
    for composite in [false, true] {
        for scale in [8., 128.] {
            let mut document = document("grow");
            let (w, h) = (width / scale, height / scale);
            let child = format!("<ofd:PathObject ID=\"41\" Boundary=\"0 0 {w} {h}\" Fill=\"true\" Stroke=\"false\"><ofd:FillColor><ofd:RadialShd StartPoint=\"{} {}\" EndPoint=\"{} {}\" StartRadius=\"{}\" EndRadius=\"{}\"><ofd:Segment Position=\"0\"><ofd:Color Value=\"255 0 0\"/></ofd:Segment><ofd:Segment Position=\"1\"><ofd:Color Value=\"0 0 255\"/></ofd:Segment></ofd:RadialShd></ofd:FillColor><ofd:AbbreviatedData>M 0 0 L {w} 0 L {w} {h} L 0 {h} C</ofd:AbbreviatedData></ofd:PathObject>", 24. * unit / scale, 32. * unit / scale, 72. * unit / scale, 32. * unit / scale, 8. * unit / scale, 20. * unit / scale);
            let path = document.pages().unwrap()[0].path.clone();
            let source = std::str::from_utf8(document.entry(&path).unwrap().unwrap()).unwrap();
            let parsed = roxmltree::Document::parse(source).unwrap();
            let mut page = source.to_owned();
            if composite {
                let object = parsed
                    .descendants()
                    .find(|node| node.tag_name().name() == "PathObject")
                    .unwrap();
                let id = object.attribute("ID").unwrap();
                page.replace_range(object.range(), &format!("<ofd:CompositeObject ID=\"{id}\" ResourceID=\"40\" CTM=\"{scale} 0 0 {scale} 0 0\" Boundary=\"0 0 {width} {height}\"/>"));
            } else {
                let color = parsed
                    .descendants()
                    .find(|node| node.tag_name().name() == "FillColor")
                    .unwrap();
                page.replace_range(color.range(), &format!("<ofd:FillColor><ofd:Pattern Width=\"{w}\" Height=\"{h}\" CTM=\"{scale} 0 0 {scale} 0 0\"><ofd:CellContent>{child}</ofd:CellContent></ofd:Pattern></ofd:FillColor>"));
            }
            document.set_entry(&path, page.into_bytes()).unwrap();
            if composite {
                let root = "Doc_0/PublicRes.xml";
                let source = std::str::from_utf8(document.entry(root).unwrap().unwrap()).unwrap();
                let source = source.replace("</ofd:Res>", &format!("<ofd:CompositeGraphicUnits><ofd:CompositeGraphicUnit ID=\"40\" Width=\"{w}\" Height=\"{h}\"><ofd:Content>{child}</ofd:Content></ofd:CompositeGraphicUnit></ofd:CompositeGraphicUnits></ofd:Res>"));
                document.set_entry(root, source.into_bytes()).unwrap();
            }
            let root = "Doc_0/Document.xml";
            let source = std::str::from_utf8(document.entry(root).unwrap().unwrap()).unwrap();
            let parsed = roxmltree::Document::parse(source).unwrap();
            let range = parsed
                .descendants()
                .find(|node| node.tag_name().name() == "MaxUnitID")
                .unwrap()
                .range();
            let mut source = source.to_owned();
            source.replace_range(range, "<ofd:MaxUnitID>41</ofd:MaxUnitID>");
            document.set_entry(root, source.into_bytes()).unwrap();
            let svg = document.page_svg(0).unwrap();
            let parsed = roxmltree::Document::parse(&svg).unwrap();
            let data = parsed
                .descendants()
                .find(|node| node.tag_name().name() == "image")
                .unwrap()
                .attribute("href")
                .unwrap()
                .strip_prefix("data:image/png;base64,")
                .unwrap();
            let tile = image::load_from_memory(
                &base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap(),
            )
            .unwrap();
            assert!(
                tile.width() >= 542 && tile.height() >= 362,
                "{composite}, {scale}: {}x{}",
                tile.width(),
                tile.height()
            );
            let actual = image::load_from_memory(&document.page_png(0, 144.).unwrap())
                .unwrap()
                .to_rgb8();
            common::assert_raster_matches(
                &actual,
                &reference,
                &format!("composite={composite}, scale={scale}"),
            );
            check_pdf(
                &document.to_pdf().unwrap(),
                &reference,
                &format!("composite={composite}, scale={scale}"),
            );
            let issues = document.validate_references().unwrap();
            assert!(
                issues.is_empty(),
                "composite={composite}, scale={scale}: {issues:?}"
            );
        }
    }
}

case!(ellipse, "ellipse");
