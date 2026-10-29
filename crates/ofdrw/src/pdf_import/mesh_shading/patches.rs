//! PDF Coons and tensor patches, approximated in both parameter-space color and geometry.

use super::{invalid, Bits, GouraudControlPoint, MeshColors, MeshSamples, OfdError, MAX_POINTS};

const MAX_PATCHES: usize = 4096;
const GEOMETRY_TOLERANCE_MM: f64 = 0.025;

#[derive(Clone)]
struct Patch {
    points: [[f64; 2]; 16],
    colors: [Vec<f64>; 4],
    tensor: bool,
}

fn bernstein(t: f64) -> [f64; 4] {
    let s = 1. - t;
    [s * s * s, 3. * s * s * t, 3. * s * t * t, t * t * t]
}

fn curve(points: [[f64; 2]; 4], t: f64) -> [f64; 2] {
    let weights = bernstein(t);
    std::array::from_fn(|axis| (0..4).map(|i| points[i][axis] * weights[i]).sum())
}

impl Patch {
    fn position(&self, u: f64, v: f64) -> [f64; 2] {
        let p = &self.points;
        if self.tensor {
            // The stream follows the boundary, then gives the four internal points.
            let indices = [[0, 1, 2, 3], [11, 12, 13, 4], [10, 15, 14, 5], [9, 8, 7, 6]];
            let bu = bernstein(u);
            let bv = bernstein(v);
            std::array::from_fn(|axis| {
                (0..4)
                    .flat_map(|i| (0..4).map(move |j| p[indices[i][j]][axis] * bu[i] * bv[j]))
                    .sum()
            })
        } else {
            let left = curve([p[0], p[1], p[2], p[3]], v);
            let right = curve([p[9], p[8], p[7], p[6]], v);
            let bottom = curve([p[0], p[11], p[10], p[9]], u);
            let top = curve([p[3], p[4], p[5], p[6]], u);
            std::array::from_fn(|axis| {
                (1. - u) * left[axis] + u * right[axis] + (1. - v) * bottom[axis] + v * top[axis]
                    - ((1. - u) * (1. - v) * p[0][axis]
                        + (1. - u) * v * p[3][axis]
                        + u * v * p[6][axis]
                        + u * (1. - v) * p[9][axis])
            })
        }
    }

    fn sample(
        &self,
        u: f64,
        v: f64,
        colors: &MeshColors<'_>,
    ) -> Result<GouraudControlPoint, OfdError> {
        let [x, y] = self.position(u, v);
        if !x.is_finite() || !y.is_finite() || x.abs() > 1e9 || y.abs() > 1e9 {
            return Err(invalid());
        }
        let weights = [(1. - u) * (1. - v), (1. - u) * v, u * v, u * (1. - v)];
        let values = (0..self.colors[0].len())
            .map(|channel| (0..4).map(|i| self.colors[i][channel] * weights[i]).sum())
            .collect::<Vec<_>>();
        Ok(GouraudControlPoint {
            x,
            y,
            color: colors.evaluate(&values)?,
            alpha: None,
            edge_flag: 0,
        })
    }
}

fn interpolated(vertices: &[GouraudControlPoint; 3], weights: [f64; 3]) -> ([f64; 2], [f64; 3]) {
    let position = [
        (0..3).map(|i| vertices[i].x * weights[i]).sum(),
        (0..3).map(|i| vertices[i].y * weights[i]).sum(),
    ];
    let color = std::array::from_fn(|channel| {
        (0..3)
            .map(|i| f64::from(vertices[i].color[channel]) * weights[i])
            .sum()
    });
    (position, color)
}

