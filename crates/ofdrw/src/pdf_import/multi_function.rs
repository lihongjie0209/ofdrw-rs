//! Bounded multi-input sampled/calculator functions for composite color conversion.

use super::{
    pdf_byte_stream_data, pdf_function_integer, pdf_function_numbers, pdf_function_type,
    pdf_resolved_value, pdf_sample_cubic_coefficients, pdf_shading_function_range, read_pdf_sample,
    Object, OfdError, PdfDocument,
};
use crate::pdf_calculator::Calculator;

const MAX_SAMPLES: usize = 1_000_000;
const MAX_WORK: usize = 1_000_000;

fn invalid(reason: &str) -> OfdError {
    OfdError::Invalid(format!("invalid PDF multi-input Function: {reason}"))
}

pub(super) struct Function {
    domain: Vec<f64>,
    range: Vec<f64>,
    kind: Kind,
    work: usize,
}

enum Kind {
    Calculator(Calculator),
    Sampled {
        size: Vec<usize>,
        encode: Vec<f64>,
        decode: Vec<f64>,
        samples: Vec<f64>,
        cubic: bool,
    },
}

impl Function {
    pub(super) fn parse(
        pdf: &PdfDocument,
        object: &Object,
        inputs: usize,
        outputs: usize,
    ) -> Result<Self, OfdError> {
        if !(1..=32).contains(&inputs) || !(1..=32).contains(&outputs) {
            return Err(invalid("component count"));
        }
        let object = pdf_resolved_value(Some(pdf), object, "multi-input Function")?;
        let stream = object.as_stream().map_err(|_| invalid("expected stream"))?;
        let dict = &stream.dict;
        let domain = pdf_function_numbers(pdf, dict, b"Domain", inputs * 2)?;
        if domain.chunks_exact(2).any(|pair| pair[0] >= pair[1]) {
            return Err(invalid("Domain bounds"));
        }
        let range = pdf_shading_function_range(pdf, dict, outputs)?
            .ok_or_else(|| invalid("missing Range"))?;
        let (kind, work) = match pdf_function_type(pdf, dict)? {
            4 => {
                let calculator = Calculator::parse(&pdf_byte_stream_data(
                    stream,
                    Some(pdf),
                    "multi-input calculator Function",
                )?)?;
                let work = calculator.work_cost();
                (Kind::Calculator(calculator), work)
            }
            0 => {
                let values = pdf_function_numbers(pdf, dict, b"Size", inputs)?;
                if values
                    .iter()
                    .any(|value| *value < 1. || value.fract() != 0. || *value > MAX_SAMPLES as f64)
                {
                    return Err(invalid("Size"));
                }
                let size: Vec<_> = values.into_iter().map(|value| value as usize).collect();
                let count = size
                    .iter()
                    .try_fold(outputs, |count, size| count.checked_mul(*size))
                    .filter(|count| *count <= MAX_SAMPLES)
                    .ok_or_else(|| {
                        OfdError::Limit("PDF Function exceeds one million samples".into())
                    })?;
                let bits = pdf_function_integer(pdf, dict, b"BitsPerSample", None)?;
                if !matches!(bits, 1 | 2 | 4 | 8 | 12 | 16 | 24 | 32) {
                    return Err(invalid("BitsPerSample"));
                }
                let cubic = match pdf_function_integer(pdf, dict, b"Order", Some(1))? {
                    1 => false,
                    3 => true,
                    _ => return Err(invalid("Order")),
                };
                let work = size
                    .iter()
                    .try_fold(outputs, |work, size| {
                        work.checked_mul(if *size == 1 {
                            1
                        } else if cubic && *size >= 3 {
                            4
                        } else {
                            2
                        })
                    })
                    .filter(|work| *work <= MAX_WORK)
                    .ok_or_else(|| {
                        OfdError::Limit(
                            "PDF Function interpolation exceeds one million work units".into(),
                        )
                    })?;
                let encode = if dict.has(b"Encode") {
                    pdf_function_numbers(pdf, dict, b"Encode", inputs * 2)?
                } else {
                    size.iter()
                        .flat_map(|size| [0., (*size - 1) as f64])
                        .collect()
                };
                let decode = if dict.has(b"Decode") {
                    pdf_function_numbers(pdf, dict, b"Decode", outputs * 2)?
                } else {
                    range.clone()
                };
                let data = pdf_byte_stream_data(stream, Some(pdf), "multi-input sampled Function")?;
                if data.len() < (count * bits as usize).div_ceil(8) {
                    return Err(invalid("truncated samples"));
                }
                let maximum = ((1_u64 << bits) - 1) as f64;
                let samples = (0..count)
                    .map(|i| f64::from(read_pdf_sample(&data, i, bits as usize)) / maximum)
                    .collect();
                (
                    Kind::Sampled {
                        size,
                        encode,
                        decode,
                        samples,
                        cubic,
                    },
                    work,
                )
            }
            _ => return Err(invalid("multiple inputs require FunctionType 0 or 4")),
        };
        let work = work.max(inputs + outputs);
        Ok(Self {
            domain,
            range,
            kind,
            work,
        })
    }

