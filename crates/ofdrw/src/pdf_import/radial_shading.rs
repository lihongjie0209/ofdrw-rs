//! General PDF two-circle fields that cannot use OFD's nested radial primitive.
use super::*;

pub(super) fn needs_general(coords: &[f64]) -> bool {
    coords[2] == coords[5]
        || (coords[3] - coords[0]).hypot(coords[4] - coords[1]) + coords[2].min(coords[5])
            >= coords[2].max(coords[5])
}

#[cfg(test)]
use crate::two_circle::MAX_PIXELS;
use crate::two_circle::{dimensions, Geometry};

pub(super) fn import(
    output: &mut OfdDocument,
    page: &PdfPageContext<'_>,
    state: &GraphicsState,
    dict: &lopdf::Dictionary,
    coords: &[f64],
    visible: bool,
) -> Result<(), OfdError> {
    let extend = match dict.get(b"Extend") {
        Ok(value) => {
            let values = pdf_resolved_array(page.pdf, value, "radial Extend", 2)?;
            [
                values[0]
                    .as_bool()
                    .map_err(|_| OfdError::Invalid("invalid PDF radial Extend".into()))?,
                values[1]
                    .as_bool()
                    .map_err(|_| OfdError::Invalid("invalid PDF radial Extend".into()))?,
            ]
        }
        Err(_) => [false, false],
    };
    let geometry = Geometry::new(coords, extend)?;
    let domain = match dict.get(b"Domain") {
        Ok(value) => {
            let v = pdf_numeric_array(page.pdf, value, "radial Domain", 2)?;
            [v[0], v[1]]
        }
        Err(_) => [0., 1.],
    };
    if domain[0] >= domain[1] {
        return Err(OfdError::Invalid("invalid PDF radial Domain".into()));
    }
    let space = parse_color_space(
        Some(page.pdf),
        Some(page.resources),
        dict.get(b"ColorSpace")
            .map_err(|_| OfdError::Invalid("PDF radial shading has no ColorSpace".into()))?,
        0,
    )?;
    if matches!(
        space,
        PdfColorSpace::Pattern { .. } | PdfColorSpace::Indexed { .. }
    ) {
        return Err(OfdError::Invalid(
            "PDF radial shading cannot use Pattern or Indexed color".into(),
        ));
    }
    if space.nonpainting() {
        return Ok(());
    }
    let segments = pdf_shading_segments(
        page.pdf,
        dict.get(b"Function")
            .map_err(|_| OfdError::Invalid("PDF radial shading has no Function".into()))?,
        &space,
        state.rendering_intent,
        domain,
    )?;
    if !visible || (coords[2] == 0. && coords[5] == 0.) {
        return Ok(());
    }
    let inverse = crate::inverse_matrix(state.ctm)
        .ok_or_else(|| OfdError::Invalid("singular PDF radial transform".into()))?;
    let [a, b, c, d, e, f] = inverse;
    let width = (page.box_[2] - page.box_[0]) * MM_PER_PT;
    let height = (page.box_[3] - page.box_[1]) * MM_PER_PT;
    let [columns, rows] = dimensions(width, height, page.raster_scale)?;
    let mut data = Vec::with_capacity(columns as usize * rows as usize * 4);
    let mut painted = false;
    for row in 0..rows {
        let y =
            page.box_[3] - (f64::from(row) + 0.5) / f64::from(rows) * (page.box_[3] - page.box_[1]);
        for column in 0..columns {
            let x = page.box_[0]
                + (f64::from(column) + 0.5) / f64::from(columns) * (page.box_[2] - page.box_[0]);
            if let Some(s) = geometry.parameter(a * x + c * y + e, b * x + d * y + f) {
                data.extend_from_slice(&separation_color::color(&segments, s));
                data.push(255);
                painted = true;
            } else {
                data.extend_from_slice(&[0; 4]);
            }
        }
    }
    if !painted {
        return Ok(());
    }
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, columns, rows);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&data))
            .map_err(|error| {
                OfdError::Invalid(format!("cannot encode PDF radial shading: {error}"))
            })?;
    }
    let id = output.add_image_object(page.index, &png, "png", [0., 0., width, height])?;
    attach_nonstroking_alpha(output, page.index, &id, "ImageObject", state.fill_alpha)?;
    attach_blend_mode(output, page.index, &id, state.blend_mode)?;
    attach_clips(output, page.index, &id, "ImageObject", &state.clips)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_radii_choose_largest_valid_root() {
        let geometry = Geometry::new(&[0., 0., 1., 4., 0., 1.], [false; 2]).unwrap();
        assert_eq!(geometry.parameter(2., 0.), Some(0.75));
        // The larger root is outside the unextended domain; use the other root.
        assert_eq!(geometry.parameter(4., 0.), Some(0.75));
    }

    #[test]
    fn extension_uses_endpoint_color() {
        let geometry = Geometry::new(&[0., 0., 1., 4., 0., 1.], [true; 2]).unwrap();
        assert_eq!(geometry.parameter(4., 0.), Some(1.));
        assert_eq!(geometry.parameter(-2., 0.), Some(0.));
    }

    #[test]
    fn extension_rejects_negative_radius() {
        let geometry = Geometry::new(&[0., 0., 0., 4., 0., 1.], [true; 2]).unwrap();
        assert_eq!(geometry.parameter(-1., 0.), None);
    }

    #[test]
    fn tangent_circles_use_linear_limit() {
        let geometry = Geometry::new(&[0., 0., 0., 2., 0., 2.], [false; 2]).unwrap();
        assert_eq!(geometry.parameter(1., 1.), Some(0.5));
        assert_eq!(geometry.parameter(-1., 1.), None);
        assert_eq!(geometry.parameter(0., 0.), Some(1.));
    }

    #[test]
    fn points_outside_circle_sweep_are_unpainted() {
        let geometry = Geometry::new(&[0., 0., 1., 4., 0., 1.], [true; 2]).unwrap();
        assert_eq!(geometry.parameter(2., 2.), None);
        assert_eq!(geometry.parameter(2., 1.), Some(0.5));
    }

    #[test]
    fn near_linear_quadratic_preserves_small_root() {
        let geometry = Geometry::new(&[0., 0., 1., 1.000001, 0., 2.], [false; 2]).unwrap();
        let x = 1. + 1e-10;
        let expected = (x - 1.) / 2.000001;
        let actual = geometry.parameter(x, 0.).unwrap();
        assert!(
            (actual - expected).abs() < expected * 1e-8,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn invalid_geometry_returns_errors() {
        for coords in [
            vec![0.; 5],
            vec![0., 0., -1., 4., 0., 1.],
            vec![f64::NAN, 0., 1., 4., 0., 1.],
            vec![1e10, 0., 1., 4., 0., 1.],
            vec![0., 0., 1., 0., 0., 1.],
        ] {
            assert!(Geometry::new(&coords, [false; 2]).is_err(), "{coords:?}");
        }
    }

    #[test]
    fn raster_dimensions_follow_outer_magnification() {
        assert_eq!(dimensions(10., 5., 1.).unwrap(), [160, 80]);
        assert_eq!(dimensions(10., 5., 2.).unwrap(), [320, 160]);
    }

    #[test]
    fn raster_dimensions_cap_extreme_values_without_overflow() {
        for (width, height, scale) in [
            (10., 5., f64::INFINITY),
            (1e300, 1e300, 1.),
            (1e300, 1e-300, 1.),
            (1e-300, 1e300, 1.),
            (1e-300, 1e-300, f64::INFINITY),
        ] {
            let [x, y] = dimensions(width, height, scale).unwrap();
            assert!(x > 0 && y > 0 && u64::from(x) * u64::from(y) <= MAX_PIXELS as u64);
            assert!(x > 1 || y > 1);
        }
    }

    #[test]
    fn invalid_raster_dimensions_return_errors() {
        for (width, height, scale) in [
            (0., 1., 1.),
            (1., -1., 1.),
            (f64::INFINITY, 1., 1.),
            (1., 1., 0.),
            (1., 1., f64::NAN),
        ] {
            assert!(dimensions(width, height, scale).is_err());
        }
    }
}