fn tessellate(
    patch: &Patch,
    colors: &MeshColors<'_>,
    output: &mut Vec<GouraudControlPoint>,
) -> Result<(), OfdError> {
    // Uniform grids keep neighboring cells on identical edges. Increase the grid
    // only while independent surface probes fail either error tolerance.
    for depth in 0..=7 {
        let cells = 1usize << depth;
        if output.len() + 6 * cells * cells > MAX_POINTS {
            return Err(OfdError::Limit(
                "PDF patch conversion exceeds 100000 mesh points".into(),
            ));
        }
        let mut result = Vec::with_capacity(6 * cells * cells);
        let mut accurate = true;
        'cells: for row in 0..cells {
            for column in 0..cells {
                let u0 = column as f64 / cells as f64;
                let u1 = (column + 1) as f64 / cells as f64;
                let v0 = row as f64 / cells as f64;
                let v1 = (row + 1) as f64 / cells as f64;
                let a = patch.sample(u0, v0, colors)?;
                let b = patch.sample(u1, v0, colors)?;
                let c = patch.sample(u0, v1, colors)?;
                let d = patch.sample(u1, v1, colors)?;
                let triangles = [[a.clone(), b.clone(), c.clone()], [b, d, c]];
                for j in 0..=4 {
                    for i in 0..=4 {
                        let s = f64::from(i) / 4.;
                        let t = f64::from(j) / 4.;
                        let actual =
                            patch.sample(u0 + s * (u1 - u0), v0 + t * (v1 - v0), colors)?;
                        let (vertices, weights) = if s + t <= 1. {
                            (&triangles[0], [1. - s - t, s, t])
                        } else {
                            (&triangles[1], [1. - t, s + t - 1., 1. - s])
                        };
                        let (position, color) = interpolated(vertices, weights);
                        let geometry_error = (actual.x - position[0]).hypot(actual.y - position[1]);
                        if geometry_error > GEOMETRY_TOLERANCE_MM
                            || (0..3).any(|channel| {
                                (f64::from(actual.color[channel]) - color[channel]).abs() > 2.
                            })
                        {
                            accurate = false;
                            break 'cells;
                        }
                    }
                }
                for triangle in triangles {
                    let [a, b, c] = &triangle;
                    let area = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
                    if area.abs() >= 1e-12 {
                        result.extend(triangle);
                    }
                }
            }
        }
        if accurate {
            output.extend(result);
            return Ok(());
        }
    }
    Err(OfdError::Limit(
        "PDF patch cannot meet geometry or color tolerance".into(),
    ))
}

