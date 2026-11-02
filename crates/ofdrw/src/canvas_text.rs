use super::*;

/// Alignment of canvas text along its reading direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CanvasTextAlign {
    #[default]
    Start,
    End,
    Left,
    Right,
    Center,
}

/// Font and positioning settings for unshaped canvas text.
///
/// A missing font ID selects SimSun/宋体, adding a named resource when needed.
/// Sizes and tracking are millimeters; directions are 0, 90, 180 or 270 degrees.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasFont {
    pub font_id: Option<String>,
    pub size: f64,
    pub letter_spacing: f64,
    pub weight: u16,
    pub italic: bool,
    pub read_direction: u16,
    pub char_direction: u16,
    pub align: CanvasTextAlign,
}

impl Default for CanvasFont {
    fn default() -> Self {
        Self {
            font_id: None,
            size: 1.,
            letter_spacing: 0.,
            weight: 400,
            italic: false,
            read_direction: 0,
            char_direction: 0,
            align: CanvasTextAlign::Start,
        }
    }
}

impl CanvasFont {
    pub(super) fn validate(&self) -> Result<(), OfdError> {
        TextTypographyStyle {
            size: self.size,
            h_scale: None,
            read_direction: Some(self.read_direction),
            char_direction: Some(self.char_direction),
            weight: Some(self.weight),
            italic: None,
        }
        .validate()?;
        if !self.letter_spacing.is_finite() {
            return Err(OfdError::Invalid(
                "canvas letter spacing must be finite".into(),
            ));
        }
        Ok(())
    }
}

/// Logical canvas text advances, without kerning or shaping.
///
/// Offsets are signed increments along the reading axis. Alignment is applied
/// by `FillText`, not included in `first_offset`. Supplementary Unicode scalars
/// occupy one advance instead of Java's two UTF-16 surrogate advances.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CanvasTextMetrics {
    pub width: f64,
    pub first_offset: [f64; 2],
    pub offsets: Vec<f64>,
}

impl OfdDocument {
    /// Measure canvas text without allocating resources or changing the document.
    ///
    /// ```
    /// use ofdrw::{CanvasCommand, CanvasFont, OfdDocument};
    /// let mut doc = OfdDocument::new();
    /// doc.add_blank_page(40., 30.)?;
    /// let font = CanvasFont { size: 4., ..Default::default() };
    /// let metrics = doc.measure_canvas_text(0, "Canvas", &font)?;
    /// assert!(metrics.width > 0.);
    /// let ids = doc.draw_canvas(0, [0., 0., 40., 30.], &[
    ///     CanvasCommand::SetFont { font },
    ///     CanvasCommand::FillText { text: "Canvas".into(), point: [2., 8.] },
    /// ])?;
    /// assert_eq!(ids.len(), 1);
    /// # Ok::<(), ofdrw::OfdError>(())
    /// ```
    pub fn measure_canvas_text(
        &self,
        page_index: usize,
        text: &str,
        font: &CanvasFont,
    ) -> Result<CanvasTextMetrics, OfdError> {
        font.validate()?;
        self.page_location(page_index)?;
        if text.len() > 1_000_000 {
            return Err(OfdError::Limit("canvas text exceeds 1 MB".into()));
        }
        if text.is_empty() {
            return Ok(CanvasTextMetrics::default());
        }
        let selected = self.canvas_font(page_index, font)?;
        let widths = canvas_widths(self, &selected, text, font.size)?;
        let first = widths[0];
        let last = widths[widths.len() - 1];
        let rd = font.read_direction;
        let cd = font.char_direction;
        let horizontal_glyph = rd % 180 == cd % 180;
        let reversed_glyph = horizontal_glyph && rd != cd;
        let sign = if rd >= 180 { -1. } else { 1. };
        let offsets: Vec<f64> = (0..widths.len() - 1)
            .map(|i| {
                sign * (if horizontal_glyph {
                    widths[i + usize::from(reversed_glyph)]
                } else {
                    font.size
                } + font.letter_spacing)
            })
            .collect();
        let terminal = if horizontal_glyph {
            if reversed_glyph {
                first
            } else {
                last
            }
        } else {
            font.size
        };
        let width = offsets.iter().map(|v| v.abs()).sum::<f64>() + terminal;
        let size = font.size;
        let first_offset = match (rd, cd) {
            (0, 180) => [first, -size],
            (0, 90) | (180, 180) => [0., -size],
            (0, 270) | (90, 180) | (270, 270) => [size, 0.],
            (180, 0) => [-first, 0.],
            (180, 90) => [-size, -size],
            (90, 0) => [0., size],
            (90, 270) => [size, first],
            (270, 180) => [size, -size],
            (270, 90) => [0., -first],
            _ => [0., 0.],
        };
        if !width.is_finite()
            || offsets
                .iter()
                .chain(first_offset.iter())
                .any(|v| !v.is_finite())
        {
            return Err(OfdError::Invalid("canvas text metrics overflow".into()));
        }
        Ok(CanvasTextMetrics {
            width,
            first_offset,
            offsets,
        })
    }

