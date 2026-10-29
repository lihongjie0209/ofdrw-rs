//! Two-input PDF functions and bounded function-plane rasterization.

use super::{
    import_image_stream, parse_color_space, pdf_byte_stream_data, pdf_function_integer,
    pdf_function_numbers, pdf_function_type, pdf_resolved_value, pdf_sample_cubic_coefficients,
    pdf_shading_function_range, read_pdf_sample, GraphicsState, Object, OfdDocument, OfdError,
    PdfColorSpace, PdfDocument, PdfPageContext,
};
use crate::pdf_calculator::Calculator;

const MAX_SAMPLES: usize = 1_000_000;
const MAX_PIXELS: usize = 4_000_000;
const MAX_WORK: usize = 100_000_000;

fn invalid(reason: &str) -> OfdError {
    OfdError::Invalid(format!("invalid PDF function shading: {reason}"))
}

fn limit(reason: &str) -> OfdError {
    OfdError::Limit(format!("PDF function shading {reason}"))
}

struct Function {
    domain: [f64; 4],
    range: Vec<f64>,
    kind: Kind,
}

enum Kind {
    Sampled {
        size: [usize; 2],
        encode: [f64; 4],
        decode: Vec<f64>,
        samples: Vec<f64>,
        cubic: bool,
    },
    Calculator(Calculator),
}

fn domain(
    pdf: &PdfDocument,
    dict: &lopdf::Dictionary,
    required: bool,
) -> Result<[f64; 4], OfdError> {
    let values = if required || dict.has(b"Domain") {
        pdf_function_numbers(pdf, dict, b"Domain", 4)?
    } else {
        vec![0., 1., 0., 1.]
    };
    let values: [f64; 4] = values
        .try_into()
        .map_err(|_| invalid("Domain dimensions"))?;
    if values[0] >= values[1] || values[2] >= values[3] {
        return Err(invalid("Domain bounds"));
    }
    Ok(values)
}

impl Function {
    fn parse(
        pdf: &PdfDocument,
        object: &Object,
        outputs: usize,
        shading_domain: [f64; 4],
        total: &mut usize,
    ) -> Result<Self, OfdError> {
        let object = pdf_resolved_value(Some(pdf), object, "two-input Function")?;
        let stream = object
            .as_stream()
            .map_err(|_| invalid("Function must be a stream"))?;
        let dict = &stream.dict;
        let domain = domain(pdf, dict, true)?;
        for axis in 0..2 {
            if domain[axis * 2] > shading_domain[axis * 2]
                || domain[axis * 2 + 1] < shading_domain[axis * 2 + 1]
            {
                return Err(invalid("Function Domain must contain shading Domain"));
            }
        }
        let range = pdf_shading_function_range(pdf, dict, outputs)?
            .ok_or_else(|| invalid("Function has no Range"))?;
        let kind = match pdf_function_type(pdf, dict)? {
            0 => {
                let sizes = pdf_function_numbers(pdf, dict, b"Size", 2)?;
                if sizes
                    .iter()
                    .any(|n| *n < 1. || n.fract() != 0. || *n > MAX_SAMPLES as f64)
                {
                    return Err(invalid("sample dimensions"));
                }
                let size = [sizes[0] as usize, sizes[1] as usize];
                let count = size[0]
                    .checked_mul(size[1])
                    .and_then(|count| count.checked_mul(outputs))
                    .ok_or_else(|| limit("sample count overflow"))?;
                *total = total
                    .checked_add(count)
                    .ok_or_else(|| limit("sample count overflow"))?;
                if *total > MAX_SAMPLES {
                    return Err(limit("has more than one million samples"));
                }
                let bits = pdf_function_integer(pdf, dict, b"BitsPerSample", None)?;
                if !matches!(bits, 1 | 2 | 4 | 8 | 12 | 16 | 24 | 32) {
                    return Err(invalid("BitsPerSample"));
                }
                let cubic = match pdf_function_integer(pdf, dict, b"Order", Some(1))? {
                    1 => false,
                    3 => true,
                    _ => return Err(invalid("interpolation Order")),
                };
                let encode = if dict.has(b"Encode") {
                    pdf_function_numbers(pdf, dict, b"Encode", 4)?
                        .try_into()
                        .map_err(|_| invalid("Encode"))?
                } else {
                    [0., (size[0] - 1) as f64, 0., (size[1] - 1) as f64]
                };
                let decode = if dict.has(b"Decode") {
                    pdf_function_numbers(pdf, dict, b"Decode", outputs * 2)?
                } else {
                    range.clone()
                };
                let data = pdf_byte_stream_data(stream, Some(pdf), "two-input sampled Function")?;
                if data.len() < (count * bits as usize).div_ceil(8) {
                    return Err(invalid("truncated samples"));
                }
                let maximum = ((1_u64 << bits) - 1) as f64;
                let samples = (0..count)
                    .map(|i| f64::from(read_pdf_sample(&data, i, bits as usize)) / maximum)
                    .collect();
                Kind::Sampled {
                    size,
                    encode,
                    decode,
                    samples,
                    cubic,
                }
            }
            4 => Kind::Calculator(Calculator::parse(&pdf_byte_stream_data(
                stream,
                Some(pdf),
                "two-input calculator Function",
            )?)?),
            _ => return Err(invalid("Function must have two inputs (Type 0 or 4)")),
        };
        Ok(Self {
            domain,
            range,
            kind,
        })
    }