pub(super) fn parse(
    samples: &MeshSamples<'_, '_>,
    colors: &MeshColors<'_>,
    kind: i64,
) -> Result<Vec<GouraudControlPoint>, OfdError> {
    let mut bits = Bits {
        bytes: samples.data,
        position: 0,
    };
    let mut previous: Option<Patch> = None;
    let mut count = 0;
    let mut output = Vec::new();
    let point_count = if kind == 7 { 16 } else { 12 };
    // Each patch ends at a byte boundary; padding bits are ignored.
    while samples.data.len() * 8 - bits.position > 7 {
        count += 1;
        if count > MAX_PATCHES {
            return Err(OfdError::Limit(
                "PDF patch mesh exceeds 4096 patches".into(),
            ));
        }
        let flag = bits.read(samples.flag_bits)? as usize & 3;
        let mut patch = Patch {
            points: [[0.; 2]; 16],
            colors: std::array::from_fn(|_| Vec::new()),
            tensor: kind == 7,
        };
        let (first_point, first_color) = if flag == 0 {
            (0, 0)
        } else {
            let previous = previous.as_ref().ok_or_else(invalid)?;
            for i in 0..4 {
                patch.points[i] = previous.points[(flag * 3 + i) % 12];
            }
            patch.colors[0] = previous.colors[flag].clone();
            patch.colors[1] = previous.colors[(flag + 1) % 4].clone();
            (4, 2)
        };
        for i in first_point..point_count {
            patch.points[i] = samples.point(&mut bits)?;
        }
        for i in first_color..4 {
            patch.colors[i] = samples.components(&mut bits)?;
        }
        tessellate(&patch, colors, &mut output)?;
        previous = Some(patch);
        bits.position = bits.position.div_ceil(8) * 8;
    }
    if count == 0 {
        return Err(invalid());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::{GraphicsState, Object, PdfColorSpace, PdfDocument, PdfPageContext};
    use super::*;

    fn planar(tensor: bool) -> Patch {
        let points = [
            [0., 0.],
            [0., 1.],
            [0., 2.],
            [0., 3.],
            [1., 3.],
            [2., 3.],
            [3., 3.],
            [3., 2.],
            [3., 1.],
            [3., 0.],
            [2., 0.],
            [1., 0.],
            [1., 1.],
            [1., 2.],
            [2., 2.],
            [2., 1.],
        ];
        Patch {
            points,
            colors: [
                vec![1., 0., 0.],
                vec![0., 1., 0.],
                vec![0., 0., 1.],
                vec![1., 1., 0.],
            ],
            tensor,
        }
    }

    #[test]
    fn coons_and_tensor_geometry_agree_on_planar_control_points() {
        for tensor in [false, true] {
            let patch = planar(tensor);
            for u in [0., 0.125, 0.5, 0.875, 1.] {
                for v in [0., 0.125, 0.5, 0.875, 1.] {
                    let position = patch.position(u, v);
                    assert!((position[0] - u * 3.).abs() < 1e-12);
                    assert!((position[1] - v * 3.).abs() < 1e-12);
                }
            }
        }
        assert_eq!(
            curve([[0., 0.], [1., 1.], [2., 1.], [3., 0.]], 0.5),
            [1.5, 0.75]
        );
    }

    #[test]
    fn patch_colors_interpolate_bilinearly_before_conversion_and_refinement() {
        let pdf = PdfDocument::new();
        let resources = Object::Dictionary(lopdf::Dictionary::new());
        let page = PdfPageContext {
            pdf: &pdf,
            resources: &resources,
            box_: [0., 0., 96., 96.],
            index: 0,
            raster_scale: 1.,
        };
        let colors = MeshColors::new(
            &page,
            &GraphicsState::default(),
            &PdfColorSpace::Rgb,
            None,
            &[0., 1., 0., 1., 0., 1.],
        )
        .unwrap();
        let patch = planar(false);
        let point = patch.sample(0.5, 0.5, &colors).unwrap();
        assert_eq!([point.x, point.y], [1.5, 1.5]);
        assert_eq!(point.color, [128, 128, 64]);
        let mut points = Vec::new();
        tessellate(&patch, &colors, &mut points).unwrap();
        assert!(points.len() > 6);
        assert!(points.len() <= MAX_POINTS);
        let mut invalid = patch;
        invalid.points[0][0] = f64::INFINITY;
        assert!(invalid.sample(0., 0., &colors).is_err());
    }

    #[test]
    fn patch_tessellation_has_point_color_and_geometry_limits() {
        let pdf = PdfDocument::new();
        let resources = Object::Dictionary(lopdf::Dictionary::new());
        let page = PdfPageContext {
            pdf: &pdf,
            resources: &resources,
            box_: [0., 0., 96., 96.],
            index: 0,
            raster_scale: 1.,
        };
        let colors = MeshColors::new(
            &page,
            &GraphicsState::default(),
            &PdfColorSpace::Rgb,
            None,
            &[0., 1., 0., 1., 0., 1.],
        )
        .unwrap();
        let patch = planar(true);
        let mut points = vec![
            GouraudControlPoint {
                x: 0.,
                y: 0.,
                color: [0; 3],
                alpha: None,
                edge_flag: 0
            };
            MAX_POINTS
        ];
        assert!(matches!(
            tessellate(&patch, &colors, &mut points),
            Err(OfdError::Limit(_))
        ));
        points.clear();
        let mut curved = patch.clone();
        curved.points[12] = [1e8, 1e8];
        assert!(matches!(
            tessellate(&curved, &colors, &mut points),
            Err(OfdError::Limit(_))
        ));
        colors.remaining.set(0);
        assert!(matches!(
            tessellate(&patch, &colors, &mut points),
            Err(OfdError::Limit(_))
        ));
    }

    #[test]
    fn patch_streams_reject_truncation_or_reuse_without_a_previous_patch() {
        let original = PdfDocument::load_mem(include_bytes!(
            "../../../tests/fixtures/patch-coons-flat.pdf"
        ))
        .unwrap();
        let raw = original
            .get_object((5, 0))
            .unwrap()
            .as_stream()
            .unwrap()
            .get_plain_content()
            .unwrap();
        let run = |data: Vec<u8>| {
            let mut pdf = original.clone();
            let stream = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
            stream.dict.remove(b"Filter");
            stream.set_content(data);
            let mut bytes = Vec::new();
            pdf.save_to(&mut bytes).unwrap();
            crate::OfdDocument::from_pdf_bytes(&bytes)
        };
        for count in 0..raw.len() {
            assert!(run(raw[..count].to_vec()).is_err(), "truncated at {count}");
        }
        for flag in 1..=3 {
            let mut data = raw.clone();
            data[0] |= flag << 6;
            assert!(run(data).is_err());
        }
        let mut extra = raw;
        extra.push(0);
        assert!(run(extra).is_err());
    }

    #[test]
    fn patch_padding_is_ignored_and_degenerate_patch_count_is_bounded() {
        let pdf = PdfDocument::new();
        let resources = Object::Dictionary(lopdf::Dictionary::new());
        let page = PdfPageContext {
            pdf: &pdf,
            resources: &resources,
            box_: [0., 0., 96., 96.],
            index: 0,
            raster_scale: 1.,
        };
        let state = GraphicsState::default();
        let colors = MeshColors::new(
            &page,
            &state,
            &PdfColorSpace::Gray,
            None,
            &[0., 1., 0., 1., 0., 1.],
        )
        .unwrap();
        // Each 30-bit patch occupies four bytes including two ignored padding bits.
        let data = vec![0; 4 * (MAX_PATCHES + 1)];
        let samples = MeshSamples {
            data: &data,
            decode: &[0., 1., 0., 1., 0., 1.],
            coordinate_bits: 1,
            component_bits: 1,
            flag_bits: 2,
            channels: 1,
            page: &page,
            state: &state,
        };
        let padded = [0, 0, 0, 3, 0, 0, 0, 3];
        let pair = MeshSamples {
            data: &padded,
            ..samples
        };
        assert!(parse(&pair, &colors, 6).unwrap().is_empty());
        assert!(matches!(
            parse(&samples, &colors, 6),
            Err(OfdError::Limit(_))
        ));
    }
}
