//! One-input, one-output PDF functions for component-wise shading arrays.

use super::{
    pdf_byte_stream_data, pdf_function_array, pdf_function_domain, pdf_function_exponent,
    pdf_function_integer, pdf_function_numbers, pdf_function_type, pdf_sample_cubic_coefficients,
    pdf_sample_cubic_extrema, pdf_shading_function_range, pdf_stitching_bounds, read_pdf_sample,
    Object, OfdError, PdfDocument,
};
use crate::pdf_calculator::Calculator;

pub(super) struct ScalarFunctions {
    functions: Vec<Function>,
}

pub(super) struct ScalarValues {
    pub values: Vec<f64>,
    pub regions: Vec<(usize, u64)>,
}

struct Function {
    id: usize,
    domain: [f64; 2],
    range: Option<[f64; 2]>,
    kind: Kind,
}

enum Kind {
    Exponential {
        c0: f64,
        c1: f64,
        exponent: f64,
    },
    Sampled(Sampled),
    Stitching {
        bounds: Vec<f64>,
        encode: Vec<f64>,
        functions: Vec<Function>,
    },
    Calculator(Calculator),
}

struct Sampled {
    samples: Vec<f64>,
    cubic: bool,
    encode: [f64; 2],
    decode: [f64; 2],
}

struct Parser<'a> {
    pdf: &'a PdfDocument,
    nodes: usize,
    samples: usize,
    code_bytes: usize,
}

fn invalid(message: &str) -> OfdError {
    OfdError::Invalid(format!("invalid PDF component Function: {message}"))
}

fn limit(message: &str) -> OfdError {
    OfdError::Limit(format!("PDF component Function array {message}"))
}

fn sorted_positions(positions: &mut Vec<f64>) -> Result<(), OfdError> {
    positions.sort_by(f64::total_cmp);
    positions.dedup();
    if positions.len() > 10_000 {
        return Err(limit("has too many color stops"));
    }
    Ok(())
}

fn add_position(positions: &mut Vec<f64>, position: f64) -> Result<(), OfdError> {
    if position > 0.0 && position < 1.0 {
        positions.push(position);
        if positions.len() >= 20_000 {
            sorted_positions(positions)?;
        }
    }
    Ok(())
}

impl ScalarFunctions {
    pub fn parse(pdf: &PdfDocument, objects: &[Object]) -> Result<Self, OfdError> {
        let mut parser = Parser {
            pdf,
            nodes: 0,
            samples: 0,
            code_bytes: 0,
        };
        let functions = objects
            .iter()
            .map(|object| parser.function(object, 0))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { functions })
    }

    pub fn evaluate(&self, input: f64) -> Result<ScalarValues, OfdError> {
        let mut regions = Vec::new();
        let values = self
            .functions
            .iter()
            .map(|function| function.evaluate(input, &mut regions))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ScalarValues { values, regions })
    }

    pub fn positions(&self, input: [f64; 2]) -> Result<Vec<f64>, OfdError> {
        let mut positions = vec![0.0, 1.0];
        for function in &self.functions {
            function.positions(input, &mut positions)?;
            sorted_positions(&mut positions)?;
        }
        Ok(positions)
    }
}

