use pdf_writer::types::{FunctionShadingType, MaskType};
use pdf_writer::{Chunk, Content, Filter, Finish, Name, Ref};
use usvg::{Paint, Rect, SpreadMethod, Transform};

use crate::{ConversionError, Result};

use crate::util::context::Context;
use crate::util::helper::{bbox_to_non_zero_rect, NameExt, RectExt, StopExt, TransformExt};
use crate::util::resources::ResourceContainer;

/// An alternative representation of a usvg::Stop that allows us to store
/// both, RGB gradients and grayscale gradients.
#[derive(Copy, Clone)]
pub struct Stop<const COUNT: usize> {
    pub color: [f32; COUNT],
    pub offset: f32,
}

struct GradientProperties {
    coords: Vec<f32>,
    shading_type: FunctionShadingType,
    stops: Vec<usvg::Stop>,
    transform: Transform,
    spread: SpreadMethod,
    domain: [f32; 2],
}

impl GradientProperties {
    fn try_from_paint(paint: &Paint) -> Option<Self> {
        match paint {
            Paint::LinearGradient(l) => Some(Self {
                coords: vec![l.x1(), l.y1(), l.x2(), l.y2()],
                shading_type: FunctionShadingType::Axial,
                stops: Vec::from(l.stops()),
                transform: l.transform(),
                spread: l.spread_method(),
                domain: [0., 1.],
            }),
            Paint::RadialGradient(r) => Some(Self {
                coords: vec![r.fx(), r.fy(), r.fr().get(), r.cx(), r.cy(), r.r().get()],
                shading_type: FunctionShadingType::Radial,
                stops: Vec::from(r.stops()),
                transform: r.transform(),
                spread: r.spread_method(),
                domain: [0., 1.],
            }),
            _ => None,
        }
    }

    fn cover(&mut self, bbox: Rect) -> Result<()> {
        if self.spread == SpreadMethod::Pad {
            return Ok(());
        }
        let inverse = self
            .transform
            .invert()
            .ok_or(ConversionError::InvalidGradient)?;
        let mut points = [
            usvg::tiny_skia_path::Point::from_xy(bbox.left(), bbox.top()),
            usvg::tiny_skia_path::Point::from_xy(bbox.right(), bbox.top()),
            usvg::tiny_skia_path::Point::from_xy(bbox.left(), bbox.bottom()),
            usvg::tiny_skia_path::Point::from_xy(bbox.right(), bbox.bottom()),
        ];
        inverse.map_points(&mut points);
        if points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return Err(ConversionError::InvalidGradient);
        }
        let coords: Vec<_> = self.coords.iter().map(|&v| f64::from(v)).collect();
        let mut lower = 0_f64;
        let mut upper = 1_f64;
        match coords.as_slice() {
            &[x, y, ex, ey] => {
                let (dx, dy) = (ex - x, ey - y);
                let length = dx * dx + dy * dy;
                if length == 0. {
                    return Err(ConversionError::InvalidGradient);
                }
                for point in points {
                    let t =
                        ((f64::from(point.x) - x) * dx + (f64::from(point.y) - y) * dy) / length;
                    lower = lower.min(t.floor());
                    upper = upper.max(t.ceil());
                }
            }
            &[x, y, radius, ex, ey, er] => {
                let (dx, dy, dr) = (ex - x, ey - y, er - radius);
                let a = dx * dx + dy * dy - dr * dr;
                // SVG's increasing nested circles have convex sublevel sets:
                // a circle containing the four corners contains the whole box.
                if dr <= 0. || a >= 0. {
                    return Err(ConversionError::InvalidGradient);
                }
                lower = -radius / dr;
                for point in points {
                    let (px, py) = (f64::from(point.x) - x, f64::from(point.y) - y);
                    let b = -2. * (px * dx + py * dy + radius * dr);
                    let c = px * px + py * py - radius * radius;
                    let discriminant = (b * b - 4. * a * c).max(0.);
                    let q = -0.5 * (b + discriminant.sqrt().copysign(b));
                    let t = if q == 0. { 0. } else { (q / a).max(c / q) };
                    upper = upper.max(t.ceil());
                }
            }
            _ => return Err(ConversionError::InvalidGradient),
        }
        if !lower.is_finite() || !upper.is_finite() || upper.ceil() - lower.floor() > 16_777_216. {
            return Err(ConversionError::InvalidGradient);
        }
        // Coordinates and domains are PDF real numbers (f32 in pdf-writer).
        // Keep the radius-zero end exact rather than emitting a negative radius
        // after rounding a fractional lower domain bound.
        let mut expanded = Vec::with_capacity(coords.len());
        let half = coords.len() / 2;
        for t in [lower, upper] {
            for i in 0..half {
                expanded.push((coords[i] + (coords[i + half] - coords[i]) * t) as f32);
            }
        }
        if half == 3 {
            expanded[2] = 0.;
        }
        if expanded.iter().any(|v| !v.is_finite()) {
            return Err(ConversionError::InvalidGradient);
        }
        self.coords = expanded;
        self.domain = [lower as f32, upper as f32];
        Ok(())
    }
}