    fn canvas_font(&self, page_index: usize, style: &CanvasFont) -> Result<FontInfo, OfdError> {
        let (document_index, page) = self.page_location(page_index)?;
        let fonts = self.fonts_for_page(&page, document_index)?;
        let found = if let Some(id) = &style.font_id {
            fonts
                .into_iter()
                .find(|font| resource_id_key(&font.id) == resource_id_key(id))
                .ok_or_else(|| OfdError::Invalid("canvas font is not defined".into()))?
        } else if let Some(font) = fonts.into_iter().find(|font| {
            [Some(font.font_name.as_str()), font.family_name.as_deref()]
                .into_iter()
                .flatten()
                .any(|name| name.eq_ignore_ascii_case("SimSun") || name == "宋体")
        }) {
            font
        } else {
            FontInfo {
                document_index,
                id: String::new(),
                font_name: "SimSun".into(),
                family_name: Some("SimSun".into()),
                resource_path: String::new(),
                embedded_path: None,
                attributes: BTreeMap::new(),
            }
        };
        Ok(found)
    }

    pub(super) fn add_canvas_text(
        &mut self,
        page_index: usize,
        boundary: [f64; 4],
        text: &str,
        point: [f64; 2],
        style: &CanvasFont,
    ) -> Result<String, OfdError> {
        let metrics = self.measure_canvas_text(page_index, text, style)?;
        let mut font = self.canvas_font(page_index, style)?;
        if font.id.is_empty() {
            font.id = self.add_canvas_default_font(font.document_index)?;
        }
        let mut origin = [
            point[0] + metrics.first_offset[0],
            point[1] + metrics.first_offset[1],
        ];
        let factor = match style.align {
            CanvasTextAlign::Start | CanvasTextAlign::Left => 0.,
            CanvasTextAlign::End | CanvasTextAlign::Right => -metrics.width,
            CanvasTextAlign::Center => -metrics.width / 2.,
        } * if style.read_direction >= 180 { -1. } else { 1. };
        let vertical = style.read_direction % 180 == 90;
        origin[usize::from(vertical)] += factor;
        if origin.iter().any(|v| !v.is_finite()) {
            return Err(OfdError::Invalid(
                "canvas text origin must be finite".into(),
            ));
        }
        origin = [canvas_xml_number(origin[0])?, canvas_xml_number(origin[1])?];
        let offsets = metrics
            .offsets
            .iter()
            .map(|value| canvas_xml_number(*value))
            .collect::<Result<Vec<_>, _>>()?;
        let serialized_size = canvas_xml_number(style.size)?;
        let id = self.add_text_object_with_delta_x(
            page_index,
            text,
            boundary,
            serialized_size,
            Some(&font.id),
            Some(&offsets),
        )?;
        self.set_page_text_typography(
            page_index,
            &id,
            TextTypographyStyle {
                size: serialized_size,
                h_scale: None,
                read_direction: (style.read_direction != 0).then_some(style.read_direction),
                char_direction: (style.char_direction != 0).then_some(style.char_direction),
                weight: (style.weight != 400).then_some(style.weight),
                italic: style.italic.then_some(true),
            },
        )?;
        self.set_page_text_code_layout(
            page_index,
            &id,
            0,
            &TextCodeLayout {
                x: Some(origin[0]),
                y: Some(origin[1]),
                delta_x: (!vertical && !offsets.is_empty()).then(|| offsets.clone()),
                delta_y: (vertical && !offsets.is_empty()).then_some(offsets),
            },
        )?;
        Ok(id)
    }