impl Parser<'_> {
    fn function(&mut self, object: &Object, depth: usize) -> Result<Function, OfdError> {
        if depth > 16 {
            return Err(limit("nesting exceeds 16"));
        }
        if self.nodes == 4096 {
            return Err(limit("has more than 4096 function nodes"));
        }
        let id = self.nodes;
        self.nodes += 1;
        let (_, object) = self
            .pdf
            .dereference(object)
            .map_err(|error| invalid(&format!("broken reference: {error}")))?;
        let dictionary = match object {
            Object::Dictionary(dictionary) => dictionary,
            Object::Stream(stream) => &stream.dict,
            _ => return Err(invalid("expected a dictionary or stream")),
        };
        let function_type = pdf_function_type(self.pdf, dictionary)?;
        if matches!(function_type, 0 | 4) && !dictionary.has(b"Domain") {
            return Err(invalid("missing Domain"));
        }
        let domain = pdf_function_domain(self.pdf, dictionary)?;
        let range = pdf_shading_function_range(self.pdf, dictionary, 1)?
            .map(|values| [values[0], values[1]]);
        let kind = match function_type {
            0 => {
                let stream = object
                    .as_stream()
                    .map_err(|_| invalid("sampled function must be a stream"))?;
                let range = range.ok_or_else(|| invalid("sampled function has no Range"))?;
                let size = pdf_function_numbers(self.pdf, dictionary, b"Size", 1)?[0];
                if size < 1.0 || size.fract() != 0.0 {
                    return Err(invalid("invalid sample Size"));
                }
                if size > 10_000.0 {
                    return Err(limit("has too many samples in one function"));
                }
                let size = size as usize;
                self.samples += size;
                if self.samples > 100_000 {
                    return Err(limit("has more than 100000 samples"));
                }
                let bits = pdf_function_integer(self.pdf, dictionary, b"BitsPerSample", None)?;
                if !matches!(bits, 1 | 2 | 4 | 8 | 12 | 16 | 24 | 32) {
                    return Err(invalid("invalid BitsPerSample"));
                }
                let cubic = match pdf_function_integer(self.pdf, dictionary, b"Order", Some(1))? {
                    1 => false,
                    3 => size >= 3,
                    _ => return Err(invalid("invalid interpolation Order")),
                };
                let encode = if dictionary.has(b"Encode") {
                    let values = pdf_function_numbers(self.pdf, dictionary, b"Encode", 2)?;
                    [values[0], values[1]]
                } else {
                    [0.0, (size - 1) as f64]
                };
                let decode = if dictionary.has(b"Decode") {
                    let values = pdf_function_numbers(self.pdf, dictionary, b"Decode", 2)?;
                    [values[0], values[1]]
                } else {
                    range
                };
                let bytes = pdf_byte_stream_data(stream, Some(self.pdf), "sampled Function")?;
                if bytes.len() < (size * bits as usize).div_ceil(8) {
                    return Err(invalid("truncated sample data"));
                }
                let maximum = ((1u64 << bits) - 1) as f64;
                let samples = (0..size)
                    .map(|index| f64::from(read_pdf_sample(&bytes, index, bits as usize)) / maximum)
                    .collect();
                Kind::Sampled(Sampled {
                    samples,
                    cubic,
                    encode,
                    decode,
                })
            }
            2 => {
                let endpoint = |key: &[u8], default| {
                    if dictionary.has(key) {
                        pdf_function_numbers(self.pdf, dictionary, key, 1).map(|values| values[0])
                    } else {
                        Ok(default)
                    }
                };
                Kind::Exponential {
                    c0: endpoint(b"C0", 0.0)?,
                    c1: endpoint(b"C1", 1.0)?,
                    exponent: pdf_function_exponent(self.pdf, dictionary, domain)?,
                }
            }
            3 => {
                let objects = pdf_function_array(self.pdf, dictionary, b"Functions")?;
                let bounds = pdf_stitching_bounds(self.pdf, dictionary, objects.len(), domain)?;
                let encode =
                    pdf_function_numbers(self.pdf, dictionary, b"Encode", objects.len() * 2)?;
                let functions = objects
                    .iter()
                    .map(|object| self.function(object, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                Kind::Stitching {
                    bounds,
                    encode,
                    functions,
                }
            }
            4 => {
                if range.is_none() {
                    return Err(invalid("calculator function has no Range"));
                }
                let stream = object
                    .as_stream()
                    .map_err(|_| invalid("calculator function must be a stream"))?;
                let code = pdf_byte_stream_data(stream, Some(self.pdf), "calculator Function")?;
                self.code_bytes += code.len();
                if self.code_bytes > 1_048_576 {
                    return Err(limit("has more than 1 MiB of calculator code"));
                }
                Kind::Calculator(Calculator::parse(&code)?)
            }
            _ => return Err(invalid("unsupported FunctionType")),
        };
        Ok(Function {
            id,
            domain,
            range,
            kind,
        })
    }
}

impl Sampled {
    fn coefficients(&self, index: usize) -> [f64; 4] {
        let last = self.samples.len() - 1;
        pdf_sample_cubic_coefficients([
            self.samples[index.saturating_sub(1)],
            self.samples[index],
            self.samples[(index + 1).min(last)],
            self.samples[(index + 2).min(last)],
        ])
    }

    fn evaluate(&self, input: f64, domain: [f64; 2]) -> f64 {
        let last = self.samples.len() - 1;
        let x = (self.encode[0]
            + (input - domain[0]) / (domain[1] - domain[0]) * (self.encode[1] - self.encode[0]))
            .clamp(0.0, last as f64);
        let first = x.floor() as usize;
        let fraction = x - first as f64;
        let value = if self.cubic {
            let [a, b, c, d] = self.coefficients(first);
            ((d * fraction + c) * fraction + b) * fraction + a
        } else {
            self.samples[first] * (1.0 - fraction) + self.samples[(first + 1).min(last)] * fraction
        };
        self.decode[0] + value * (self.decode[1] - self.decode[0])
    }
}

impl Function {
    fn evaluate(&self, input: f64, regions: &mut Vec<(usize, u64)>) -> Result<f64, OfdError> {
        let input = input.clamp(self.domain[0], self.domain[1]);
        let value = match &self.kind {
            Kind::Exponential { c0, c1, exponent } => c0 + input.powf(*exponent) * (c1 - c0),
            Kind::Sampled(sampled) => sampled.evaluate(input, self.domain),
            Kind::Stitching {
                bounds,
                encode,
                functions,
            } => {
                let index = bounds.partition_point(|bound| *bound <= input);
                regions.push((self.id * 4097, index as u64));
                let first = if index == 0 {
                    self.domain[0]
                } else {
                    bounds[index - 1]
                };
                let last = if index == bounds.len() {
                    self.domain[1]
                } else {
                    bounds[index]
                };
                let encoded = encode[2 * index]
                    + (input - first) / (last - first)
                        * (encode[2 * index + 1] - encode[2 * index]);
                functions[index].evaluate(encoded, regions)?
            }
            Kind::Calculator(program) => {
                let result = program.evaluate(&[input], 1)?;
                // Each component and nested calculator has its own trace namespace.
                regions.extend(
                    result
                        .regions
                        .into_iter()
                        .map(|(id, region)| (self.id * 4097 + id + 1, region)),
                );
                result.values[0]
            }
        };
        let value = self
            .range
            .map_or(value, |range| value.clamp(range[0], range[1]));
        if value.is_nan() {
            return Err(invalid("undefined function result"));
        }
        Ok(value)
    }

    fn positions(&self, input: [f64; 2], positions: &mut Vec<f64>) -> Result<(), OfdError> {
        if input[0] == input[1] {
            return Ok(());
        }
        let position = |value| (value - input[0]) / (input[1] - input[0]);
        for bound in self.domain {
            add_position(positions, position(bound))?;
        }
        match &self.kind {
            Kind::Sampled(sampled) if sampled.encode[0] != sampled.encode[1] => {
                let mut knots = (0..sampled.samples.len())
                    .map(|i| i as f64)
                    .collect::<Vec<_>>();
                if sampled.cubic {
                    for index in 0..sampled.samples.len() - 1 {
                        knots.extend(
                            pdf_sample_cubic_extrema(sampled.coefficients(index))
                                .into_iter()
                                .map(|fraction| index as f64 + fraction),
                        );
                    }
                }
                for knot in knots {
                    let value = self.domain[0]
                        + (knot - sampled.encode[0]) / (sampled.encode[1] - sampled.encode[0])
                            * (self.domain[1] - self.domain[0]);
                    if value >= self.domain[0] && value <= self.domain[1] {
                        add_position(positions, position(value))?;
                    }
                }
            }
            Kind::Exponential { c0, c1, exponent } => {
                if self.domain[0] < 0.0 && self.domain[1] > 0.0 {
                    add_position(positions, position(0.0))?;
                }
                if *exponent == 1.0 && c0 != c1 {
                    if let Some(range) = self.range {
                        for bound in range {
                            let value = (bound - c0) / (c1 - c0);
                            if value >= self.domain[0] && value <= self.domain[1] {
                                add_position(positions, position(value))?;
                            }
                        }
                    }
                }
            }
            Kind::Stitching {
                bounds,
                encode,
                functions,
            } => {
                for bound in bounds {
                    add_position(positions, position(*bound))?;
                }
                for (index, function) in functions.iter().enumerate() {
                    let first = if index == 0 {
                        self.domain[0]
                    } else {
                        bounds[index - 1]
                    };
                    let last = if index == bounds.len() {
                        self.domain[1]
                    } else {
                        bounds[index]
                    };
                    let low = first.max(input[0].min(input[1]));
                    let high = last.min(input[0].max(input[1]));
                    if high <= low {
                        continue;
                    }
                    let encoded = |value| {
                        encode[2 * index]
                            + (value - first) / (last - first)
                                * (encode[2 * index + 1] - encode[2 * index])
                    };
                    let mut nested = Vec::new();
                    function.positions([encoded(low), encoded(high)], &mut nested)?;
                    for local in nested {
                        add_position(positions, position(low + local * (high - low)))?;
                    }
                }
            }
            Kind::Calculator(_) => {
                for index in 1..64 {
                    add_position(positions, f64::from(index) / 64.0)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
