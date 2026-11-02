use ofdrw::CanvasUnits;

fn from_bits(value: &serde_json::Value) -> f64 {
    f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}

#[test]
fn pixel_and_mm_conversions_match_independent_java_results_bit_for_bit() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/canvas-units-java.json")).unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 329);
    for case in cases {
        let units = CanvasUnits::new(from_bits(&case["ppm_bits"])).unwrap();
        if case["op"] == "mm" {
            let actual = units.mm(i32::try_from(case["input"].as_i64().unwrap()).unwrap());
            assert_eq!(
                actual.to_bits(),
                from_bits(&case["expected_bits"]).to_bits(),
                "{case}"
            );
        } else {
            assert_eq!(
                i64::from(units.pixel(from_bits(&case["input_bits"]))),
                case["expected"].as_i64().unwrap(),
                "{case}"
            );
        }
    }
}

#[test]
fn conversion_density_rejects_invalid_values_and_preserves_java_default() {
    for density in [0., -0., -1., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(CanvasUnits::new(density).is_err(), "{density}");
    }
    let units = CanvasUnits::default();
    assert_eq!(units.pixels_per_mm(), 3.78);
    assert_eq!(units.mm(378), 100.);
    assert_eq!(units.pixel(100.), 378);
}

#[test]
fn pixel_cast_truncates_toward_zero_and_saturates_instead_of_wrapping() {
    let units = CanvasUnits::new(4.).unwrap();
    assert_eq!(units.pixel(0.49), 1);
    assert_eq!(units.pixel(-0.49), -1);
    assert_eq!(units.pixel(f64::NAN), 0);
    assert_eq!(units.pixel(f64::INFINITY), i32::MAX);
    assert_eq!(units.pixel(f64::NEG_INFINITY), i32::MIN);
    assert_eq!(units.pixel(f64::MAX), i32::MAX);
    assert_eq!(units.pixel(-f64::MAX), i32::MIN);
}
