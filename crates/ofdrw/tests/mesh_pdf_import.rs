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
];

#[test]
fn pdf_meshes_match_independent_rasters_and_survive_ofd_round_trips() {
    for (index, (source, reference)) in FIXTURES.iter().enumerate() {
        let document = OfdDocument::from_pdf_bytes(source).unwrap();
        let svg = document.page_svg(0).unwrap();
        let png = document.page_png(0, 72.).unwrap();
        let actual = image::load_from_memory(&png).unwrap().to_rgb8();
        let expected = image::load_from_memory(reference).unwrap().to_rgb8();
        assert_eq!(actual.dimensions(), expected.dimensions());
        // Ignore a one-pixel boundary neighborhood, where independent triangle
        // rasterizers have different coverage and antialiasing conventions.
        let mut error = 0_u64;
        let mut samples = 0_u64;
        for y in 1..actual.height() - 1 {
            for x in 1..actual.width() - 1 {
                let neighborhood =
                    (y - 1..=y + 1).flat_map(|y| (x - 1..=x + 1).map(move |x| (x, y)));
                if neighborhood
                    .clone()
                    .any(|(x, y)| expected.get_pixel(x, y).0 == [255; 3])
                {
                    continue;
                }
                for channel in 0..3 {
                    error += u64::from(
                        actual.get_pixel(x, y)[channel].abs_diff(expected.get_pixel(x, y)[channel]),
                    );
                    samples += 1;
                }
            }
        }
        let mean = error as f64 / samples as f64;
        assert!(
            samples > 1000 && mean < 4.,
            "mesh {index}: mean error {mean} over {samples} samples"
        );
        let reopened = OfdDocument::from_bytes(&document.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_svg(0).unwrap(), svg);
        assert_eq!(reopened.page_png(0, 72.).unwrap(), png);
        assert!(reopened.validate_references().unwrap().is_empty());
        assert!(lopdf::Document::load_mem(&reopened.to_pdf().unwrap()).is_ok());
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
