//! PDF Pattern color selection shared by tiling and shading patterns.

use super::*;

pub(super) enum Source {
    Tiling(tiling_pattern::Source),
    Shading(shading_pattern::Source),
}

fn kind(pdf: &PdfDocument, value: &Object) -> Result<i64, OfdError> {
    let dict = match value {
        Object::Dictionary(dict) => dict,
        Object::Stream(stream) => &stream.dict,
        _ => {
            return Err(OfdError::Invalid(
                "PDF Pattern is not a dictionary or stream".into(),
            ))
        }
    };
    pdf_resolved_value(
        Some(pdf),
        dict.get(b"PatternType")
            .map_err(|_| OfdError::Invalid("PDF Pattern has no PatternType".into()))?,
        "PatternType",
    )?
    .as_i64()
    .map_err(|_| OfdError::Invalid("PDF PatternType is not an integer".into()))
}

impl Source {
    pub(super) fn resolve(
        page: &PdfPageContext<'_>,
        name: &[u8],
        parent: Arc<GraphicsState>,
        uncolored: bool,
    ) -> Result<Self, OfdError> {
        let (_, value) = resource(page.pdf, page.resources, b"Pattern", name)?;
        match kind(page.pdf, value)? {
            1 => Ok(Self::Tiling(tiling_pattern::Source::resolve(
                page, name, parent, uncolored,
            )?)),
            2 if !uncolored => Ok(Self::Shading(shading_pattern::Source::resolve(
                page, value, parent,
            )?)),
            2 => Err(OfdError::Invalid(
                "PDF shading Pattern cannot have an underlying color space".into(),
            )),
            _ => Err(OfdError::Invalid("unsupported PDF PatternType".into())),
        }
    }

    pub(super) fn style(
        &self,
        output: &mut OfdDocument,
        page: &PdfPageContext<'_>,
        state: &GraphicsState,
        rgb: [u8; 3],
        alpha: f64,
    ) -> Result<crate::PatternStyle, OfdError> {
        match self {
            Self::Tiling(source) => source.style(output, page, state, rgb, alpha),
            Self::Shading(source) => source.style(output, page, state, alpha),
        }
    }
}

pub(super) fn validate_opaque(
    pdf: &PdfDocument,
    resources: &Object,
    name: &[u8],
    active: &mut Vec<ObjectId>,
) -> Result<(), OfdError> {
    let (_, value) = resource(pdf, resources, b"Pattern", name)?;
    match kind(pdf, value)? {
        1 => tiling_pattern::validate_opaque(pdf, resources, name, active),
        2 => {
            let page = PdfPageContext {
                pdf,
                resources,
                index: 0,
                box_: [0., 0., 1., 1.],
            };
            let source =
                shading_pattern::Source::resolve(&page, value, Arc::new(GraphicsState::default()))?;
            source.require_opaque(pdf)
        }
        _ => Err(OfdError::Invalid("unsupported PDF PatternType".into())),
    }
}
