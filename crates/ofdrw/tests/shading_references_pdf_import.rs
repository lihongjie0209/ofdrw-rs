//! Shading dictionaries may reference both arrays and individual array entries.
mod common;
use lopdf::{content::Operation, dictionary, Document as PdfDocument, Object, ObjectId};
use ofdrw::OfdDocument;

fn dictionary_mut(pdf: &mut PdfDocument, id: ObjectId) -> &mut lopdf::Dictionary {
    match pdf.get_object_mut(id).unwrap() {
        Object::Dictionary(dict) => dict,
        Object::Stream(stream) => &mut stream.dict,
        _ => panic!("fixture object is not a dictionary"),
    }
}

fn serialize(pdf: &mut PdfDocument) -> Vec<u8> {
    let mut data = Vec::new();
    pdf.save_to(&mut data).unwrap();
    data
}

fn add_explicit_defaults(pdf: &mut PdfDocument) {
    for object in pdf.objects.values_mut() {
        let dict = match object {
            Object::Dictionary(dict) => dict,
            Object::Stream(stream) => &mut stream.dict,
            _ => continue,
        };
        let Some(kind) = dict.get(b"ShadingType").ok().and_then(|v| v.as_i64().ok()) else {
            continue;
        };
        if !dict.has(b"BBox") {
            // This neutral clip contains the visible inverse-mapped region of
            // every authored fixture, including the sheared radial pattern.
            dict.set(
                "BBox",
                vec![(-1024).into(), (-1024).into(), 1024.into(), 1024.into()],
            );
        }
        if !dict.has(b"Domain") && matches!(kind, 1..=3) {
            dict.set(
                "Domain",
                if kind == 1 {
                    vec![0.into(), 1.into(), 0.into(), 1.into()]
                } else {
                    vec![0.into(), 1.into()]
                },
            );
        }
    }
}

fn indirect_parameters(pdf: &mut PdfDocument) {
    let mut changes = Vec::new();
    for (id, object) in &pdf.objects {
        let dict = match object {
            Object::Dictionary(dict) => dict,
            Object::Stream(stream) => &stream.dict,
            _ => continue,
        };
        let keys: &[&[u8]] = if dict.has(b"ShadingType") {
            &[
                b"ShadingType",
                b"Coords",
                b"Extend",
                b"Domain",
                b"BBox",
                b"Matrix",
                b"Background",
                b"Decode",
                b"BitsPerCoordinate",
                b"BitsPerComponent",
                b"BitsPerFlag",
                b"VerticesPerRow",
            ]
        } else if dict.has(b"PatternType") {
            &[b"PatternType", b"Matrix"]
        } else {
            continue;
        };
        for key in keys {
            if let Ok(value) = dict.get(key) {
                changes.push((*id, key.to_vec(), value.clone()));
            }
        }
    }
    for (id, key, value) in changes {
        let value = match value {
            Object::Array(values) => Object::Array(
                values
                    .into_iter()
                    .map(|value| Object::Reference(pdf.add_object(value)))
                    .collect(),
            ),
            value => value,
        };
        let value_id = pdf.add_object(value);
        let alias = pdf.add_object(Object::Reference(value_id));
        dictionary_mut(pdf, id).set(key, Object::Reference(alias));
    }
}

fn check(source: &[u8], reference: &[u8], name: &str) {
    let mut pdf = PdfDocument::load_mem(source).unwrap();
    add_explicit_defaults(&mut pdf);
    let direct = OfdDocument::from_pdf_bytes(&serialize(&mut pdf)).unwrap();
    indirect_parameters(&mut pdf);
    let indirect = OfdDocument::from_pdf_bytes(&serialize(&mut pdf))
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_eq!(
        indirect.page_svg(0).unwrap(),
        direct.page_svg(0).unwrap(),
        "{name}: reference encoding changed vector geometry"
    );
    let png = indirect.page_png(0, 144.).unwrap();
    assert_eq!(
        png,
        direct.page_png(0, 144.).unwrap(),
        "{name}: reference encoding changed raster output"
    );
    let actual = image::load_from_memory(&png).unwrap().to_rgb8();
    let expected = image::load_from_memory(reference).unwrap().to_rgb8();
    common::assert_raster_matches(&actual, &expected, name);
    let reopened = OfdDocument::from_bytes(&indirect.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_png(0, 144.).unwrap(), png);
    assert!(reopened.validate_references().unwrap().is_empty());
}

macro_rules! reference_case {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check(
                include_bytes!(concat!("fixtures/shading-pattern-", $name, ".pdf")),
                include_bytes!(concat!("fixtures/shading-pattern-", $name, ".png")),
                $name,
            );
        }
    };
}
reference_case!(axial, "axial");
reference_case!(radial, "radial");
reference_case!(affine_radial, "affine-radial");
reference_case!(bbox_background, "bbox-background");
reference_case!(plane, "plane");
reference_case!(mesh, "mesh");
reference_case!(lattice, "lattice");
reference_case!(coons, "coons");
reference_case!(tensor, "tensor");

