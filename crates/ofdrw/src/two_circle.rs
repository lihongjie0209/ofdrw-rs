//! Stable two-circle parameter solver shared by PDF import and OFD paints.
use crate::OfdError;

pub(crate) struct Geometry {
    origin: [f64; 2],
    delta: [f64; 2],
    radius: f64,
    dr: f64,
    a: f64,
    extend: [bool; 2],
}

impl Geometry {
    pub(crate) fn new(coords: &[f64], extend: [bool; 2]) -> Result<Self, OfdError> {
        if coords.len() != 6
            || coords.iter().any(|v| !v.is_finite() || v.abs() > 1e9)
            || coords[2] < 0.
            || coords[5] < 0.
        {
            return Err(OfdError::Invalid(
                "invalid PDF radial shading circles".into(),
            ));
        }
        if coords[2] > 0.
            && coords[0] == coords[3]
            && coords[1] == coords[4]
            && coords[2] == coords[5]
        {
            return Err(OfdError::Invalid(
                "coincident nonzero PDF radial circles are degenerate".into(),
            ));
        }
        let delta = [coords[3] - coords[0], coords[4] - coords[1]];
        let dr = coords[5] - coords[2];
        Ok(Self {
            origin: [coords[0], coords[1]],
            delta,
            radius: coords[2],
            dr,
            a: delta[0] * delta[0] + delta[1] * delta[1] - dr * dr,
            extend,
        })
    }

    fn accept(&self, s: f64) -> Option<f64> {
        if !s.is_finite() || (s < 0. && !self.extend[0]) || (s > 1. && !self.extend[1]) {
            return None;
        }
        let radius = self.dr.mul_add(s, self.radius);
        let tolerance = 32. * f64::EPSILON * self.radius.max((s * self.dr).abs());
        (radius >= -tolerance).then_some(s)
    }

    pub(crate) fn parameter(&self, x: f64, y: f64) -> Option<f64> {
        self.parameter_unclamped(x, y)
            .map(|value| value.clamp(0., 1.))
    }

    pub(crate) fn parameter_unclamped(&self, x: f64, y: f64) -> Option<f64> {
        let [x, y] = [x - self.origin[0], y - self.origin[1]];
        let b = -2. * (x * self.delta[0] + y * self.delta[1] + self.radius * self.dr);
        let c = x * x + y * y - self.radius * self.radius;
        let largest = self.a.abs().max(b.abs()).max(c.abs());
        if largest == 0. {
            return self.accept(1.);
        }
        let [a, b, c] = [self.a, b, c].map(|v| v / largest);
        if a == 0. {
            return (b != 0.).then(|| -c / b).and_then(|s| self.accept(s));
        }
        let mut discriminant = (-4. * a).mul_add(c, b * b);
        if discriminant < 0. {
            let error = 32. * f64::EPSILON * (b * b + (4. * a * c).abs());
            if discriminant < -error {
                return None;
            }
            discriminant = 0.;
        }
        // The q form avoids catastrophic cancellation at a small root.
        let q = -0.5 * (b + discriminant.sqrt().copysign(b));
        if q == 0. {
            return self.accept(0.);
        }
        [q / a, c / q]
            .into_iter()
            .filter_map(|s| self.accept(s))
            .max_by(f64::total_cmp)
    }
}

pub(crate) const MAX_PIXELS: usize = 4_000_000;
const PIXELS_PER_MM: f64 = 16.;

pub(crate) fn dimensions(width: f64, height: f64, scale: f64) -> Result<[u32; 2], OfdError> {
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.
        || height <= 0.
        || scale.is_nan()
        || scale <= 0.
    {
        return Err(OfdError::Limit(
            "invalid PDF radial raster dimensions".into(),
        ));
    }
    let x = (width * scale * PIXELS_PER_MM).ceil().max(1.);
    let y = (height * scale * PIXELS_PER_MM).ceil().max(1.);
    if x.is_finite() && y.is_finite() && x * y <= MAX_PIXELS as f64 {
        return Ok([x as u32, y as u32]);
    }
    // Compute the capped shape from its aspect ratio. Multiplying the source
    // dimensions first can overflow or underflow before applying the cap.
    let x = ((MAX_PIXELS as f64 * (width / height)).sqrt())
        .floor()
        .clamp(1., MAX_PIXELS as f64);
    let y = (MAX_PIXELS as f64 / x).floor().clamp(1., MAX_PIXELS as f64);
    Ok([x as u32, y as u32])
}
