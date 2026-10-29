//! Composite previews of single-colorant PDF Separation spaces.

use super::{
    parse_color_space_inner, pdf_resolved_value, pdf_shading_segments, Arc, Object, OfdError,
    PdfColorSpace, PdfDocument, RenderingIntent,
};
use crate::GradientSegment;

pub(super) fn parse(
    pdf: Option<&PdfDocument>,
    resources: Option<&Object>,
    values: &[Object],
    depth: usize,
    remap_device: bool,
) -> Result<PdfColorSpace, OfdError> {
    let [_, name, alternate, transform] = values else {
        return Err(OfdError::Invalid(
            "PDF Separation needs four entries".into(),
        ));
    };
    let name = pdf_resolved_value(pdf, name, "Separation colorant")?
        .as_name()
        .map_err(|_| OfdError::Invalid("PDF Separation colorant is not a name".into()))?;
    if name.len() > 1024 {
        return Err(OfdError::Limit(
            "PDF Separation colorant name is too long".into(),
        ));
    }
    // All and None never invoke the alternate space or its tint transform.
    if name == b"None" {
        return Ok(PdfColorSpace::SeparationNone);
    }
    if name == b"All" {
        return Ok(PdfColorSpace::SeparationAll);
    }
    let alternate = parse_color_space_inner(pdf, resources, alternate, depth + 1, remap_device)?;
    if matches!(
        alternate,
        PdfColorSpace::Indexed { .. }
            | PdfColorSpace::Separation { .. }
            | PdfColorSpace::SeparationAll
            | PdfColorSpace::SeparationNone
    ) {
        return Err(OfdError::Invalid(
            "PDF Separation alternate must be a device or CIE color space".into(),
        ));
    }
    let empty;
    let pdf = match pdf {
        Some(pdf) => pdf,
        None => {
            empty = PdfDocument::new();
            &empty
        }
    };
    let intents = [
        RenderingIntent::Perceptual,
        RenderingIntent::RelativeColorimetric,
        RenderingIntent::Saturation,
        RenderingIntent::AbsoluteColorimetric,
    ];
    let colors = if alternate.intent_dependent() {
        let mut result = Vec::new();
        for intent in intents {
            result.push(Arc::new(pdf_shading_segments(
                pdf,
                transform,
                &alternate,
                intent,
                [0., 1.],
            )?));
        }
        result
            .try_into()
            .map_err(|_| OfdError::Invalid("invalid Separation intent table".into()))?
    } else {
        let colors = Arc::new(pdf_shading_segments(
            pdf,
            transform,
            &alternate,
            RenderingIntent::RelativeColorimetric,
            [0., 1.],
        )?);
        std::array::from_fn(|_| colors.clone())
    };
    if colors
        .iter()
        .any(|stops: &Arc<Vec<GradientSegment>>| stops.is_empty())
    {
        return Err(OfdError::Invalid(
            "PDF Separation has no tint samples".into(),
        ));
    }
    Ok(PdfColorSpace::Separation { colors })
}

pub(super) fn color(stops: &[GradientSegment], tint: f64) -> [u8; 3] {
    let index = stops.partition_point(|stop| stop.position.unwrap_or(0.) <= tint);
    if index == 0 {
        return stops[0].color;
    }
    if index == stops.len() {
        return stops[index - 1].color;
    }
    let before = &stops[index - 1];
    let after = &stops[index];
    let a = before.position.unwrap_or(0.);
    let fraction = (tint - a) / (after.position.unwrap_or(1.) - a);
    std::array::from_fn(|channel| {
        (f64::from(before.color[channel]) * (1. - fraction)
            + f64::from(after.color[channel]) * fraction)
            .round() as u8
    })
}

#[cfg(test)]
mod tests {
    use super::super::{parse_color_space, selected_color_space};
    use super::*;
    use lopdf::dictionary;

    fn function() -> Object {
        Object::Dictionary(
            dictionary! {"FunctionType"=>2,"Domain"=>vec![0.into(),1.into()],
            "C0"=>vec![1.into(),1.into(),1.into()],"C1"=>vec![1.into(),0.into(),0.into()],"N"=>2},
        )
    }

    fn space(name: &[u8], alternate: Object, function: Object) -> Object {
        Object::Array(vec![
            Object::Name(b"Separation".to_vec()),
            Object::Name(name.to_vec()),
            alternate,
            function,
        ])
    }

    fn rgb_space(name: &[u8]) -> Object {
        space(name, Object::Name(b"DeviceRGB".to_vec()), function())
    }

    #[test]
    fn ordinary_spots_have_subtractive_initial_tint_and_bounded_rgb_lookup() {
        let spot = parse_color_space(None, None, &rgb_space(b"Red"), 0).unwrap();
        assert_eq!(spot.channels(), 1);
        assert_eq!(spot.initial_components(), [1.]);
        assert_eq!(spot.default_decode(65535), [0., 1.]);
        assert!(!spot.nonpainting());
        for (tint, expected) in [(0., [255; 3]), (0.5, [255, 191, 191]), (1., [255, 0, 0])] {
            assert_eq!(spot.color(&[tint]).unwrap(), expected);
        }
        for values in [
            vec![],
            vec![0., 0.],
            vec![f64::NAN],
            vec![-0.01],
            vec![1.01],
        ] {
            assert!(spot.color(&values).is_err());
        }
    }

