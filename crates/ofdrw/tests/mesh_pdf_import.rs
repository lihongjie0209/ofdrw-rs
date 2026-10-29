mod common;

use ofdrw::OfdDocument;

const FIXTURES: &[(&[u8], &[u8])] = &[
    (
        include_bytes!("fixtures/mesh-free-rgb.pdf"),
        include_bytes!("fixtures/mesh-free-rgb.png"),
    ),
    (
        include_bytes!("fixtures/mesh-lattice-rgb.pdf"),
        include_bytes!("fixtures/mesh-lattice-rgb.png"),
    ),
    (
        include_bytes!("fixtures/mesh-packed-gray.pdf"),
        include_bytes!("fixtures/mesh-packed-gray.png"),
    ),
    (
        include_bytes!("fixtures/mesh-wide-rgb.pdf"),
        include_bytes!("fixtures/mesh-wide-rgb.png"),
    ),
    (
        include_bytes!("fixtures/mesh-lattice-clipped.pdf"),
        include_bytes!("fixtures/mesh-lattice-clipped.png"),
    ),
    (
        include_bytes!("fixtures/mesh-cmyk.pdf"),
        include_bytes!("fixtures/mesh-cmyk.png"),
    ),
    (
        include_bytes!("fixtures/mesh-calgray.pdf"),
        include_bytes!("fixtures/mesh-calgray.png"),
    ),
    (
        include_bytes!("fixtures/mesh-function.pdf"),
        include_bytes!("fixtures/mesh-function.png"),
    ),
    (
        include_bytes!("fixtures/mesh-function-array.pdf"),
        include_bytes!("fixtures/mesh-function-array.png"),
    ),
    (
        include_bytes!("fixtures/mesh-sampled-function.pdf"),
        include_bytes!("fixtures/mesh-sampled-function.png"),
    ),
    (
        include_bytes!("fixtures/mesh-stitching-function.pdf"),
        include_bytes!("fixtures/mesh-stitching-function.png"),
    ),
    (
        include_bytes!("fixtures/mesh-calculator-function.pdf"),
        include_bytes!("fixtures/mesh-calculator-function.png"),
    ),
    (
        include_bytes!("fixtures/patch-coons-flat.pdf"),
        include_bytes!("fixtures/patch-coons-flat.png"),
    ),
    (
        include_bytes!("fixtures/patch-coons-curved.pdf"),
        include_bytes!("fixtures/patch-coons-curved.png"),
    ),
    (
        include_bytes!("fixtures/patch-tensor-curved.pdf"),
        include_bytes!("fixtures/patch-tensor-curved.png"),
    ),
    (
        include_bytes!("fixtures/patch-tensor-folded.pdf"),
        include_bytes!("fixtures/patch-tensor-folded.png"),
    ),
    (
        include_bytes!("fixtures/patch-coons-reuse-1.pdf"),
        include_bytes!("fixtures/patch-coons-reuse-1.png"),
    ),
    (
        include_bytes!("fixtures/patch-coons-reuse-2.pdf"),
        include_bytes!("fixtures/patch-coons-reuse-2.png"),
    ),
    (
        include_bytes!("fixtures/patch-coons-reuse-3.pdf"),
        include_bytes!("fixtures/patch-coons-reuse-3.png"),
    ),
    (
        include_bytes!("fixtures/patch-tensor-reuse.pdf"),
        include_bytes!("fixtures/patch-tensor-reuse.png"),
    ),
    (
        include_bytes!("fixtures/patch-coons-function.pdf"),
        include_bytes!("fixtures/patch-coons-function.png"),
    ),
    (
        include_bytes!("fixtures/patch-tensor-clipped.pdf"),
        include_bytes!("fixtures/patch-tensor-clipped.png"),
    ),
    (
        include_bytes!("fixtures/patch-coons-wide.pdf"),
        include_bytes!("fixtures/patch-coons-wide.png"),
    ),
    (
        include_bytes!("fixtures/patch-coons-packed.pdf"),
        include_bytes!("fixtures/patch-coons-packed.png"),
    ),
    (
        include_bytes!("fixtures/patch-coons-calgray.pdf"),
        include_bytes!("fixtures/patch-coons-calgray.png"),
    ),
    (
        include_bytes!("fixtures/patch-tensor-cmyk.pdf"),
        include_bytes!("fixtures/patch-tensor-cmyk.png"),
    ),
];

