//! Bounded paint tiles for two-circle fields that SVG cannot represent.
use super::*;
use crate::two_circle::{dimensions, Geometry};

pub(super) struct RadialPaint {
    pub circles: [f64; 6],
    pub inverse: [f64; 6],
    pub spread: GradientMapType,
    pub extend: [bool; 2],
}

pub(super) struct Stop {
    pub position: f64,
    pub color: [u8; 4],
}

pub(super) fn magnification([a, b, c, d, _, _]: [f64; 6]) -> f64 {
    // sqrt(||A||1 * ||A||inf) bounds the largest singular value. Taking
    // square roots first avoids overflowing a representable scale product.
    ((a.abs() + b.abs()).max(c.abs() + d.abs()).sqrt()
        * (a.abs() + c.abs()).max(b.abs() + d.abs()).sqrt())
    .max(1.)
}

fn color(stops: &[Stop], parameter: f64) -> [u8; 4] {
    let next = stops.partition_point(|stop| stop.position <= parameter);
    if next == 0 {
        return stops[0].color;
    }
    if next == stops.len() {
        return stops[next - 1].color;
    }
    let (first, last) = (&stops[next - 1], &stops[next]);
    let t = (parameter - first.position) / (last.position - first.position);
    std::array::from_fn(|i| {
        (f64::from(first.color[i]) * (1. - t) + f64::from(last.color[i]) * t)
            .round()
            .clamp(0., 255.) as u8
    })
}

fn bounds(context: &SvgPaintContext<'_, '_>, object: Node<'_, '_>) -> Result<[f64; 4], OfdError> {
    if let Some(boundary) = object.attribute("Boundary") {
        let [_, _, width, height] = parse_box(boundary)
            .ok_or_else(|| OfdError::Invalid("invalid radial paint Boundary".into()))?;
        let matrix = match object.attribute("CTM") {
            Some(value) => parse_numbers::<6>(value)
                .ok_or_else(|| OfdError::Invalid("invalid radial paint CTM".into()))?,
            None => [1., 0., 0., 1., 0., 0.],
        };
        if let Some([a, b, c, d, e, f]) = inverse_matrix(matrix) {
            // Boundary clips before the object CTM. Cover its inverse image,
            // including coordinates outside the nominal local rectangle.
            let points = [[0., 0.], [width, 0.], [0., height], [width, height]]
                .map(|[x, y]| [a * x + c * y + e, b * x + d * y + f]);
            let left = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
            let top = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
            let right = points
                .iter()
                .map(|p| p[0])
                .fold(f64::NEG_INFINITY, f64::max);
            let bottom = points
                .iter()
                .map(|p| p[1])
                .fold(f64::NEG_INFINITY, f64::max);
            return Ok([left, top, right - left, bottom - top]);
        }
        return Ok([0., 0., width, height]);
    }
    let data = descendant(object, "AbbreviatedData")
        .map(element_text)
        .ok_or_else(|| OfdError::Invalid("radial paint object has no Boundary or path".into()))?;
    let path = svg_path_data(&data)?;
    let stroke = resolved_stroke_style(object, context.draw_style)?;
    let width = if stroke.line_width == 0. {
        1.
    } else {
        stroke.line_width
    };
    let cap = stroke.cap.as_deref().unwrap_or("butt");
    let join = stroke.join.as_deref().unwrap_or("miter");
    let miter = stroke.miter_limit.unwrap_or(3.528);
    let source = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"><path d=\"{}\" fill=\"black\" stroke=\"black\" stroke-width=\"{width}\" stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\" stroke-miterlimit=\"{miter}\"/></svg>", escape_xml(&path));
    let tree = usvg::Tree::from_str(&source, &usvg::Options::default())
        .map_err(|error| OfdError::Invalid(format!("invalid radial paint path: {error}")))?;
    let bounds = tree.root().stroke_bounding_box();
    Ok([
        f64::from(bounds.x()),
        f64::from(bounds.y()),
        f64::from(bounds.width()),
        f64::from(bounds.height()),
    ])
}

impl RadialPaint {
    fn parameter(&self, geometry: &Geometry, x: f64, y: f64) -> Option<f64> {
        let [a, b, c, d, e, f] = self.inverse;
        let value = geometry.parameter_unclamped(a * x + c * y + e, b * x + d * y + f)?;
        Some(match self.spread {
            GradientMapType::Direct => value.clamp(0., 1.),
            GradientMapType::Repeat => value.rem_euclid(1.),
            GradientMapType::Reflect => {
                let value = value.rem_euclid(2.);
                if value > 1. {
                    2. - value
                } else {
                    value
                }
            }
        })
    }

