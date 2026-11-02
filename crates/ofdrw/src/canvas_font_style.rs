//! Canvas font shorthand and document resource selection.
use crate::{CanvasFont, OfdDocument, OfdError};

/// Parsed OFDRW font shorthand. Absent style fields retain the previous setting.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasFontStyle {
    pub family: String,
    pub size: f64,
    pub weight: Option<u16>,
    pub italic: Option<bool>,
}

/// Parse `[style] [weight] size family` using OFDRW millimeter/px units.
///
/// Pixel sizes use the caller's positive pixels-per-millimeter value (Java's
/// default is 3.78). Unitless sizes are millimeters. Families may contain spaces
/// and may be quoted. CSS family lists, escapes and line heights are unsupported.
/// Without a prefix, weight and italic retain the previous settings. A
/// style-only prefix resets weight to 400, while a weight-only prefix retains
/// italic, matching Java's state updates.
/// Invalid or oversized inputs return errors without changing a document.
///
/// ```
/// use ofdrw::parse_canvas_font_style;
/// let style = parse_canvas_font_style("italic bold 12px Times New Roman", 4.)?;
/// assert_eq!(style.family, "Times New Roman");
/// assert_eq!(style.size, 3.);
/// assert_eq!(style.weight, Some(700));
/// # Ok::<(), ofdrw::OfdError>(())
/// ```
pub fn parse_canvas_font_style(
    input: &str,
    pixels_per_mm: f64,
) -> Result<CanvasFontStyle, OfdError> {
    if input.len() > 4096 {
        return Err(OfdError::Limit(
            "canvas font style exceeds 4096 bytes".into(),
        ));
    }
    if !pixels_per_mm.is_finite() || pixels_per_mm <= 0. {
        return Err(invalid("pixels per millimeter must be finite and positive"));
    }
    let input = input.trim();
    let mut spans = Vec::new();
    let mut start = None;
    for (index, ch) in input.char_indices() {
        if ch.is_whitespace() {
            if let Some(start) = start.take() {
                spans.push((start, index));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(start) = start {
        spans.push((start, input.len()));
    }
    if spans.len() < 2 {
        return Err(invalid("font size and family are required"));
    }
    // A numeric weight is also a possible unitless size. Prefer a complete
    // recognized prefix, falling back to a size when no later size exists.
    for index in (0..spans.len().saturating_sub(1).min(4)).rev() {
        let token = &input[spans[index].0..spans[index].1];
        let Some(size) = size(token, pixels_per_mm) else {
            continue;
        };
        let prefix: Vec<_> = spans[..index]
            .iter()
            .map(|&(start, end)| &input[start..end])
            .collect();
        let Some((weight, italic)) = prefix_style(&prefix) else {
            continue;
        };
        let family = input[spans[index + 1].0..].trim();
        let mut quoted = false;
        let family = if let Some(quote) = family.chars().next().filter(|c| matches!(c, '\'' | '"'))
        {
            quoted = true;
            family
                .strip_prefix(quote)
                .and_then(|value| value.strip_suffix(quote))
                .filter(|value| !value.contains(quote))
                .ok_or_else(|| invalid("unbalanced family quotes"))?
        } else {
            if family.contains(['\'', '"']) {
                return Err(invalid("unbalanced family quotes"));
            }
            family
        };
        if family.trim().is_empty()
            || family
                .chars()
                .any(|c| c.is_control() || !crate::is_xml_char(c))
            || family.contains('\\')
            || (!quoted && !unquoted_family(family))
        {
            return Err(invalid("invalid or unsupported font family"));
        }
        CanvasFont {
            size,
            ..Default::default()
        }
        .validate()?;
        return Ok(CanvasFontStyle {
            family: family.to_owned(),
            size,
            weight,
            italic,
        });
    }
    Err(invalid("invalid font style or size"))
}

fn unquoted_family(family: &str) -> bool {
    family.split_whitespace().all(|word| {
        let start = word
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_' || ch == '-' || !ch.is_ascii());
        start
            && word
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || !ch.is_ascii())
    })
}

fn invalid(message: &str) -> OfdError {
    OfdError::Invalid(format!("canvas font style: {message}"))
}

fn size(token: &str, pixels_per_mm: f64) -> Option<f64> {
    let (number, divisor) = if let Some(number) = token.strip_suffix("px") {
        (number, pixels_per_mm)
    } else {
        (token.strip_suffix("mm").unwrap_or(token), 1.)
    };
    let value = number.parse::<f64>().ok()? / divisor;
    (value.is_finite() && value > 0.).then_some(value)
}

fn prefix_style(tokens: &[&str]) -> Option<(Option<u16>, Option<bool>)> {
    if tokens.is_empty() {
        return Some((None, None));
    }
    let mut weight = None;
    let mut italic = None;
    let mut normal = 0;
    for token in tokens {
        let normalized = token.to_ascii_lowercase();
        match normalized.as_str() {
            "normal" => normal += 1,
            "italic" if italic.is_none() => italic = Some(true),
            token if weight.is_none() => {
                weight = Some(match token {
                    "lighter" | "100" => 100,
                    "200" => 200,
                    "300" => 300,
                    "400" => 400,
                    "500" => 500,
                    "600" => 600,
                    "bold" | "700" => 700,
                    "800" => 800,
                    "bolder" | "900" => 900,
                    _ => return None,
                })
            }
            _ => return None,
        }
    }
    match normal {
        0 => (),
        1 if weight.is_none() => weight = Some(400),
        1 if italic.is_none() => italic = Some(false),
        2 if weight.is_none() && italic.is_none() => {
            weight = Some(400);
            italic = Some(false);
        }
        _ => return None,
    }
    Some((Some(weight.unwrap_or(400)), italic))
}

impl OfdDocument {
    /// Resolve font shorthand against a page's fonts, creating a named font
    /// resource when absent. Existing named/embedded resources are reused.
    /// Tracking, alignment and directions are retained from `base`. This method
    /// may add a resource; subsequent Canvas measurement remains read-only.
    ///
    /// ```
    /// use ofdrw::{CanvasFont, OfdDocument};
    /// let mut doc = OfdDocument::new();
    /// doc.add_blank_page(40., 30.)?;
    /// let font = doc.resolve_canvas_font_style(0, "bold 12px Arial", &CanvasFont::default(), 3.78)?;
    /// assert_eq!(font.weight, 700);
    /// assert!(font.font_id.is_some());
    /// # Ok::<(), ofdrw::OfdError>(())
    /// ```
    pub fn resolve_canvas_font_style(
        &mut self,
        page_index: usize,
        input: &str,
        base: &CanvasFont,
        pixels_per_mm: f64,
    ) -> Result<CanvasFont, OfdError> {
        base.validate()?;
        let parsed = parse_canvas_font_style(input, pixels_per_mm)?;
        let (document_index, page) = self.page_location(page_index)?;
        let fonts = self.fonts_for_page(&page, document_index)?;
        let exact: Vec<_> = fonts
            .iter()
            .filter(|font| font.font_name.eq_ignore_ascii_case(&parsed.family))
            .collect();
        let matches = if exact.is_empty() {
            fonts
                .iter()
                .filter(|font| {
                    font.family_name
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(&parsed.family))
                })
                .collect()
        } else {
            exact
        };
        if matches.len() > 1 {
            return Err(invalid("font family is ambiguous on this page"));
        }
        let mut result = base.clone();
        result.size = parsed.size;
        result.weight = parsed.weight.unwrap_or(base.weight);
        result.italic = parsed.italic.unwrap_or(base.italic);
        result.validate()?;
        result.font_id = Some(if let Some(font) = matches.first() {
            font.id.clone()
        } else {
            self.add_canvas_named_font(document_index, &parsed.family)?
        });
        Ok(result)
    }
}