    fn work_cost(&self) -> usize {
        match &self.kind {
            Kind::Calculator(calculator) => calculator.work_cost(),
            Kind::Sampled { cubic, .. } => self.range.len() / 2 * if *cubic { 16 } else { 4 },
        }
    }

    fn evaluate(&self, mut input: [f64; 2]) -> Result<Vec<f64>, OfdError> {
        for (axis, value) in input.iter_mut().enumerate() {
            *value = value.clamp(self.domain[2 * axis], self.domain[2 * axis + 1]);
        }
        let outputs = self.range.len() / 2;
        let values = match &self.kind {
            Kind::Calculator(calculator) => calculator.evaluate(&input, outputs)?.values,
            Kind::Sampled {
                size,
                encode,
                decode,
                samples,
                cubic,
            } => {
                let at: [f64; 2] = std::array::from_fn(|axis| {
                    let i = axis * 2;
                    let t = (input[axis] - self.domain[i]) / (self.domain[i + 1] - self.domain[i]);
                    (encode[i] * (1. - t) + encode[i + 1] * t).clamp(0., (size[axis] - 1) as f64)
                });
                let interpolate = |axis: usize, get: &dyn Fn(usize) -> f64| {
                    let first = at[axis].floor() as usize;
                    let fraction = at[axis] - first as f64;
                    if *cubic && size[axis] >= 3 {
                        let coefficients = pdf_sample_cubic_coefficients([
                            get(first.saturating_sub(1)),
                            get(first),
                            get((first + 1).min(size[axis] - 1)),
                            get((first + 2).min(size[axis] - 1)),
                        ]);
                        ((coefficients[3] * fraction + coefficients[2]) * fraction
                            + coefficients[1])
                            * fraction
                            + coefficients[0]
                    } else {
                        get(first) * (1. - fraction)
                            + get((first + 1).min(size[axis] - 1)) * fraction
                    }
                };
                (0..outputs)
                    .map(|channel| {
                        let sample = interpolate(1, &|y| {
                            interpolate(0, &|x| samples[(y * size[0] + x) * outputs + channel])
                        });
                        decode[channel * 2]
                            + sample * (decode[channel * 2 + 1] - decode[channel * 2])
                    })
                    .collect()
            }
        };
        Ok(values
            .into_iter()
            .enumerate()
            .map(|(i, value)| value.clamp(self.range[i * 2], self.range[i * 2 + 1]))
            .collect())
    }
}

fn dimensions(x: f64, y: f64) -> Result<[u32; 2], OfdError> {
    if !x.is_finite() || !y.is_finite() || x <= 0. || y <= 0. {
        return Err(invalid("raster dimensions"));
    }
    let width = (x * 8.).ceil().max(1.);
    let height = (y * 8.).ceil().max(1.);
    let scale = (MAX_PIXELS as f64 / (width * height)).sqrt().min(1.);
    let width = (width * scale).floor().max(1.);
    let height = (height * scale).floor().max(1.);
    if !width.is_finite()
        || !height.is_finite()
        || width > MAX_PIXELS as f64
        || height > MAX_PIXELS as f64
        || width * height > MAX_PIXELS as f64
    {
        return Err(limit("raster dimensions exceed four million pixels"));
    }
    Ok([width as u32, height as u32])
}