    #[test]
    fn all_and_none_ignore_alternate_colors_and_do_not_share_painting_semantics() {
        let blue = Object::Dictionary(
            dictionary! {"FunctionType"=>2,"Domain"=>vec![0.into(),1.into()],
            "C0"=>vec![0.into(),0.into(),1.into()],"C1"=>vec![0.into(),0.into(),1.into()],"N"=>1},
        );
        let none = parse_color_space(
            None,
            None,
            &space(b"None", Object::Name(b"DeviceRGB".to_vec()), blue.clone()),
            0,
        )
        .unwrap();
        let all = parse_color_space(
            None,
            None,
            &space(b"All", Object::Name(b"DeviceRGB".to_vec()), blue),
            0,
        )
        .unwrap();
        assert!(none.nonpainting());
        assert!(!all.nonpainting());
        assert_eq!(all.initial_components(), [1.]);
        for (tint, expected) in [(0., [255; 3]), (0.5, [128; 3]), (1., [0; 3])] {
            assert_eq!(all.color(&[tint]).unwrap(), expected);
        }
    }

    #[test]
    fn sampled_stitching_and_calculator_tints_keep_their_function_semantics() {
        for (source, tint, expected) in [
            (
                include_bytes!("../../tests/fixtures/separation-sampled.pdf").as_slice(),
                0.25,
                [128, 255, 112],
            ),
            (
                include_bytes!("../../tests/fixtures/separation-stitching.pdf").as_slice(),
                0.5,
                [0, 255, 0],
            ),
            (
                include_bytes!("../../tests/fixtures/separation-calculator.pdf").as_slice(),
                0.5,
                [128, 128, 64],
            ),
        ] {
            let pdf = PdfDocument::load_mem(source).unwrap();
            let resources = pdf
                .get_object((3, 0))
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"Resources")
                .unwrap();
            let spot = selected_color_space(&pdf, resources, b"CS").unwrap();
            assert_eq!(spot.color(&[tint]).unwrap(), expected);
        }
    }

    #[test]
    fn indexed_spots_preserve_tint_lookup_and_none_paint_suppression() {
        for name in [b"Red".as_slice(), b"None"] {
            let object = Object::Array(vec![
                Object::Name(b"Indexed".to_vec()),
                rgb_space(name),
                1.into(),
                Object::String(vec![0, 255], lopdf::StringFormat::Hexadecimal),
            ]);
            let indexed = parse_color_space(None, None, &object, 0).unwrap();
            assert_eq!(indexed.nonpainting(), name == b"None");
            assert_eq!(indexed.initial_components(), [0.]);
            if name != b"None" {
                assert_eq!(indexed.color(&[0.]).unwrap(), [255; 3]);
                assert_eq!(indexed.color(&[1.]).unwrap(), [255, 0, 0]);
            }
        }
    }

    #[test]
    fn invalid_spot_names_alternates_and_transform_dimensions_return_errors() {
        let invalid = [
            Object::Array(vec![
                Object::Name(b"Separation".to_vec()),
                Object::Name(b"Red".to_vec()),
            ]),
            Object::Array(vec![
                Object::Name(b"Separation".to_vec()),
                0.into(),
                Object::Name(b"DeviceRGB".to_vec()),
                function(),
            ]),
            space(
                b"Spot",
                Object::Array(vec![
                    Object::Name(b"Indexed".to_vec()),
                    Object::Name(b"DeviceRGB".to_vec()),
                    0.into(),
                    Object::String(vec![0, 0, 0], lopdf::StringFormat::Hexadecimal),
                ]),
                function(),
            ),
            space(b"Spot", rgb_space(b"Inner"), function()),
            space(
                b"Spot",
                Object::Name(b"DeviceRGB".to_vec()),
                Object::Dictionary(
                    dictionary! {"FunctionType"=>2,"C0"=>vec![0.into()],"C1"=>vec![1.into()],"N"=>1},
                ),
            ),
        ];
        for object in invalid {
            assert!(parse_color_space(None, None, &object, 0).is_err());
        }
        assert!(matches!(
            parse_color_space(None, None, &rgb_space(&vec![b'x'; 1025]), 0),
            Err(OfdError::Limit(_))
        ));
    }

    #[test]
    fn icc_alternates_keep_all_four_rendering_intents() {
        let mut pdf = PdfDocument::new();
        let profile = pdf.add_object(lopdf::Stream::new(
            dictionary! {"N"=>4},
            include_bytes!("../../tests/fixtures/snap-newsprint-cmyk.icc").to_vec(),
        ));
        let alternate = Object::Array(vec![
            Object::Name(b"ICCBased".to_vec()),
            Object::Reference(profile),
        ]);
        let transform = Object::Dictionary(
            dictionary! {"FunctionType"=>2,"Domain"=>vec![0.into(),1.into()],
            "C0"=>vec![0.into(),0.into(),0.into(),0.into()],"C1"=>vec![Object::Real(0.8),Object::Real(0.7),Object::Real(0.6),Object::Real(0.1)],"N"=>1},
        );
        let spot = parse_color_space(
            Some(&pdf),
            None,
            &space(b"Proof", alternate.clone(), transform),
            0,
        )
        .unwrap();
        let alternate = parse_color_space(Some(&pdf), None, &alternate, 0).unwrap();
        for intent in [
            RenderingIntent::Perceptual,
            RenderingIntent::RelativeColorimetric,
            RenderingIntent::Saturation,
            RenderingIntent::AbsoluteColorimetric,
        ] {
            assert_eq!(
                spot.color_with_intent(&[0.], intent).unwrap(),
                alternate
                    .color_with_intent(&[0., 0., 0., 0.], intent)
                    .unwrap()
            );
            let actual = spot.color_with_intent(&[0.5], intent).unwrap();
            let expected = alternate
                .color_with_intent(&[0.4, 0.35, 0.3, 0.05], intent)
                .unwrap();
            assert!(actual
                .into_iter()
                .zip(expected)
                .all(|(a, b)| a.abs_diff(b) <= 3));
        }
    }
}
