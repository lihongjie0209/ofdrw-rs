//! Image-backed Canvas patterns and document resource creation.
use crate::canvas_text::canvas_xml_number;
use crate::{
    CanvasUnits, OfdDocument, OfdError, PatternCell, PatternReflectMethod, PatternRelativeTo,
    PatternStyle,
};
use std::{borrow::Cow, io::Cursor};

/// An image-backed Canvas pattern, compatible with Java `CanvasPattern`.
///
/// Create it with [`OfdDocument::create_canvas_pattern`], optionally resize or
/// transform the cell, then pass its style to `DrawParamStyle.fill_pattern` or
/// `stroke_pattern`. Mutating the pattern does not alter an already drawn object.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasPattern {
    style: PatternStyle,
}

fn image_dimension(value: f64) -> Result<f64, OfdError> {
    if !value.is_finite() || value <= 0. {
        return Err(OfdError::Invalid(
            "canvas pattern image size must be positive and finite".into(),
        ));
    }
    let formatted = canvas_xml_number(value)?;
    if formatted <= 0. {
        return Err(OfdError::Invalid(
            "canvas pattern image size rounds to zero".into(),
        ));
    }
    Ok(formatted)
}

impl CanvasPattern {
    /// Borrow the repeated-image style for use in a drawing parameter.
    pub fn as_pattern_style(&self) -> &PatternStyle {
        &self.style
    }

    /// Consume the helper and return the repeated-image style.
    pub fn into_pattern_style(self) -> PatternStyle {
        self.style
    }

    /// Set the pattern cell size in millimeters, preserving reflection and CTM.
    /// Invalid geometry leaves the pattern unchanged.
    pub fn set_image_size(&mut self, width: f64, height: f64) -> Result<&mut Self, OfdError> {
        let mut style = self.style.clone();
        style.width = image_dimension(width)?;
        style.height = image_dimension(height)?;
        for cell in &mut style.cells {
            if let PatternCell::TransformedImage { boundary, ctm, .. } = cell {
                *boundary = [0., 0., style.width, style.height];
                *ctm = [width, 0., 0., height, 0., 0.];
            }
        }
        style.validate()?;
        self.style = style;
        Ok(self)
    }

    /// Set the finite `[a,b,c,d,e,f]` pattern transform, preserving image size.
    /// Invalid matrices leave the pattern unchanged.
    pub fn set_transform(&mut self, matrix: [f64; 6]) -> Result<&mut Self, OfdError> {
        let mut style = self.style.clone();
        style.ctm = Some(matrix);
        style.validate()?;
        self.style = style;
        Ok(self)
    }
}

impl OfdDocument {
    /// Create an image-backed pattern in the owning document of an existing page.
    ///
    /// Natural cell size is pixel size divided by `pixels_per_mm` (Java defaults
    /// to 3.78), then serialized with Java's default STBase numeric precision. The image CTM
    /// retains the unrounded pixel ratio. Repetition accepts `repeat`, empty, `row`, `column` and
    /// `row-column`, case-insensitively. Rows/columns reflect alternating cells;
    /// `repeat-x`, `repeat-y`, `no-repeat` and `normal` are not Java aliases.
    /// All input and cell geometry are validated before adding the image resource.
    /// No page object is painted. Errors leave document bytes unchanged.
    ///
    /// ```
    /// use ofdrw::{CanvasCommand, DrawParamStyle, OfdDocument};
    /// # let image = include_bytes!("../tests/fixtures/canvas-images-java/source.png");
    /// let mut doc = OfdDocument::new();
    /// doc.add_blank_page(40., 30.)?;
    /// let mut pattern = doc.create_canvas_pattern(0, image, "png", "row-column", 3.78)?;
    /// pattern.set_image_size(4., 3.)?.set_transform([1., 0., 0., 1., 2., 1.])?;
    /// doc.draw_canvas(0, [0., 0., 40., 30.], &[
    ///     CanvasCommand::SetDrawParam { style: Box::new(DrawParamStyle {
    ///         fill_pattern: Some(pattern.into_pattern_style()), ..Default::default()
    ///     })},
    ///     CanvasCommand::FillRect { rect: [2., 2., 30., 20.] },
    /// ])?;
    /// # Ok::<(), ofdrw::OfdError>(())
    /// ```
    pub fn create_canvas_pattern(
        &mut self,
        page_index: usize,
        data: &[u8],
        format: &str,
        repetition: &str,
        pixels_per_mm: f64,
    ) -> Result<CanvasPattern, OfdError> {
        let units = CanvasUnits::new(pixels_per_mm)?;
        if repetition.len() > 10 {
            return Err(OfdError::Invalid(
                "unsupported canvas pattern repetition".into(),
            ));
        }
        let reflect = match repetition.to_ascii_lowercase().as_str() {
            "" | "repeat" => PatternReflectMethod::Normal,
            "row" => PatternReflectMethod::Row,
            "column" => PatternReflectMethod::Column,
            "row-column" => PatternReflectMethod::RowAndColumn,
            _ => {
                return Err(OfdError::Invalid(
                    "unsupported canvas pattern repetition".into(),
                ))
            }
        };
        let (document_index, _) = self.page_location(page_index)?;
        self.ensure_unsigned_document(document_index)?;
        if data.is_empty() || data.len() as u64 > crate::MAX_ENTRY_BYTES {
            return Err(OfdError::Invalid(
                "pattern image is empty or too large".into(),
            ));
        }
        let (canonical, _) = crate::checked_image_format(data, format)?;
        let raster = match canonical {
            "GBIG2" => Cow::Owned(crate::jbig2_to_png(data)?),
            "JPEG2000" => Cow::Owned(crate::jpeg2000_to_png(data)?),
            _ => Cow::Borrowed(data),
        };
        let (pixels_wide, pixels_high) = image::ImageReader::new(Cursor::new(raster.as_ref()))
            .with_guessed_format()
            .map_err(|e| OfdError::Invalid(format!("cannot identify pattern image: {e}")))?
            .into_dimensions()
            .map_err(|e| OfdError::Invalid(format!("cannot read pattern image dimensions: {e}")))?;
        let width = f64::from(pixels_wide) / units.pixels_per_mm();
        let height = f64::from(pixels_high) / units.pixels_per_mm();
        let cell_width = image_dimension(width)?;
        let cell_height = image_dimension(height)?;
        let mut style = PatternStyle {
            width: cell_width,
            height: cell_height,
            x_step: None,
            y_step: None,
            reflect,
            relative_to: PatternRelativeTo::Object,
            ctm: None,
            alpha: None,
            thumbnail: None,
            cells: vec![PatternCell::TransformedImage {
                resource_id: String::new(),
                boundary: [0., 0., cell_width, cell_height],
                ctm: [width, 0., 0., height, 0., 0.],
            }],
        };
        // Validate overflow (including doubled reflected tiles) before editing resources.
        style.validate()?;
        let id = self.add_multimedia_resource(document_index, "Image", canonical, data)?;
        if let Some(PatternCell::TransformedImage { resource_id, .. }) = style.cells.first_mut() {
            *resource_id = id;
        }
        Ok(CanvasPattern { style })
    }
}