    fn add_canvas_default_font(&mut self, document_index: usize) -> Result<String, OfdError> {
        self.ensure_unsigned_document(document_index)?;
        let root = self
            .doc_roots()?
            .into_iter()
            .nth(document_index)
            .ok_or_else(|| OfdError::Invalid("document index is out of bounds".into()))?;
        let bytes = self.required_entry(&root)?;
        let source = std::str::from_utf8(bytes).map_err(|e| OfdError::Invalid(e.to_string()))?;
        let parsed = parse_xml(&root, bytes)?;
        let common = descendant(parsed.root_element(), "CommonData")
            .ok_or_else(|| OfdError::Invalid("document has no CommonData".into()))?;
        let max = descendant(common, "MaxUnitID")
            .ok_or_else(|| OfdError::Invalid("document has no MaxUnitID".into()))?;
        let id = element_text(max)
            .trim()
            .parse::<usize>()
            .map_err(|_| OfdError::Invalid("invalid MaxUnitID".into()))?
            .checked_add(1)
            .ok_or_else(|| OfdError::Limit("font ID overflow".into()))?;
        let paths = self.document_resource_paths(&root, common, "PublicRes")?;
        let path = paths.first().cloned().unwrap_or(resolve_path(
            &parent_path(&root),
            &format!("PublicRes_{id}.xml"),
        )?);
        self.ensure_unsigned_entry(&path)?;
        let resource = if paths.is_empty() {
            format!("<ofd:Res xmlns:ofd=\"{OFD_NS}\"><ofd:Fonts><ofd:Font ID=\"{id}\" FontName=\"SimSun\" FamilyName=\"SimSun\"/></ofd:Fonts></ofd:Res>")
        } else {
            self.ensure_resource_file_not_shared(document_index, &path)?;
            let raw = std::str::from_utf8(self.required_entry(&path)?)
                .map_err(|e| OfdError::Invalid(e.to_string()))?;
            let xml = parse_xml(&path, raw.as_bytes())?;
            if !is_ofd_element(xml.root_element(), "Res") {
                return Err(OfdError::Invalid(
                    "canvas font resource is not a Res file".into(),
                ));
            }
            let fonts = descendant(xml.root_element(), "Fonts");
            let parent = fonts.unwrap_or(xml.root_element());
            let tag = qualified_tag(raw, parent)?;
            let prefix = tag
                .rsplit_once(':')
                .map_or(String::new(), |(p, _)| format!("{p}:"));
            let item =
                format!("<{prefix}Font ID=\"{id}\" FontName=\"SimSun\" FamilyName=\"SimSun\"/>");
            let item = if fonts.is_some() {
                item
            } else {
                format!("<{prefix}Fonts>{item}</{prefix}Fonts>")
            };
            insert_xml_child(raw, parent, &item)?
        };
        let mut document = source.to_owned();
        document.replace_range(element_content_range(max, source)?, &id.to_string());
        if paths.is_empty() {
            if self.entries.contains_key(&path) {
                return Err(OfdError::Invalid(
                    "canvas font resource path already exists".into(),
                ));
            }
            let parsed = parse_xml(&root, document.as_bytes())?;
            let common = descendant(parsed.root_element(), "CommonData")
                .ok_or_else(|| OfdError::Invalid("document has no CommonData".into()))?;
            let tag = qualified_tag(&document, common)?;
            let prefix = tag
                .rsplit_once(':')
                .map_or(String::new(), |(p, _)| format!("{p}:"));
            let position = common
                .children()
                .find(|node| {
                    is_ofd_element(*node, "DocumentRes") || is_ofd_element(*node, "MaxUnitID")
                })
                .ok_or_else(|| {
                    OfdError::Invalid("document has no resource insertion point".into())
                })?
                .range()
                .start;
            let filename = path
                .rsplit('/')
                .next()
                .ok_or_else(|| OfdError::Invalid("invalid resource path".into()))?;
            document.insert_str(
                position,
                &format!("<{prefix}PublicRes>{filename}</{prefix}PublicRes>"),
            );
        }
        let next = id
            .checked_add(1)
            .ok_or_else(|| OfdError::Limit("font ID overflow".into()))?;
        self.apply_entries(vec![
            (path, resource.into_bytes()),
            (root, document.into_bytes()),
        ])?;
        self.next_id = self.next_id.max(next);
        Ok(id.to_string())
    }
}