    pub(super) fn render(
        &self,
        context: &SvgPaintContext<'_, '_>,
        object: Node<'_, '_>,
        stops: &[Stop],
        id: &str,
        svg: &mut String,
    ) -> Result<String, OfdError> {
        if stops.len() < 2 || stops.len() > 10_000 {
            return Err(OfdError::Invalid(
                "radial paint requires 2..10000 stops".into(),
            ));
        }
        let geometry = Geometry::new(&self.circles, self.extend)?;
        let [x, y, width, height] = bounds(context, object)?;
        if ![x, y, width, height].iter().all(|value| value.is_finite()) {
            return Err(OfdError::Limit(
                "radial paint bounds exceed coordinate range".into(),
            ));
        }
        let [columns, rows] = dimensions(width, height, context.resources.raster_scale)?;
        let mut pixels = vec![0; columns as usize * rows as usize * 4];
        if self.circles[2] != 0. || self.circles[5] != 0. {
            for row in 0..rows {
                let py = y + (f64::from(row) + 0.5) * height / f64::from(rows);
                for column in 0..columns {
                    let px = x + (f64::from(column) + 0.5) * width / f64::from(columns);
                    if let Some(parameter) = self.parameter(&geometry, px, py) {
                        let offset = (row as usize * columns as usize + column as usize) * 4;
                        pixels[offset..offset + 4].copy_from_slice(&color(stops, parameter));
                    }
                }
            }
        }
        let mut png = Vec::new();
        let mut encoder = png::Encoder::new(&mut png, columns, rows);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&pixels))
            .map_err(|error| OfdError::Invalid(format!("cannot encode radial paint: {error}")))?;
        let data = STANDARD.encode(png);
        let sx = width / f64::from(columns);
        let sy = height / f64::from(rows);
        // Integer pixel-space tiles avoid reader-dependent rounding of tiny
        // pattern rectangles under large object transforms.
        svg.push_str(&format!("<defs><pattern id=\"{id}\" patternUnits=\"userSpaceOnUse\" width=\"{columns}\" height=\"{rows}\" patternTransform=\"matrix({sx} 0 0 {sy} {x} {y})\"><image width=\"{columns}\" height=\"{rows}\" preserveAspectRatio=\"none\" href=\"data:image/png;base64,{data}\"/></pattern></defs>"));
        Ok(format!("url(#{id})"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periodic_fields_preserve_unclamped_roots_and_negative_cycles() {
        let mut paint = RadialPaint {
            circles: [0., 0., 1., 4., 0., 1.],
            inverse: [1., 0., 0., 1., 0., 0.],
            spread: GradientMapType::Repeat,
            extend: [true; 2],
        };
        let geometry = Geometry::new(&paint.circles, paint.extend).unwrap();
        assert!((geometry.parameter_unclamped(6., 0.).unwrap() - 1.75).abs() < 1e-14);
        assert_eq!(geometry.parameter(6., 0.), Some(1.));
        assert!((paint.parameter(&geometry, 6., 0.).unwrap() - 0.75).abs() < 1e-14);
        assert!((paint.parameter(&geometry, -2., 0.).unwrap() - 0.75).abs() < 1e-14);
        paint.spread = GradientMapType::Reflect;
        assert!((paint.parameter(&geometry, 6., 0.).unwrap() - 0.25).abs() < 1e-14);
        assert!((paint.parameter(&geometry, -2., 0.).unwrap() - 0.25).abs() < 1e-14);
    }

    #[test]
    fn color_stops_keep_hard_transitions_and_interpolate_opacity() {
        let stops = [
            Stop {
                position: 0.,
                color: [255, 0, 0, 0],
            },
            Stop {
                position: 0.5,
                color: [255, 0, 0, 128],
            },
            Stop {
                position: 0.5,
                color: [0, 0, 255, 128],
            },
            Stop {
                position: 1.,
                color: [0, 255, 0, 255],
            },
        ];
        assert_eq!(color(&stops, -1.), [255, 0, 0, 0]);
        assert_eq!(color(&stops, 0.25), [255, 0, 0, 64]);
        assert_eq!(color(&stops, 0.5), [0, 0, 255, 128]);
        assert_eq!(color(&stops, 0.75), [0, 128, 128, 192]);
        assert_eq!(color(&stops, 2.), [0, 255, 0, 255]);
    }

    #[test]
    fn scale_estimate_handles_reflections_shears_and_extreme_magnification() {
        assert_eq!(magnification([1., 0., 0., 1., 0., 0.]), 1.);
        assert_eq!(magnification([-4., 0., 0., 2., 0., 0.]), 4.);
        assert!(magnification([1., 1., 0., 1., 0., 0.]) >= 1.618);
        assert!((magnification([1e300, 0., 0., 1e300, 0., 0.]) / 1e300 - 1.).abs() < 1e-14);
    }
}