pub(super) fn import(
    output: &mut OfdDocument,
    page: &PdfPageContext<'_>,
    state: &GraphicsState,
    dict: &lopdf::Dictionary,
    visible: bool,
) -> Result<(), OfdError> {
    let domain = domain(page.pdf, dict, false)?;
    let matrix = if dict.has(b"Matrix") {
        pdf_function_numbers(page.pdf, dict, b"Matrix", 6)?
            .try_into()
            .map_err(|_| invalid("Matrix"))?
    } else {
        [1., 0., 0., 1., 0., 0.]
    };
    let space = parse_color_space(
        Some(page.pdf),
        Some(page.resources),
        dict.get(b"ColorSpace")
            .map_err(|_| invalid("missing ColorSpace"))?,
        0,
    )?;
    if matches!(space, PdfColorSpace::Pattern { .. }) {
        return Err(OfdError::Invalid(
            "PDF shading cannot use Pattern color".into(),
        ));
    }
    if space.nonpainting() {
        return Ok(());
    }
    if matches!(space, PdfColorSpace::Indexed { .. }) {
        return Err(invalid("Indexed color is not allowed"));
    }
    let function = pdf_resolved_value(
        Some(page.pdf),
        dict.get(b"Function")
            .map_err(|_| invalid("missing Function"))?,
        "shading Function",
    )?;
    let mut total = 0;
    let functions = if let Object::Array(functions) = function {
        if functions.len() != space.channels() {
            return Err(invalid("component Function count"));
        }
        functions
            .iter()
            .map(|function| Function::parse(page.pdf, function, 1, domain, &mut total))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![Function::parse(
            page.pdf,
            function,
            space.channels(),
            domain,
            &mut total,
        )?]
    };
    let mut transformed = state.clone();
    transformed.concat(matrix);
    transformed.concat([
        domain[1] - domain[0],
        0.,
        0.,
        domain[3] - domain[2],
        domain[0],
        domain[2],
    ]);
    let a = transformed.point(0., 0., page.box_);
    let b = transformed.point(1., 0., page.box_);
    let c = transformed.point(0., 1., page.box_);
    let d = transformed.point(1., 1., page.box_);
    if [a, b, c, d]
        .iter()
        .any(|&(x, y)| !x.is_finite() || !y.is_finite() || x.abs() > 1e9 || y.abs() > 1e9)
    {
        return Err(invalid("transformed coordinates"));
    }
    let determinant = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    if !visible || determinant == 0. {
        return Ok(());
    }
    let [width, height] = dimensions((b.0 - a.0).hypot(b.1 - a.1), (c.0 - a.0).hypot(c.1 - a.1))?;
    let pixels = width as usize * height as usize;
    let work: usize = functions.iter().map(Function::work_cost).sum();
    if work.checked_mul(pixels).is_none_or(|work| work > MAX_WORK) {
        return Err(limit("exceeds 100 million evaluation work units"));
    }
    let mut data = Vec::with_capacity(pixels * 3);
    for row in 0..height {
        let y = domain[3] - (f64::from(row) + 0.5) / f64::from(height) * (domain[3] - domain[2]);
        for column in 0..width {
            let x =
                domain[0] + (f64::from(column) + 0.5) / f64::from(width) * (domain[1] - domain[0]);
            let mut values = Vec::with_capacity(space.channels());
            for function in &functions {
                values.extend(function.evaluate([x, y])?);
            }
            for (i, value) in values.iter_mut().enumerate() {
                let bounds = space.component_bounds(i);
                *value = value.clamp(bounds[0], bounds[1]);
            }
            data.extend(space.color_with_intent(&values, state.rendering_intent)?);
        }
    }
    let image = lopdf::Stream::new(
        lopdf::dictionary! { "Type"=>"XObject", "Subtype"=>"Image", "Width"=>i64::from(width), "Height"=>i64::from(height), "ColorSpace"=>"DeviceRGB", "BitsPerComponent"=>8 },
        data,
    );
    let resources = Object::Dictionary(lopdf::Dictionary::new());
    import_image_stream(
        output,
        page.index,
        page.pdf,
        &resources,
        page.box_,
        &transformed,
        &image,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    fn sampled(bits: usize, size: [usize; 2], values: &[u32]) -> (PdfDocument, Object) {
        let mut bytes = vec![0; (values.len() * bits).div_ceil(8)];
        for (index, value) in values.iter().enumerate() {
            for shift in 0..bits {
                let position = index * bits + shift;
                bytes[position / 8] |=
                    (((value >> (bits - shift - 1)) & 1) as u8) << (7 - position % 8);
            }
        }
        let mut pdf = PdfDocument::new();
        let object = pdf.add_object(lopdf::Stream::new(lopdf::dictionary! {
            "FunctionType"=>0,"Domain"=>vec![0.into(),1.into(),0.into(),1.into()],
            "Range"=>vec![0.into(),1.into()],"Size"=>vec![(size[0] as i64).into(),(size[1] as i64).into()],
            "BitsPerSample"=>bits as i64,
        },bytes));
        (pdf, Object::Reference(object))
    }

    fn parse(pdf: &PdfDocument, object: &Object) -> Result<Function, OfdError> {
        Function::parse(pdf, object, 1, [0., 1., 0., 1.], &mut 0)
    }

    #[test]
    fn sampled_planes_decode_all_bit_depths_in_x_first_order() {
        for bits in [1, 2, 4, 8, 12, 16, 24, 32] {
            let max = ((1_u64 << bits) - 1) as u32;
            let (pdf, object) = sampled(bits, [2, 2], &[0, max, max, 0]);
            let function = parse(&pdf, &object).unwrap();
            for (input, expected) in [
                ([0., 0.], 0.),
                ([1., 0.], 1.),
                ([0., 1.], 1.),
                ([1., 1.], 0.),
                ([0.5, 0.5], 0.5),
                ([0.25, 0.75], 0.625),
            ] {
                assert!((function.evaluate(input).unwrap()[0] - expected).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn sampled_planes_apply_reversed_encode_decode_and_output_clipping() {
        let (mut pdf, object) = sampled(8, [2, 2], &[0, 255, 255, 0]);
        let id = object.as_reference().unwrap();
        pdf.get_object_mut(id)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set("Encode", vec![1.into(), 0.into(), 0.into(), 1.into()]);
        assert_eq!(
            parse(&pdf, &object).unwrap().evaluate([0., 0.]).unwrap(),
            [1.]
        );
        let dict = &mut pdf
            .get_object_mut(id)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict;
        dict.remove(b"Encode");
        dict.set("Range", vec![Object::Real(0.2), Object::Real(0.8)]);
        dict.set("Decode", vec![2.into(), (-1).into()]);
        let function = parse(&pdf, &object).unwrap();
        assert!((function.evaluate([0., 0.]).unwrap()[0] - 0.8).abs() < 1e-6);
        assert!((function.evaluate([1., 0.]).unwrap()[0] - 0.2).abs() < 1e-6);
        assert_eq!(function.evaluate([0.5, 0.5]).unwrap(), [0.5]);
    }

    #[test]
    fn cubic_planes_reproduce_interior_quadratics_and_three_point_edges() {
        let max = u32::MAX as f64;
        let values = (0..4)
            .flat_map(|y| {
                (0..4).map(move |x| (((x * x + y * y) as f64 / 18.) * max).round() as u32)
            })
            .collect::<Vec<_>>();
        let (mut pdf, object) = sampled(32, [4, 4], &values);
        pdf.get_object_mut(object.as_reference().unwrap())
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set("Order", 3);
        assert!(
            (parse(&pdf, &object).unwrap().evaluate([0.5, 0.5]).unwrap()[0] - 0.25).abs() < 1e-8
        );
        let (mut pdf, object) = sampled(8, [3, 1], &[0, 255, 0]);
        pdf.get_object_mut(object.as_reference().unwrap())
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set("Order", 3);
        assert_eq!(
            parse(&pdf, &object)
                .unwrap()
                .evaluate([0.25, 0.75])
                .unwrap(),
            [0.5625]
        );
        let (mut pdf, object) = sampled(8, [2, 1], &[0, 255]);
        pdf.get_object_mut(object.as_reference().unwrap())
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set("Order", 3);
        assert_eq!(
            parse(&pdf, &object)
                .unwrap()
                .evaluate([0.25, 0.75])
                .unwrap(),
            [0.25]
        );
    }

    #[test]
    fn three_sample_cubic_functions_agree_in_scalar_and_plane_importers() {
        let (mut pdf, object) = sampled(8, [3, 1], &[0, 255, 0]);
        let dict = &mut pdf
            .get_object_mut(object.as_reference().unwrap())
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict;
        dict.set("Domain", vec![0.into(), 1.into()]);
        dict.set("Size", vec![3.into()]);
        dict.set("Order", 3);
        let functions = crate::pdf_import::scalar_function::ScalarFunctions::parse(
            &pdf,
            std::slice::from_ref(&object),
        )
        .unwrap();
        assert_eq!(functions.evaluate(0.25).unwrap().values, [0.5625]);
        let segments = crate::pdf_import::pdf_shading_segments(
            &pdf,
            &object,
            &PdfColorSpace::Gray,
            moxcms::RenderingIntent::RelativeColorimetric,
            [0., 1.],
        )
        .unwrap();
        let midpoint = segments
            .iter()
            .find(|segment| segment.position == Some(0.25))
            .unwrap();
        assert_eq!(midpoint.color, [143; 3]);
    }

    #[test]
    fn sampled_planes_reject_malformed_metadata_and_truncated_data() {
        let (original, object) = sampled(8, [2, 2], &[0, 255, 255, 0]);
        let id = object.as_reference().unwrap();
        for (key, value) in [
            ("Domain", Object::Array(vec![0.into(), 1.into()])),
            (
                "Domain",
                Object::Array(vec![1.into(), 0.into(), 0.into(), 1.into()]),
            ),
            (
                "Domain",
                Object::Array(vec![0.into(), 1.into(), 0.into(), Object::Real(0.5)]),
            ),
            ("Range", Object::Array(vec![1.into(), 0.into()])),
            ("Size", Object::Array(vec![0.into(), 2.into()])),
            ("Size", Object::Array(vec![Object::Real(1.5), 2.into()])),
            ("BitsPerSample", Object::Integer(3)),
            ("Order", Object::Integer(2)),
            ("Encode", Object::Array(vec![0.into()])),
            ("Decode", Object::Array(vec![0.into()])),
            ("FunctionType", Object::Integer(2)),
        ] {
            let mut pdf = original.clone();
            pdf.get_object_mut(id)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .set(key, value);
            assert!(parse(&pdf, &object).is_err(), "{key}");
        }
        for key in ["Range", "Domain", "BitsPerSample"] {
            let mut pdf = original.clone();
            pdf.get_object_mut(id)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .remove(key.as_bytes());
            assert!(parse(&pdf, &object).is_err(), "missing {key}");
        }
        let mut pdf = original;
        pdf.get_object_mut(id)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_content(vec![0; 3]);
        assert!(parse(&pdf, &object).is_err());
    }

    #[test]
    fn sample_tables_dimensions_and_total_evaluation_work_are_bounded() {
        let (mut pdf, object) = sampled(8, [1001, 1001], &[]);
        assert!(matches!(parse(&pdf, &object), Err(OfdError::Limit(_))));
        pdf.get_object_mut(object.as_reference().unwrap())
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set("Size", vec![2.into(), 2.into()]);
        let mut total = MAX_SAMPLES;
        assert!(matches!(
            Function::parse(&pdf, &object, 1, [0., 1., 0., 1.], &mut total),
            Err(OfdError::Limit(_))
        ));
        for (x, y) in [(f64::NAN, 1.), (1., f64::INFINITY), (0., 1.), (1e9, 1e-9)] {
            assert!(dimensions(x, y).is_err());
        }
        let [width, height] = dimensions(2000., 2000.).unwrap();
        assert!(u64::from(width) * u64::from(height) <= MAX_PIXELS as u64);
        let mut pdf = PdfDocument::load_mem(include_bytes!(
            "../../tests/fixtures/plane-calculator-rgb.pdf"
        ))
        .unwrap();
        pdf.get_object_mut((5, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set(
                "Matrix",
                vec![
                    600.into(),
                    0.into(),
                    0.into(),
                    600.into(),
                    0.into(),
                    0.into(),
                ],
            );
        let function = pdf.get_object_mut((6, 0)).unwrap().as_stream_mut().unwrap();
        function.set_content(b"{ 2 copy pop pop 1 index 1 exch sub }".to_vec());
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        assert!(matches!(
            OfdDocument::from_pdf_bytes(&bytes),
            Err(OfdError::Limit(_))
        ));
    }

    #[test]
    fn calculator_planes_clip_outputs_and_count_branch_and_stack_work() {
        let calculator =
            Calculator::parse(b"{ 1 index .5 gt { 2 copy pop pop } { } ifelse add }").unwrap();
        assert!(calculator.work_cost() >= 256);
        assert_eq!(calculator.evaluate(&[0.75, 0.5], 1).unwrap().values, [1.25]);
        let mut pdf =
            PdfDocument::load_mem(include_bytes!("../../tests/fixtures/plane-calgray.pdf"))
                .unwrap();
        let function = parse(&pdf, pdf.get_object((6, 0)).unwrap()).unwrap();
        assert_eq!(function.evaluate([0.25, 0.75]).unwrap(), [0.5]);
        pdf.get_object_mut((6, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set("Range", vec![Object::Real(0.3), Object::Real(0.6)]);
        let function = parse(&pdf, pdf.get_object((6, 0)).unwrap()).unwrap();
        assert!((function.evaluate([0., 0.]).unwrap()[0] - 0.3).abs() < 1e-6);
        assert!((function.evaluate([1., 1.]).unwrap()[0] - 0.6).abs() < 1e-6);
        assert!(
            Calculator::parse(b"{ 1 index .5 gt { 2 copy pop pop } if add }")
                .unwrap()
                .work_cost()
                >= 256
        );
    }
}