fn assert_mesh_raster(
    source: &[u8],
    reference: &[u8],
    label: &str,
) -> (OfdDocument, String, Vec<u8>) {
    let document = OfdDocument::from_pdf_bytes(source).unwrap();
    let svg = document.page_svg(0).unwrap();
    let png = document.page_png(0, 72.).unwrap();
    let actual = image::load_from_memory(&png).unwrap().to_rgb8();
    let expected = image::load_from_memory(reference).unwrap().to_rgb8();
    common::assert_raster_matches(&actual, &expected, label);
    (document, svg, png)
}

fn assert_mesh_round_trip(document: &OfdDocument, svg: &str, png: &[u8]) {
    let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
    assert_eq!(reopened.page_svg(0).unwrap(), svg);
    assert_eq!(reopened.page_png(0, 72.).unwrap(), png);
    assert!(reopened.validate_references().unwrap().is_empty());
    assert!(lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok());
}

#[test]
fn pdf_meshes_match_independent_rasters_and_survive_ofd_round_trips() {
    for (index, (source, reference)) in FIXTURES.iter().enumerate() {
        let (document, svg, png) = assert_mesh_raster(source, reference, &index.to_string());
        assert_mesh_round_trip(&document, &svg, &png);
    }
}

#[test]
fn pdf_mesh_topology_and_graphics_state_are_retained() {
    for (index, (source, _)) in FIXTURES.iter().enumerate() {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        let page = document.pages().unwrap().remove(0);
        let xml = std::str::from_utf8(document.entry(&page.path).unwrap().unwrap()).unwrap();
        if index == 1 || index == 4 {
            assert!(xml.contains("LaGouraudShd VerticesPerRow=\"3\""));
        } else {
            assert!(xml.contains("GouraudShd"));
        }
        if index == 0 {
            assert!(xml.contains("EdgeFlag=\"1\""));
            assert!(xml.contains("EdgeFlag=\"2\""));
            assert!(!xml.contains("EdgeFlag=\"3\""));
        }
        if index == 4 {
            assert!(xml.contains("Alpha=\"128\""));
            assert!(xml.contains("BlendMode=\"Multiply\""));
            assert!(xml.contains("Clips"));
            assert!(!xml.contains("BackColor"));
        }
    }
}

#[test]
fn malformed_mesh_metadata_and_partial_vertex_records_return_errors() {
    let original = lopdf::Document::load_mem(FIXTURES[0].0).unwrap();
    for (key, value) in [
        ("BitsPerCoordinate", lopdf::Object::Integer(3)),
        ("BitsPerComponent", lopdf::Object::Integer(32)),
        ("BitsPerFlag", lopdf::Object::Integer(1)),
        ("Decode", lopdf::Object::Array(vec![0.into(), 1.into()])),
        ("ColorSpace", lopdf::Object::Name(b"Pattern".to_vec())),
    ] {
        let mut pdf = original.clone();
        pdf.get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set(key, value);
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        assert!(OfdDocument::from_pdf_bytes(&bytes).is_err(), "{key}");
    }
    for length in [0, 1, 5, 6, 7, 13] {
        let mut pdf = original.clone();
        let stream = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
        stream.decompress().unwrap();
        stream.set_content(vec![0; length]);
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        assert!(
            OfdDocument::from_pdf_bytes(&bytes).is_err(),
            "length {length}"
        );
    }
}

#[test]
fn folded_tensor_fixture_has_opposite_triangle_orientations() {
    let (document, svg, png) = assert_mesh_raster(
        include_bytes!("fixtures/patch-tensor-folded.pdf"),
        include_bytes!("fixtures/patch-tensor-folded.png"),
        "folded tensor",
    );
    let page = document.pages().unwrap().remove(0);
    let xml = std::str::from_utf8(document.entry(&page.path).unwrap().unwrap()).unwrap();
    let parsed = roxmltree::Document::parse(xml).unwrap();
    let points = parsed
        .descendants()
        .filter(|node| node.tag_name().name() == "Point")
        .map(|node| {
            [
                node.attribute("X").unwrap().parse::<f64>().unwrap(),
                node.attribute("Y").unwrap().parse::<f64>().unwrap(),
            ]
        })
        .collect::<Vec<_>>();
    let areas = points
        .chunks_exact(3)
        .map(|p| {
            (p[1][1] - p[2][1]) * (p[0][0] - p[2][0]) + (p[2][0] - p[1][0]) * (p[0][1] - p[2][1])
        })
        .collect::<Vec<_>>();
    assert!(areas.iter().any(|area| *area > 1e-6));
    assert!(areas.iter().any(|area| *area < -1e-6));
    assert_mesh_round_trip(&document, &svg, &png);
}
