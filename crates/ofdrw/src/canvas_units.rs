use crate::OfdError;

/// Pixel/millimeter conversions compatible with Java `DrawContext`.
///
/// Default density is 3.78 pixels per millimeter. Conversion to pixels truncates
/// toward zero and saturates to the signed 32-bit range, matching Java's cast.
/// This helper does not change page geometry or the drawing transformation.
///
/// ```
/// use ofdrw::CanvasUnits;
/// let units = CanvasUnits::new(4.)?;
/// assert_eq!(units.mm(12), 3.);
/// assert_eq!(units.pixel(-0.49), -1);
/// # Ok::<(), ofdrw::OfdError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasUnits {
    pixels_per_mm: f64,
}

impl Default for CanvasUnits {
    fn default() -> Self {
        Self {
            pixels_per_mm: 3.78,
        }
    }
}

impl CanvasUnits {
    /// Create conversions with a finite, positive pixel density.
    pub fn new(pixels_per_mm: f64) -> Result<Self, OfdError> {
        if !pixels_per_mm.is_finite() || pixels_per_mm <= 0. {
            return Err(OfdError::Invalid(
                "pixels per millimeter must be finite and positive".into(),
            ));
        }
        Ok(Self { pixels_per_mm })
    }

    /// Return the configured pixels per millimeter.
    pub fn pixels_per_mm(self) -> f64 {
        self.pixels_per_mm
    }

    /// Convert signed integer pixels to millimeters using floating-point division.
    /// An extremely small density can produce an infinite result, as in Java.
    pub fn mm(self, pixel: i32) -> f64 {
        f64::from(pixel) / self.pixels_per_mm
    }

    /// Convert millimeters to signed integer pixels, truncating toward zero.
    /// Out-of-range products and infinities saturate; NaN becomes zero, as in Java.
    pub fn pixel(self, mm: f64) -> i32 {
        (mm * self.pixels_per_mm) as i32
    }
}