fn canvas_widths(
    doc: &OfdDocument,
    font: &FontInfo,
    text: &str,
    size: f64,
) -> Result<Vec<f64>, OfdError> {
    let measure = |data: &[u8], index: u32| {
        let face = ttf_parser::Face::parse(data, index)
            .map_err(|_| OfdError::Invalid("cannot measure canvas font".into()))?;
        let units = f64::from(face.units_per_em());
        Ok(text
            .chars()
            .map(|ch| {
                let glyph = face.glyph_index(ch).unwrap_or(ttf_parser::GlyphId(0));
                f64::from(face.glyph_hor_advance(glyph).unwrap_or(face.units_per_em())) / units
                    * size
            })
            .collect())
    };
    if let Some(path) = &font.embedded_path {
        return measure(doc.required_entry(path)?, 0);
    }
    use resvg::usvg::fontdb::{Family, Query};
    let database = system_fonts();
    if let Some(id) = database.query(&Query {
        families: &[Family::Name(&font.font_name), Family::Serif],
        ..Query::default()
    }) {
        if let Some(result) = database.with_face_data(id, measure) {
            return result;
        }
    }
    Ok(text
        .chars()
        .map(|ch| if ch.is_ascii() { size * 0.5 } else { size })
        .collect())
}

// Java STBase.fmt keeps three decimal places above one, and truncates to
// three significant fractional digits below one after rounding to eight places.
fn canvas_xml_number(value: f64) -> Result<f64, OfdError> {
    if !value.is_finite() {
        return Err(OfdError::Invalid("nonfinite canvas text number".into()));
    }
    if value.fract() == 0. {
        return Ok(value);
    }
    let small = value.abs() < 1.;
    let precision = if small { 8 } else { 3 };
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .ok_or_else(|| OfdError::Invalid("invalid canvas text number".into()))?;
    let exponent = exponent
        .parse::<i32>()
        .map_err(|e| OfdError::Invalid(e.to_string()))?;
    let digits = mantissa.replace('.', "");
    let position = 1 + exponent;
    let (integer, mut fraction) = if position <= 0 {
        ("0".to_owned(), "0".repeat((-position) as usize) + &digits)
    } else if position as usize >= digits.len() {
        (
            digits.clone() + &"0".repeat(position as usize - digits.len()),
            String::new(),
        )
    } else {
        (
            digits[..position as usize].to_owned(),
            digits[position as usize..].to_owned(),
        )
    };
    while fraction.len() <= precision {
        fraction.push('0');
    }
    let round_up = fraction.as_bytes()[precision] >= b'5';
    let mut rounded = (integer + &fraction[..precision]).into_bytes();
    if round_up {
        let mut carry = true;
        for digit in rounded.iter_mut().rev() {
            if *digit == b'9' {
                *digit = b'0';
            } else {
                *digit += 1;
                carry = false;
                break;
            }
        }
        if carry {
            rounded.insert(0, b'1');
        }
    }
    let mut text = String::from_utf8(rounded).map_err(|e| OfdError::Invalid(e.to_string()))?;
    text.insert(text.len() - precision, '.');
    if small {
        if let Some(first) = text
            .bytes()
            .position(|digit| (b'1'..=b'9').contains(&digit))
        {
            text.truncate((first + 3).min(text.len()));
        }
    }
    let result = text
        .parse::<f64>()
        .map_err(|e| OfdError::Invalid(e.to_string()))?;
    Ok(if value.is_sign_negative() {
        -result
    } else {
        result
    })
}
