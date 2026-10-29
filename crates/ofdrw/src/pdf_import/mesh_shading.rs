//! Byte-aligned PDF triangle meshes, retaining source interpolation before RGB conversion.

use super::{
    parse_color_space, pdf_byte_stream_data, pdf_function_numbers, pdf_resolved_value,
    pdf_shading_segments, GraphicsState, Object, OfdError, PdfColorSpace, PdfDocument,
    PdfPageContext,
};
use crate::{GouraudControlPoint, GouraudFillStyle};

const MAX_POINTS: usize = 100_000;

fn invalid() -> OfdError {
    OfdError::Invalid("invalid PDF Gouraud mesh".into())
}

fn integer(pdf: &PdfDocument, dict: &lopdf::Dictionary, key: &[u8]) -> Result<usize, OfdError> {
    let value = pdf_resolved_value(
        Some(pdf),
        dict.get(key).map_err(|_| invalid())?,
        "mesh integer",
    )?;
    usize::try_from(value.as_i64().map_err(|_| invalid())?).map_err(|_| invalid())
}

struct Bits<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Bits<'_> {
    fn read(&mut self, count: usize) -> Result<u32, OfdError> {
        let mut value = 0;
        for _ in 0..count {
            let byte = self.bytes.get(self.position / 8).ok_or_else(invalid)?;
            value = (value << 1) | u32::from((byte >> (7 - self.position % 8)) & 1);
            self.position += 1;
        }
        Ok(value)
    }
}

#[derive(Clone)]
struct Vertex {
    position: [f64; 2],
    components: Vec<f64>,
    flag: u8,
}

fn mixed(vertices: &[Vertex; 3], weights: [f64; 3]) -> Vertex {
    Vertex {
        position: std::array::from_fn(|axis| {
            (0..3)
                .map(|i| vertices[i].position[axis] * weights[i])
                .sum()
        }),
        components: (0..vertices[0].components.len())
            .map(|channel| {
                (0..3)
                    .map(|i| vertices[i].components[channel] * weights[i])
                    .sum()
            })
            .collect(),
        flag: 0,
    }
}

fn triangles(points: &[Vertex], columns: Option<usize>) -> Result<Vec<[usize; 3]>, OfdError> {
    let mut triangles = Vec::new();
    if let Some(columns) = columns {
        if columns < 2 || columns > points.len() / 2 || !points.len().is_multiple_of(columns) {
            return Err(invalid());
        }
        for row in 0..points.len() / columns - 1 {
            for column in 0..columns - 1 {
                let a = row * columns + column;
                triangles.push([a, a + 1, a + columns]);
                triangles.push([a + 1, a + columns, a + columns + 1]);
            }
        }
    } else {
        let mut index = 0;
        let mut previous = None;
        while index < points.len() {
            let triangle = match points[index].flag {
                0 if index + 2 < points.len() => {
                    index += 3;
                    [index - 3, index - 2, index - 1]
                }
                1 | 2 => {
                    let [a, b, c] = previous.ok_or_else(invalid)?;
                    let triangle = if points[index].flag == 1 {
                        [b, c, index]
                    } else {
                        [a, c, index]
                    };
                    index += 1;
                    triangle
                }
                _ => return Err(invalid()),
            };
            previous = Some(triangle);
            triangles.push(triangle);
        }
    }
    if triangles.is_empty() {
        return Err(invalid());
    }
    Ok(triangles)
}