/// Turn a (gradient) paint into a shading pattern object. Stop opacities will be ignored and
/// need to be rendered separately using `create_shading_soft_mask`. The paint
/// needs to be either a linear gradient or a radial gradient.
pub fn create_shading_pattern(
    paint: &Paint,
    chunk: &mut Chunk,
    ctx: &mut Context,
    accumulated_transform: &Transform,
    bbox: Rect,
) -> Result<Ref> {
    let mut properties =
        GradientProperties::try_from_paint(paint).ok_or(ConversionError::InvalidGradient)?;
    properties.cover(bbox)?;
    Ok(shading_pattern(
        &properties,
        chunk,
        ctx,
        accumulated_transform,
    ))
}

/// Return a soft mask that will render the stop opacities of a gradient into a gray scale
/// shading. If no soft mask is necessary (because no stops have an opacity),
/// `None` will be returned.
pub fn create_shading_soft_mask(
    paint: &Paint,
    chunk: &mut Chunk,
    ctx: &mut Context,
    bbox: Rect,
) -> Result<Option<Ref>> {
    let mut properties =
        GradientProperties::try_from_paint(paint).ok_or(ConversionError::InvalidGradient)?;
    if properties
        .stops
        .iter()
        .any(|stop| stop.opacity().get() < 1.0)
    {
        properties.cover(bbox)?;
        Ok(Some(shading_soft_mask(&properties, chunk, ctx, bbox)))
    } else {
        Ok(None)
    }
}

fn shading_pattern(
    properties: &GradientProperties,
    chunk: &mut Chunk,
    ctx: &mut Context,
    accumulated_transform: &Transform,
) -> Ref {
    let pattern_ref = ctx.alloc_ref();

    let matrix = accumulated_transform.pre_concat(properties.transform);

    let shading_ref = shading_function(properties, chunk, ctx, false);
    let mut shading_pattern = chunk.shading_pattern(pattern_ref);
    shading_pattern.pair(Name(b"Shading"), shading_ref);
    shading_pattern.matrix(matrix.to_pdf_transform());
    shading_pattern.finish();

    pattern_ref
}

fn shading_soft_mask(
    properties: &GradientProperties,
    chunk: &mut Chunk,
    ctx: &mut Context,
    bbox: Rect,
) -> Ref {
    let mut rc = ResourceContainer::new();
    let x_object_id = ctx.alloc_ref();
    let shading_ref = shading_function(properties, chunk, ctx, true);
    let shading_name = rc.add_shading(shading_ref);
    let bbox = bbox_to_non_zero_rect(Some(bbox)).to_pdf_rect();

    let transform = properties.transform;

    let mut content = Content::new();
    content.transform(transform.to_pdf_transform());
    content.shading(shading_name.to_pdf_name());
    let content_stream = ctx.finish_content(content);

    let mut x_object = chunk.form_xobject(x_object_id, &content_stream);
    rc.finish(&mut x_object.resources());

    x_object
        .group()
        .transparency()
        .isolated(false)
        .knockout(false)
        .color_space()
        .icc_based(ctx.sgray_ref());

    if ctx.options.compress {
        x_object.filter(Filter::FlateDecode);
    }

    x_object.bbox(bbox);
    x_object.finish();

    let gs_ref = ctx.alloc_ref();
    let mut gs = chunk.ext_graphics(gs_ref);
    gs.soft_mask()
        .subtype(MaskType::Luminosity)
        .group(x_object_id)
        .finish();

    gs_ref
}