fn axial_pdf() -> PdfDocument {
    let mut pdf =
        PdfDocument::load_mem(include_bytes!("fixtures/shading-pattern-axial.pdf")).unwrap();
    add_explicit_defaults(&mut pdf);
    pdf
}

#[test]
fn unresolved_and_cyclic_shading_parameters_do_not_fall_back_to_defaults() {
    for key in [
        b"ShadingType".as_slice(),
        b"Coords",
        b"Extend",
        b"Domain",
        b"BBox",
    ] {
        for cyclic in [false, true] {
            let mut pdf = axial_pdf();
            let id = if cyclic {
                let id = pdf.add_object(Object::Null);
                pdf.objects.insert(id, Object::Reference(id));
                id
            } else {
                (1_000_000, 0)
            };
            dictionary_mut(&mut pdf, (6, 0)).set(key, Object::Reference(id));
            let error = OfdDocument::from_pdf_bytes(&serialize(&mut pdf))
                .expect_err("invalid reference was accepted");
            assert!(
                error
                    .to_string()
                    .contains(std::str::from_utf8(key).unwrap()),
                "{error}"
            );
        }
    }
}

#[test]
fn malformed_referenced_arrays_and_components_remain_errors() {
    for (key, value) in [
        (b"Coords".as_slice(), Object::Array(vec![0.into(); 3])),
        (b"Domain".as_slice(), Object::Null),
        (
            b"Extend".as_slice(),
            Object::Array(vec![1.into(), 1.into()]),
        ),
        (b"BBox".as_slice(), Object::Array(vec![0.into(); 4])),
        (b"ShadingType".as_slice(), Object::Real(2.5)),
    ] {
        let mut pdf = axial_pdf();
        let value = pdf.add_object(value);
        dictionary_mut(&mut pdf, (6, 0)).set(key, Object::Reference(value));
        assert!(
            OfdDocument::from_pdf_bytes(&serialize(&mut pdf)).is_err(),
            "{}",
            String::from_utf8_lossy(key)
        );
    }
    let mut pdf = axial_pdf();
    let not_numeric = pdf.add_object(Object::Name(b"NotANumber".to_vec()));
    dictionary_mut(&mut pdf, (6, 0)).set(
        "Coords",
        vec![
            Object::Reference(not_numeric),
            32.into(),
            84.into(),
            32.into(),
        ],
    );
    assert!(OfdDocument::from_pdf_bytes(&serialize(&mut pdf)).is_err());
}

#[test]
fn direct_shading_operators_share_reference_resolution_with_patterns() {
    let mut pdf = axial_pdf();
    let page = dictionary_mut(&mut pdf, (3, 0));
    page.set(
        "Resources",
        dictionary! { "Shading"=>dictionary!{ "S"=>Object::Reference((6,0)) } },
    );
    pdf.get_object_mut((4, 0))
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_plain_content(
            lopdf::content::Content {
                operations: vec![Operation::new("sh", vec![Object::Name(b"S".to_vec())])],
            }
            .encode()
            .unwrap(),
        );
    let direct = OfdDocument::from_pdf_bytes(&serialize(&mut pdf)).unwrap();
    indirect_parameters(&mut pdf);
    let indirect = OfdDocument::from_pdf_bytes(&serialize(&mut pdf)).unwrap();
    assert_eq!(indirect.page_svg(0).unwrap(), direct.page_svg(0).unwrap());
    assert_eq!(
        indirect.page_png(0, 72.).unwrap(),
        direct.page_png(0, 72.).unwrap()
    );
}

#[test]
fn authored_indirect_parameters_match_independent_rasters() {
    for (source, reference, direct_source, pattern) in [
        (
            include_bytes!("fixtures/shading-pattern-indirect-params.pdf").as_slice(),
            include_bytes!("fixtures/shading-pattern-indirect-params.png").as_slice(),
            include_bytes!("fixtures/shading-pattern-indirect-params-reference.pdf").as_slice(),
            true,
        ),
        (
            include_bytes!("fixtures/shading-pattern-indirect-direct-sh.pdf").as_slice(),
            include_bytes!("fixtures/shading-pattern-indirect-direct-sh.png").as_slice(),
            include_bytes!("fixtures/shading-pattern-indirect-direct-sh-reference.pdf").as_slice(),
            false,
        ),
    ] {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        let direct = OfdDocument::from_pdf_bytes(direct_source).unwrap();
        assert_eq!(document.page_svg(0).unwrap(), direct.page_svg(0).unwrap());
        let png = document.page_png(0, 144.).unwrap();
        assert_eq!(png, direct.page_png(0, 144.).unwrap());
        let actual = image::load_from_memory(&png).unwrap().to_rgb8();
        let expected = image::load_from_memory(reference).unwrap().to_rgb8();
        common::assert_raster_matches(&actual, &expected, "authored indirect parameters");
        assert_eq!(document.page_svg(0).unwrap().contains("<pattern"), pattern);
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_png(0, 144.).unwrap(), png);
        assert!(reopened.validate_references().unwrap().is_empty());
    }
}
