// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::render::Context;

pub fn render(
    path: &usvg::Path,
    blend_mode: tiny_skia::BlendMode,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    if !path.is_visible() {
        return;
    }

    if path.paint_order() == usvg::PaintOrder::FillAndStroke {
        fill_path(path, blend_mode, ctx, transform, pixmap);
        stroke_path(path, blend_mode, ctx, transform, pixmap);
    } else {
        stroke_path(path, blend_mode, ctx, transform, pixmap);
        fill_path(path, blend_mode, ctx, transform, pixmap);
    }
}

pub fn fill_path(
    path: &usvg::Path,
    blend_mode: tiny_skia::BlendMode,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    let fill = path.fill()?;

    // Horizontal and vertical lines cannot be filled. Skip.
    if path.data().bounds().width() == 0.0 || path.data().bounds().height() == 0.0 {
        return None;
    }

    let rule = match fill.rule() {
        usvg::FillRule::NonZero => tiny_skia::FillRule::Winding,
        usvg::FillRule::EvenOdd => tiny_skia::FillRule::EvenOdd,
    };

    let pattern_pixmap;
    let mut paint = tiny_skia::Paint::default();
    match fill.paint() {
        usvg::Paint::Color(c) => {
            paint.set_color_rgba8(c.red, c.green, c.blue, fill.opacity().to_u8());
        }
        usvg::Paint::LinearGradient(lg) => {
            paint.shader = convert_linear_gradient(lg, fill.opacity())?;
        }
        usvg::Paint::RadialGradient(rg) => {
            paint.shader = convert_radial_gradient(rg, fill.opacity())?;
        }
        usvg::Paint::Pattern(pattern) => {
            let (patt_pix, patt_ts) = render_pattern_pixmap(pattern, ctx, transform)?;

            pattern_pixmap = patt_pix;
            paint.shader = tiny_skia::Pattern::new(
                pattern_pixmap.as_ref(),
                tiny_skia::SpreadMode::Repeat,
                tiny_skia::FilterQuality::Bicubic,
                fill.opacity().get(),
                patt_ts,
            );
        }
    }
    paint.anti_alias = path.rendering_mode().use_shape_antialiasing();
    paint.blend_mode = blend_mode;

    pixmap.fill_path(path.data(), &paint, rule, transform, None);
    Some(())
}

fn stroke_path(
    path: &usvg::Path,
    blend_mode: tiny_skia::BlendMode,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    let stroke = path.stroke()?;
    let pattern_pixmap;
    let mut paint = tiny_skia::Paint::default();
    match stroke.paint() {
        usvg::Paint::Color(c) => {
            paint.set_color_rgba8(c.red, c.green, c.blue, stroke.opacity().to_u8());
        }
        usvg::Paint::LinearGradient(lg) => {
            paint.shader = convert_linear_gradient(lg, stroke.opacity())?;
        }
        usvg::Paint::RadialGradient(rg) => {
            paint.shader = convert_radial_gradient(rg, stroke.opacity())?;
        }
        usvg::Paint::Pattern(pattern) => {
            let (patt_pix, patt_ts) = render_pattern_pixmap(pattern, ctx, transform)?;

            pattern_pixmap = patt_pix;
            paint.shader = tiny_skia::Pattern::new(
                pattern_pixmap.as_ref(),
                tiny_skia::SpreadMode::Repeat,
                tiny_skia::FilterQuality::Bicubic,
                stroke.opacity().get(),
                patt_ts,
            );
        }
    }
    paint.anti_alias = path.rendering_mode().use_shape_antialiasing();
    paint.blend_mode = blend_mode;

    let mut geometry = stroke.to_tiny_skia();
    // OFD device hairlines survive the SVG parser as tagged positive-width paths.
    // tiny-skia width zero selects a one-device-pixel stroke after transformation.
    if path.id().starts_with("ofd-device-hairline-") {
        geometry.width = 0.0;
    }
    pixmap.stroke_path(path.data(), &paint, &geometry, transform, None);

    Some(())
}