fn shading_function(
    properties: &GradientProperties,
    chunk: &mut Chunk,
    ctx: &mut Context,
    use_opacities: bool,
) -> Ref {
    let shading_ref = ctx.alloc_ref();
    let mut function_ref = function(&properties.stops, chunk, ctx, use_opacities);
    if properties.spread != SpreadMethod::Pad {
        function_ref = periodic_function(properties, function_ref, chunk, ctx, use_opacities);
    }

    let mut shading = chunk.function_shading(shading_ref);
    shading.shading_type(properties.shading_type);
    if use_opacities {
        shading.color_space().icc_based(ctx.sgray_ref());
    } else {
        shading.color_space().icc_based(ctx.srgb_ref());
    }

    shading.function(function_ref);
    shading.coords(properties.coords.iter().copied());
    shading
        .insert(Name(b"Domain"))
        .array()
        .items(properties.domain);
    shading.extend([true, true]);
    shading.finish();
    shading_ref
}

// Reuse each subtree twice. This retains explicit discontinuities for PDF
// renderers without writing one copy of every color stop for every period.
fn periodic_function(
    properties: &GradientProperties,
    mut child: Ref,
    chunk: &mut Chunk,
    ctx: &mut Context,
    opacity: bool,
) -> Ref {
    let [lower, upper] = properties.domain;
    let start = lower.floor();
    let mut width = 1_f32;
    let range = get_function_range(if opacity { 1 } else { 3 });
    while width < upper - start || (width == 1. && properties.spread == SpreadMethod::Reflect) {
        let reference = ctx.alloc_ref();
        let mut function = chunk.stitching_function(reference);
        function.domain([0., 2. * width]);
        function.range(range.iter().copied());
        function.functions([child, child]);
        function.bounds([width]);
        if width == 1. && properties.spread == SpreadMethod::Reflect {
            function.encode(if start.rem_euclid(2.) == 0. {
                [0., 1., 1., 0.]
            } else {
                [1., 0., 0., 1.]
            });
        } else {
            function.encode([0., width, 0., width]);
        }
        function.finish();
        child = reference;
        width *= 2.;
    }
    let reference = ctx.alloc_ref();
    let mut function = chunk.stitching_function(reference);
    function.domain([lower, upper]);
    function.range(range);
    function.functions([child]);
    function.bounds([]);
    function.encode([lower - start, upper - start]);
    function.finish();
    reference
}

fn function(
    stops: &[usvg::Stop],
    chunk: &mut Chunk,
    ctx: &mut Context,
    use_opacities: bool,
) -> Ref {
    // Gradients with no stops and only one stop should automatically be converted by resvg
    // into no fill / plain fill, so there should be at least two stops
    debug_assert!(stops.len() > 1);

    fn pad_stops<const COUNT: usize>(mut stops: Vec<Stop<COUNT>>) -> Vec<Stop<COUNT>> {
        // We manually pad the stops if necessary so that they are always in the range from 0-1
        if let Some(first) = stops.first() {
            if first.offset != 0.0 {
                let mut new_stop = *first;
                new_stop.offset = 0.0;
                stops.insert(0, new_stop);
            }
        }

        if let Some(last) = stops.last() {
            if last.offset != 1.0 {
                let mut new_stop = *last;
                new_stop.offset = 1.0;
                stops.push(new_stop);
            }
        }

        stops
    }

    if use_opacities {
        let stops = pad_stops(
            stops
                .iter()
                .map(|s| s.opacity_stops())
                .collect::<Vec<Stop<1>>>(),
        );
        select_function(&stops, chunk, ctx)
    } else {
        let stops = pad_stops(
            stops
                .iter()
                .map(|s| s.color_stops())
                .collect::<Vec<Stop<3>>>(),
        );
        select_function(&stops, chunk, ctx)
    }
}

fn select_function<const COUNT: usize>(
    stops: &[Stop<COUNT>],
    chunk: &mut Chunk,
    ctx: &mut Context,
) -> Ref {
    if stops.len() == 2 {
        exponential_function(&stops[0], &stops[1], chunk, ctx)
    } else {
        stitching_function(stops, chunk, ctx)
    }
}