    pub(super) fn work_cost(&self) -> usize {
        self.work
    }

    pub(super) fn evaluate(&self, input: &[f64]) -> Result<Vec<f64>, OfdError> {
        if input.len() * 2 != self.domain.len() || input.iter().any(|value| !value.is_finite()) {
            return Err(invalid("input dimensions or nonfinite input"));
        }
        let input: Vec<_> = input
            .iter()
            .enumerate()
            .map(|(i, value)| value.clamp(self.domain[2 * i], self.domain[2 * i + 1]))
            .collect();
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
                // Axis zero varies fastest in the source sample table.
                let mut entries = vec![(0_usize, 1_f64)];
                let mut stride = outputs;
                for (axis, count) in size.iter().copied().enumerate() {
                    let i = axis * 2;
                    let t = (input[axis] - self.domain[i]) / (self.domain[i + 1] - self.domain[i]);
                    let at =
                        (encode[i] * (1. - t) + encode[i + 1] * t).clamp(0., (count - 1) as f64);
                    let first = at.floor() as usize;
                    let fraction = at - first as f64;
                    let weights = if count == 1 {
                        vec![(0, 1.)]
                    } else if *cubic && count >= 3 {
                        let indices = [
                            first.saturating_sub(1),
                            first,
                            (first + 1).min(count - 1),
                            (first + 2).min(count - 1),
                        ];
                        (0..4)
                            .map(|j| {
                                let coefficients =
                                    pdf_sample_cubic_coefficients(std::array::from_fn(|k| {
                                        f64::from(j == k)
                                    }));
                                let weight = ((coefficients[3] * fraction + coefficients[2])
                                    * fraction
                                    + coefficients[1])
                                    * fraction
                                    + coefficients[0];
                                (indices[j], weight)
                            })
                            .collect()
                    } else {
                        vec![
                            (first, 1. - fraction),
                            ((first + 1).min(count - 1), fraction),
                        ]
                    };
                    entries = entries
                        .into_iter()
                        .flat_map(|(offset, weight)| {
                            weights.iter().map(move |(index, factor)| {
                                (offset + index * stride, weight * factor)
                            })
                        })
                        .collect();
                    stride *= count;
                }
                (0..outputs)
                    .map(|channel| {
                        let sample: f64 = entries
                            .iter()
                            .map(|(offset, weight)| samples[offset + channel] * weight)
                            .sum();
                        decode[channel * 2]
                            + sample * (decode[channel * 2 + 1] - decode[channel * 2])
                    })
                    .collect()
            }
        };
        if values.iter().any(|value| !value.is_finite()) {
            return Err(invalid("nonfinite output"));
        }
        Ok(values
            .into_iter()
            .enumerate()
            .map(|(i, value)| value.clamp(self.range[i * 2], self.range[i * 2 + 1]))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    fn sampled(size: &[usize], bits: usize, samples: Vec<u8>) -> Object {
        Object::Stream(Stream::new(
            dictionary! {
                "FunctionType"=>0, "Domain"=>size.iter().flat_map(|_| [0.into(), 1.into()]).collect::<Vec<Object>>(),
                "Range"=>vec![0.into(),1.into()], "Size"=>size.iter().map(|n| Object::Integer(*n as i64)).collect::<Vec<_>>(),
                "BitsPerSample"=>bits as i64
            },
            samples,
        ))
    }

    #[test]
    fn three_input_samples_vary_first_axis_fastest() {
        let pdf = PdfDocument::new();
        let function = Function::parse(
            &pdf,
            &sampled(&[2, 2, 2], 8, (0..8).map(|n| n * 32).collect()),
            3,
            1,
        )
        .unwrap();
        for (input, expected) in [
            ([0., 0., 0.], 0.),
            ([1., 0., 0.], 32.),
            ([0., 1., 0.], 64.),
            ([0., 0., 1.], 128.),
            ([0.25, 0.5, 0.75], 136.),
        ] {
            assert!((function.evaluate(&input).unwrap()[0] - expected / 255.).abs() < 1e-12);
        }
        assert!(function.evaluate(&[0., 0.]).is_err());
        assert!(function.evaluate(&[0., f64::NAN, 0.]).is_err());
    }

    #[test]
    fn sampled_functions_accept_every_standard_bit_depth() {
        let pdf = PdfDocument::new();
        for bits in [1, 2, 4, 8, 12, 16, 24, 32] {
            let mut data = vec![0; (bits * 2_usize).div_ceil(8)];
            for bit in bits..bits * 2 {
                data[bit / 8] |= 1 << (7 - bit % 8);
            }
            let function = Function::parse(&pdf, &sampled(&[2, 1], bits, data), 2, 1).unwrap();
            assert!(
                (function.evaluate(&[0.25, 0.75]).unwrap()[0] - 0.25).abs() < 1e-12,
                "{bits}"
            );
        }
    }

    #[test]
    fn singleton_axes_do_not_expand_interpolation_work_exponentially() {
        let pdf = PdfDocument::new();
        let function = Function::parse(&pdf, &sampled(&[1; 32], 8, vec![128]), 32, 1).unwrap();
        assert_eq!(function.work_cost(), 33);
        assert_eq!(function.evaluate(&[0.5; 32]).unwrap(), [128. / 255.]);
    }

    #[test]
    fn multidimensional_cubic_uses_sample_coefficients_and_encode_decode_ranges() {
        let pdf = PdfDocument::new();
        let mut object = sampled(&[3, 1], 8, vec![0, 255, 0]);
        let dict = &mut object.as_stream_mut().unwrap().dict;
        dict.set("Order", 3);
        dict.set("Encode", vec![2.into(), 0.into(), 0.into(), 0.into()]);
        let function = Function::parse(&pdf, &object, 2, 1).unwrap();
        assert!((function.evaluate(&[0.25, 0.8]).unwrap()[0] - 0.5625).abs() < 1e-12);
        assert_eq!(function.evaluate(&[-1., 2.]).unwrap(), [0.]);
    }

    #[test]
    fn calculators_clip_input_domain_and_output_range() {
        let pdf = PdfDocument::new();
        let object = Object::Stream(Stream::new(
            dictionary! {"FunctionType"=>4,"Domain"=>vec![0.into(),1.into(),0.into(),1.into(),0.into(),1.into()],"Range"=>vec![0.into(),2.into()]},
            b"{ add add }".to_vec(),
        ));
        let function = Function::parse(&pdf, &object, 3, 1).unwrap();
        assert_eq!(function.evaluate(&[0.25, 0.5, 0.75]).unwrap(), [1.5]);
        assert_eq!(function.evaluate(&[-10., 10., 10.]).unwrap(), [2.]);
    }

    #[test]
    fn malformed_tables_and_excessive_dimensions_are_rejected() {
        let pdf = PdfDocument::new();
        for object in [
            sampled(&[2, 2], 8, vec![0]),
            sampled(&[0, 2], 8, vec![0]),
            sampled(&[1001, 1000], 8, vec![]),
            sampled(&[2, 2], 3, vec![0; 4]),
        ] {
            assert!(Function::parse(&pdf, &object, 2, 1).is_err());
        }
        assert!(Function::parse(&pdf, &sampled(&[1], 8, vec![0]), 0, 1).is_err());
        assert!(Function::parse(&pdf, &sampled(&[1], 8, vec![0]), 33, 1).is_err());
        let mut object = sampled(&[3; 12], 8, vec![0; 531441]);
        object.as_stream_mut().unwrap().dict.set("Order", 3);
        assert!(matches!(
            Function::parse(&pdf, &object, 12, 1),
            Err(OfdError::Limit(_))
        ));
    }
}