fn convert_linear_gradient(
    gradient: &usvg::LinearGradient,
    opacity: usvg::Opacity,
) -> Option<tiny_skia::Shader<'_>> {
    let (mode, points) = convert_base_gradient(gradient, opacity)?;

    let shader = tiny_skia::LinearGradient::new(
        (gradient.x1(), gradient.y1()).into(),
        (gradient.x2(), gradient.y2()).into(),
        points,
        mode,
        gradient.transform(),
    )?;

    Some(shader)
}

fn convert_radial_gradient(
    gradient: &usvg::RadialGradient,
    opacity: usvg::Opacity,
) -> Option<tiny_skia::Shader<'_>> {
    let (mode, points) = convert_base_gradient(gradient, opacity)?;

    let shader = tiny_skia::RadialGradient::new(
        (gradient.fx(), gradient.fy()).into(),
        gradient.fr().get(),
        (gradient.cx(), gradient.cy()).into(),
        gradient.r().get(),
        points,
        mode,
        gradient.transform(),
    )?;

    Some(shader)
}

fn convert_base_gradient(
    gradient: &usvg::BaseGradient,
    opacity: usvg::Opacity,
) -> Option<(tiny_skia::SpreadMode, Vec<tiny_skia::GradientStop>)> {
    let mode = match gradient.spread_method() {
        usvg::SpreadMethod::Pad => tiny_skia::SpreadMode::Pad,
        usvg::SpreadMethod::Reflect => tiny_skia::SpreadMode::Reflect,
        usvg::SpreadMethod::Repeat => tiny_skia::SpreadMode::Repeat,
    };

    let mut points = Vec::with_capacity(gradient.stops().len());
    for stop in gradient.stops() {
        let alpha = stop.opacity() * opacity;
        let color = tiny_skia::Color::from_rgba8(
            stop.color().red,
            stop.color().green,
            stop.color().blue,
            alpha.to_u8(),
        );
        points.push(tiny_skia::GradientStop::new(stop.offset().get(), color));
    }

    Some((mode, points))
}

fn render_pattern_pixmap(
    pattern: &usvg::Pattern,
    _ctx: &Context,
    transform: tiny_skia::Transform,
) -> Option<(tiny_skia::Pixmap, tiny_skia::Transform)> {
    let (sx, sy) = {
        let ts2 = transform.pre_concat(pattern.transform());
        ts2.get_scale()
    };

    let rect = pattern.rect();
    let img_size = tiny_skia::IntSize::from_wh(
        (rect.width() * sx).round() as u32,
        (rect.height() * sy).round() as u32,
    )?;
    let mut pixmap = tiny_skia::Pixmap::new(img_size.width(), img_size.height())?;

    let transform = tiny_skia::Transform::from_scale(sx, sy);
    // A tile is a fresh surface; its budget is independent of the parent layer origin.
    let tile_ctx = Context {
        max_bbox: tiny_skia::IntRect::from_xywh(
            i32::try_from(img_size.width()).ok()?.checked_mul(-2)?,
            i32::try_from(img_size.height()).ok()?.checked_mul(-2)?,
            img_size.width().checked_mul(5)?,
            img_size.height().checked_mul(5)?,
        )?,
    };
    crate::render::render_nodes(pattern.root(), &tile_ctx, transform, &mut pixmap.as_mut());

    let mut ts = tiny_skia::Transform::default();
    ts = ts.pre_concat(pattern.transform());
    ts = ts.pre_translate(rect.x(), rect.y());
    ts = ts.pre_scale(1.0 / sx, 1.0 / sy);

    Some((pixmap, ts))
}

#[cfg(test)]
mod device_hairline_tests {
    #[test]
    fn marked_paths_use_device_pixels_and_ordinary_strokes_keep_transform_scaling() {
        for (id, expected) in [("ofd-device-hairline-0", 255i32), ("ordinary", 510i32)] {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><path id="{id}" d="M1 4H9" transform="scale(4 2)" stroke="black" stroke-width="1" fill="none"/></svg>"#
            );
            let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap();
            let mut pixmap = tiny_skia::Pixmap::new(40, 20).unwrap();
            crate::render(
                &tree,
                tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            let coverage: i32 = (0..20)
                .map(|y| i32::from(pixmap.pixel(20, y).unwrap().alpha()))
                .sum();
            assert!(
                (coverage - expected).abs() <= 3,
                "{id}: coverage {coverage}, expected {expected}"
            );
        }
    }
}