fn stitching_function<const COUNT: usize>(
    stops: &[Stop<COUNT>],
    chunk: &mut Chunk,
    ctx: &mut Context,
) -> Ref {
    assert!(!stops.is_empty());

    let reference = ctx.alloc_ref();

    let mut functions = vec![];
    let mut bounds = vec![];
    let mut encode = vec![];

    for window in stops.windows(2) {
        let (first, second) = (&window[0], &window[1]);
        // Coincident SVG stops are hard transitions, not zero-width PDF
        // subdomains. Keep the colors on either side of the discontinuity.
        if first.offset == second.offset {
            continue;
        }
        bounds.push(second.offset);
        functions.push(exponential_function(first, second, chunk, ctx));
        encode.extend([0.0, 1.0]);
    }

    bounds.pop();

    let mut stitching_function = chunk.stitching_function(reference);
    stitching_function.domain([0.0, 1.0]);
    stitching_function.range(get_function_range(COUNT));
    stitching_function.functions(functions);
    stitching_function.bounds(bounds);
    stitching_function.encode(encode);
    reference
}

fn exponential_function<const COUNT: usize>(
    first_stop: &Stop<COUNT>,
    second_stop: &Stop<COUNT>,
    chunk: &mut Chunk,
    ctx: &mut Context,
) -> Ref {
    let reference = ctx.alloc_ref();
    let mut exp = chunk.exponential_function(reference);

    exp.range(get_function_range(COUNT));
    exp.c0(first_stop.color);
    exp.c1(second_stop.color);
    exp.domain([0.0, 1.0]);
    exp.n(1.0);
    exp.finish();
    reference
}

fn get_function_range(count: usize) -> Vec<f32> {
    [0.0, 1.0].repeat(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn properties(coords: &[f32]) -> GradientProperties {
        GradientProperties {
            coords: coords.to_vec(),
            shading_type: FunctionShadingType::Axial,
            stops: Vec::new(),
            transform: Transform::identity(),
            spread: SpreadMethod::Repeat,
            domain: [0., 1.],
        }
    }

    #[test]
    fn axial_bounds_cover_negative_and_positive_phases() {
        let mut gradient = properties(&[0., 0., 10., 0.]);
        gradient
            .cover(Rect::from_xywh(-5., -2., 20., 4.).unwrap())
            .unwrap();
        assert_eq!(
            (gradient.domain, gradient.coords),
            ([-1., 2.], vec![-10., 0., 20., 0.])
        );
    }

    #[test]
    fn focal_radius_starts_at_zero_radius_with_fractional_phase() {
        let mut gradient = properties(&[0., 0., 4., 0., 0., 10.]);
        gradient
            .cover(Rect::from_xywh(-10., -10., 20., 20.).unwrap())
            .unwrap();
        assert_eq!(
            (gradient.domain, gradient.coords),
            ([-2. / 3., 2.], vec![0., 0., 0., 0., 0., 16.])
        );
    }

    #[test]
    fn bounds_are_mapped_into_gradient_coordinates() {
        let mut gradient = properties(&[0., 0., 10., 0.]);
        gradient.transform = Transform::from_row(2., 0., 0., 0.5, 3., 4.);
        gradient
            .cover(Rect::from_xywh(-7., 3., 40., 2.).unwrap())
            .unwrap();
        assert_eq!(gradient.domain, [-1., 2.]);
    }

    #[test]
    fn invalid_periodic_geometry_returns_an_error() {
        for coords in [
            &[0., 0., 0., 0.][..],
            &[0., 0., 0., 10., 0., 1.],
            &[0., 0., 1e-8, 0.],
        ] {
            let mut gradient = properties(coords);
            assert!(matches!(
                gradient.cover(Rect::from_xywh(0., 0., 20., 20.).unwrap()),
                Err(ConversionError::InvalidGradient)
            ));
        }
        let mut gradient = properties(&[0., 0., 10., 0.]);
        gradient.transform = Transform::from_scale(0., 1.);
        assert!(matches!(
            gradient.cover(Rect::from_xywh(0., 0., 20., 20.).unwrap()),
            Err(ConversionError::InvalidGradient)
        ));
    }
}