fn subdivide<F>(
    vertices: [Vertex; 3],
    depth: usize,
    evaluate: &F,
    points: &mut Vec<GouraudControlPoint>,
) -> Result<(), OfdError>
where
    F: Fn(&[f64]) -> Result<[u8; 3], OfdError>,
{
    if points.len() + 3 > MAX_POINTS {
        return Err(OfdError::Limit(
            "PDF Gouraud conversion exceeds 100000 points".into(),
        ));
    }
    let colors = vertices
        .iter()
        .map(|v| evaluate(&v.components))
        .collect::<Result<Vec<_>, _>>()?;
    let weights = [[0.5, 0.5, 0.], [0., 0.5, 0.5], [0.5, 0., 0.5], [1. / 3.; 3]];
    let mut accurate = true;
    for weights in weights {
        let sample = mixed(&vertices, weights);
        let color = evaluate(&sample.components)?;
        for channel in 0..3 {
            let expected: f64 = (0..3)
                .map(|i| f64::from(colors[i][channel]) * weights[i])
                .sum();
            accurate &= (f64::from(color[channel]) - expected).abs() <= 2.;
        }
    }
    if accurate {
        for (vertex, color) in vertices.into_iter().zip(colors) {
            points.push(GouraudControlPoint {
                x: vertex.position[0],
                y: vertex.position[1],
                color,
                alpha: None,
                edge_flag: 0,
            });
        }
        return Ok(());
    }
    if depth == 10 {
        return Err(OfdError::Limit(
            "PDF Gouraud interpolation cannot meet color tolerance".into(),
        ));
    }
    let ab = mixed(&vertices, weights[0]);
    let bc = mixed(&vertices, weights[1]);
    let ca = mixed(&vertices, weights[2]);
    let [a, b, c] = vertices;
    for triangle in [
        [a, ab.clone(), ca.clone()],
        [ab.clone(), b, bc.clone()],
        [ca.clone(), bc.clone(), c],
        [ab, bc, ca],
    ] {
        subdivide(triangle, depth + 1, evaluate, points)?;
    }
    Ok(())
}

pub(super) fn parse(
    source: &Object,
    page: &PdfPageContext<'_>,
    state: &GraphicsState,
    kind: i64,
) -> Result<Option<GouraudFillStyle>, OfdError> {
    let stream = source.as_stream().map_err(|_| invalid())?;
    let dict = &stream.dict;
    let coordinate_bits = integer(page.pdf, dict, b"BitsPerCoordinate")?;
    let component_bits = integer(page.pdf, dict, b"BitsPerComponent")?;
    if ![1, 2, 4, 8, 12, 16, 24, 32].contains(&coordinate_bits)
        || ![1, 2, 4, 8, 12, 16].contains(&component_bits)
    {
        return Err(invalid());
    }
    let flag_bits = if kind == 4 {
        let bits = integer(page.pdf, dict, b"BitsPerFlag")?;
        if ![2, 4, 8].contains(&bits) {
            return Err(invalid());
        }
        bits
    } else {
        0
    };
    let color_space = parse_color_space(
        Some(page.pdf),
        Some(page.resources),
        dict.get(b"ColorSpace").map_err(|_| invalid())?,
        0,
    )?;
    if matches!(color_space, PdfColorSpace::Indexed { .. }) {
        return Err(invalid());
    }
    let function = dict.get(b"Function").ok();
    let channels = if function.is_some() {
        1
    } else {
        color_space.channels()
    };
    let decode = pdf_function_numbers(page.pdf, dict, b"Decode", 4 + 2 * channels)?;
    let data = pdf_byte_stream_data(stream, Some(page.pdf), "Gouraud mesh")?;
    let record_bytes = (flag_bits + coordinate_bits * 2 + component_bits * channels).div_ceil(8);
    if !data.len().is_multiple_of(record_bytes) {
        return Err(invalid());
    }
    if data.len() / record_bytes > MAX_POINTS {
        return Err(OfdError::Limit(
            "PDF Gouraud mesh exceeds 100000 vertices".into(),
        ));
    }
    let mut vertices = Vec::with_capacity(data.len() / record_bytes);
    for record in data.chunks_exact(record_bytes) {
        let mut bits = Bits {
            bytes: record,
            position: 0,
        };
        let flag = bits.read(flag_bits)? as u8 & 3;
        let mut values = Vec::with_capacity(2 + channels);
        for component in 0..2 + channels {
            let width = if component < 2 {
                coordinate_bits
            } else {
                component_bits
            };
            let sample = f64::from(bits.read(width)?);
            let fraction = sample / ((1_u64 << width) - 1) as f64;
            let value =
                decode[component * 2] * (1. - fraction) + decode[component * 2 + 1] * fraction;
            values.push(value);
        }
        let (x, y) = state.point(values[0], values[1], page.box_);
        if !x.is_finite() || !y.is_finite() || x.abs() > 1e9 || y.abs() > 1e9 {
            return Err(invalid());
        }
        vertices.push(Vertex {
            position: [x, y],
            components: values[2..].to_vec(),
            flag,
        });
    }
    let columns = (kind == 5)
        .then(|| integer(page.pdf, dict, b"VerticesPerRow"))
        .transpose()?;
    let mut triangles = triangles(&vertices, columns)?;
    let original_count = triangles.len();
    triangles.retain(|indices| {
        let [a, b, c] = indices.map(|index| vertices[index].position);
        ((b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])).abs() >= 1e-12
    });
    if triangles.is_empty() {
        return Ok(None);
    }
    let original_topology = original_count == triangles.len();
    let linear = function.is_none()
        && matches!(color_space, PdfColorSpace::Rgb | PdfColorSpace::Gray)
        && (0..channels).all(|channel| {
            decode[4 + channel * 2..6 + channel * 2]
                .iter()
                .all(|v| (0.0..=1.0).contains(v))
        });
    let mut points = Vec::new();
    if linear && original_topology {
        // Preserve lattice topology and edge reuse when source interpolation is RGB-linear.
        let mut index = 0;
        while index < vertices.len() {
            let count = if columns.is_some() || vertices[index].flag != 0 {
                1
            } else {
                3
            };
            for (offset, vertex) in vertices[index..index + count].iter().enumerate() {
                points.push(GouraudControlPoint {
                    x: vertex.position[0],
                    y: vertex.position[1],
                    color: color_space.color(&vertex.components)?,
                    alpha: None,
                    edge_flag: if columns.is_some() || offset > 0 {
                        0
                    } else {
                        vertex.flag
                    },
                });
            }
            index += count;
        }
    } else {
        let stops = function
            .map(|function| {
                let a = decode[4].min(decode[5]);
                let b = decode[4].max(decode[5]);
                pdf_shading_segments(
                    page.pdf,
                    function,
                    &color_space,
                    state.rendering_intent,
                    [a, if b == a { a + 1.0 } else { b }],
                )
            })
            .transpose()?;
        let remaining = std::cell::Cell::new(1_000_000usize);
        let evaluate = |values: &[f64]| -> Result<[u8; 3], OfdError> {
            remaining.set(remaining.get().checked_sub(1).ok_or_else(|| {
                OfdError::Limit("PDF Gouraud color evaluation budget exceeded".into())
            })?);
            let Some(stops) = &stops else {
                let values = values
                    .iter()
                    .enumerate()
                    .map(|(channel, value)| {
                        let bounds = color_space.component_bounds(channel);
                        value.clamp(bounds[0], bounds[1])
                    })
                    .collect::<Vec<_>>();
                return color_space.color_with_intent(&values, state.rendering_intent);
            };
            let a = decode[4].min(decode[5]);
            let b = decode[4].max(decode[5]);
            let b = if b == a { a + 1.0 } else { b };
            let position = ((values[0] - a) / (b - a)).clamp(0., 1.);
            let index = stops.partition_point(|stop| stop.position.unwrap_or(0.) <= position);
            if index == 0 {
                return Ok(stops[0].color);
            }
            if index == stops.len() {
                return Ok(stops[index - 1].color);
            }
            let left = &stops[index - 1];
            let right = &stops[index];
            let left_position = left.position.unwrap_or(0.);
            let fraction =
                (position - left_position) / (right.position.unwrap_or(1.) - left_position);
            Ok(std::array::from_fn(|channel| {
                (f64::from(left.color[channel]) * (1. - fraction)
                    + f64::from(right.color[channel]) * fraction)
                    .round() as u8
            }))
        };
        for triangle in triangles {
            subdivide(
                triangle.map(|i| vertices[i].clone()),
                0,
                &evaluate,
                &mut points,
            )?;
        }
    }
    let style = GouraudFillStyle {
        points,
        vertices_per_row: if linear && original_topology {
            columns
        } else {
            None
        },
        extend: false,
        back_color: None,
        back_alpha: None,
    };
    style.validate()?;
    Ok(Some(style))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_fixture(pdf: &PdfDocument, kind: i64) -> Result<Option<GouraudFillStyle>, OfdError> {
        let resources = Object::Dictionary(lopdf::Dictionary::new());
        let page = PdfPageContext {
            pdf,
            resources: &resources,
            box_: [0., 0., 96., 96.],
            index: 0,
        };
        parse(
            pdf.get_object((5, 0)).unwrap(),
            &page,
            &GraphicsState::default(),
            kind,
        )
    }

    fn source() -> PdfDocument {
        PdfDocument::load_mem(include_bytes!("../../tests/fixtures/mesh-free-rgb.pdf")).unwrap()
    }

    fn packed_record(fields: &[(u32, usize)]) -> Vec<u8> {
        let mut output = vec![0; fields.iter().map(|(_, n)| n).sum::<usize>().div_ceil(8)];
        let mut position = 0;
        for (value, width) in fields {
            for shift in (0..*width).rev() {
                output[position / 8] |= (((value >> shift) & 1) as u8) << (7 - position % 8);
                position += 1;
            }
        }
        output
    }

    #[test]
    fn all_mesh_bit_depths_and_reserved_flag_bits_decode_without_overflow() {
        for coordinates in [1, 2, 4, 8, 12, 16, 24, 32] {
            for components in [1, 2, 4, 8, 12, 16] {
                let mut pdf = source();
                let stream = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
                stream.decompress().unwrap();
                let coordinate_max = ((1_u64 << coordinates) - 1) as u32;
                let component_max = ((1_u64 << components) - 1) as u32;
                let mut data = Vec::new();
                for (index, (x, y)) in [(0, 0), (coordinate_max, 0), (0, coordinate_max)]
                    .into_iter()
                    .enumerate()
                {
                    // The first flag uses only the low two bits; the following
                    // two flags are ignored when starting a new triangle.
                    let mut fields = vec![
                        (if index == 0 { 0xfc } else { 0xff }, 8),
                        (x, coordinates),
                        (y, coordinates),
                    ];
                    fields.extend((0..3).map(|channel| {
                        (if channel == index { component_max } else { 0 }, components)
                    }));
                    data.extend(packed_record(&fields));
                }
                stream.dict.set("BitsPerCoordinate", coordinates as i64);
                stream.dict.set("BitsPerComponent", components as i64);
                stream.dict.set("BitsPerFlag", 8);
                stream.set_content(data);
                let mesh = parse_fixture(&pdf, 4).unwrap().unwrap();
                assert_eq!(mesh.points.len(), 3);
                assert_eq!(mesh.points[0].color, [255, 0, 0]);
                assert_eq!(mesh.points[1].color, [0, 255, 0]);
                assert_eq!(mesh.points[2].color, [0, 0, 255]);
                assert!(mesh.points.iter().all(|point| point.edge_flag == 0));
            }
        }
        let mut bits = Bits {
            bytes: &[0],
            position: 8,
        };
        assert!(bits.read(1).is_err());
    }

    #[test]
    fn invalid_mesh_edge_sequences_and_lattices_return_errors() {
        let vertex = |flag| Vertex {
            position: [0., 0.],
            components: vec![0.],
            flag,
        };
        for flags in [
            vec![],
            vec![0],
            vec![0, 0],
            vec![1],
            vec![2],
            vec![3],
            vec![0, 0, 0, 3],
        ] {
            let points = flags.into_iter().map(vertex).collect::<Vec<_>>();
            assert!(triangles(&points, None).is_err());
        }
        for (length, columns) in [(0, 2), (4, 0), (4, 1), (5, 2), (4, 3)] {
            assert!(triangles(&vec![vertex(0); length], Some(columns)).is_err());
        }
        assert_eq!(
            triangles(&vec![vertex(0); 6], Some(3)).unwrap(),
            [[0, 1, 3], [1, 3, 4], [1, 2, 4], [2, 4, 5]]
        );
    }

    #[test]
    fn mesh_degenerate_triangles_and_empty_streams_have_distinct_behavior() {
        let mut pdf = source();
        let stream = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
        stream.decompress().unwrap();
        stream.set_content(vec![0; 18]); // three coincident six-byte vertices
        assert!(parse_fixture(&pdf, 4).unwrap().is_none());
        pdf.get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_content(Vec::new());
        assert!(parse_fixture(&pdf, 4).is_err());
    }

    #[test]
    fn mesh_vertex_and_subdivision_work_are_bounded() {
        let mut pdf = source();
        let stream = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
        stream.decompress().unwrap();
        stream.set_content(vec![0; 6 * (MAX_POINTS + 1)]);
        assert!(matches!(parse_fixture(&pdf, 4), Err(OfdError::Limit(_))));
        let vertex = Vertex {
            position: [0., 0.],
            components: vec![0.],
            flag: 0,
        };
        let point = GouraudControlPoint {
            x: 0.,
            y: 0.,
            color: [0; 3],
            alpha: None,
            edge_flag: 0,
        };
        let mut points = vec![point; MAX_POINTS];
        assert!(matches!(
            subdivide(
                [vertex.clone(), vertex.clone(), vertex],
                0,
                &|_| Ok([0; 3]),
                &mut points
            ),
            Err(OfdError::Limit(_))
        ));
    }

    #[test]
    fn mesh_constant_function_input_retains_its_color() {
        let mut pdf =
            PdfDocument::load_mem(include_bytes!("../../tests/fixtures/mesh-function.pdf"))
                .unwrap();
        let stream = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
        stream.dict.set(
            "Decode",
            vec![
                8.into(),
                88.into(),
                8.into(),
                88.into(),
                Object::Real(0.5),
                Object::Real(0.5),
            ],
        );
        let mesh = parse_fixture(&pdf, 4).unwrap().unwrap();
        assert!(mesh.points.iter().all(|point| point.color == [191, 0, 64]));
    }

    #[test]
    fn mesh_skips_degenerate_triangles_without_losing_later_geometry() {
        let mut pdf = source();
        let stream = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
        stream.decompress().unwrap();
        let mut data = vec![0; 18];
        for (index, (x, y)) in [(0, 0), (255, 0), (0, 255)].into_iter().enumerate() {
            let mut fields = vec![(0, 2), (x, 8), (y, 8)];
            fields.extend((0..3).map(|channel| (if channel == index { 255 } else { 0 }, 8)));
            data.extend(packed_record(&fields));
        }
        stream.set_content(data);
        let mesh = parse_fixture(&pdf, 4).unwrap().unwrap();
        assert_eq!(mesh.points.len(), 3);
        assert_eq!(mesh.points[0].color, [255, 0, 0]);
        assert_eq!(mesh.points[1].color, [0, 255, 0]);
        assert_eq!(mesh.points[2].color, [0, 0, 255]);
    }

    #[test]
    fn mesh_decode_clips_after_interpolation_and_refuses_unresolvable_color_steps() {
        let mut pdf = source();
        pdf.get_object_mut((5, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set(
                "Decode",
                vec![
                    8.into(),
                    88.into(),
                    8.into(),
                    88.into(),
                    (-1).into(),
                    2.into(),
                    0.into(),
                    1.into(),
                    0.into(),
                    1.into(),
                ],
            );
        let mesh = parse_fixture(&pdf, 4).unwrap().unwrap();
        assert!(mesh.points.len() > 8);
        assert!(mesh.points.iter().any(|point| point.color[0] == 0));
        assert!(mesh.points.iter().any(|point| point.color[0] == 255));
        let vertices = [0., 1., 0.].map(|value| Vertex {
            position: [value, 0.],
            components: vec![value],
            flag: 0,
        });
        let mut points = Vec::new();
        let evaluate = |values: &[f64]| Ok([if values[0] > 0.123 { 255 } else { 0 }; 3]);
        assert!(matches!(
            subdivide(vertices, 0, &evaluate, &mut points),
            Err(OfdError::Limit(_))
        ));
    }
}
