//! Font classification for imported text, including substitute-font styling.

use super::{pdf_resolved_value, standard_fonts::StandardFont, OfdError, PdfDocument};
use lopdf::{Dictionary, Object};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FontStyle {
    pub weight: u16,
    pub italic: bool,
}

impl Default for FontStyle {
    fn default() -> Self {
        Self {
            weight: 400,
            italic: false,
        }
    }
}

impl FontStyle {
    pub fn read(pdf: &PdfDocument, font: &Dictionary) -> Result<Self, OfdError> {
        let mut style = font
            .get(b"BaseFont")
            .and_then(Object::as_name)
            .ok()
            .and_then(StandardFont::from_name)
            .map_or_else(Self::default, StandardFont::style);
        let font =
            if font.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0".as_slice()) {
                let descendants = font
                    .get(b"DescendantFonts")
                    .map_err(|_| invalid("missing DescendantFonts"))?;
                let descendants = pdf_resolved_value(Some(pdf), descendants, "DescendantFonts")?
                    .as_array()
                    .map_err(|_| invalid("DescendantFonts is not an array"))?;
                let descendant = descendants
                    .first()
                    .ok_or_else(|| invalid("empty DescendantFonts"))?;
                pdf_resolved_value(Some(pdf), descendant, "descendant font")?
                    .as_dict()
                    .map_err(|_| invalid("descendant font is not a dictionary"))?
            } else {
                font
            };
        let Ok(descriptor) = font.get(b"FontDescriptor") else {
            return Ok(style);
        };
        let descriptor = pdf_resolved_value(Some(pdf), descriptor, "FontDescriptor")?
            .as_dict()
            .map_err(|_| invalid("FontDescriptor is not a dictionary"))?;
        if let Some(value) = field(pdf, descriptor, b"FontWeight")? {
            let weight = number(value)?;
            if !(100.0..=900.0).contains(&weight) || weight % 100.0 != 0.0 {
                return Err(invalid(
                    "FontWeight must be 100 through 900 in steps of 100",
                ));
            }
            style.weight = weight as u16;
        }
        let angle = field(pdf, descriptor, b"ItalicAngle")?
            .map(number)
            .transpose()?;
        let flags = field(pdf, descriptor, b"Flags")?
            .map(|value| {
                value
                    .as_i64()
                    .ok()
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or_else(|| invalid("Flags must be an unsigned 32-bit integer"))
            })
            .transpose()?;
        if angle.is_some() || flags.is_some() {
            style.italic = angle.is_some_and(|angle| angle != 0.0)
                || flags.is_some_and(|flags| flags & 64 != 0);
        }
        Ok(style)
    }

    pub fn from_face(face: &ttf_parser::Face<'_>) -> Self {
        let weight = face.weight().to_number();
        Self {
            // OFD has nine discrete weights; retain the closest classification.
            weight: if (1..=1000).contains(&weight) {
                ((weight + 50) / 100 * 100).clamp(100, 900)
            } else if face.is_bold() {
                700
            } else {
                400
            },
            italic: face.is_italic(),
        }
    }
}

fn invalid(message: &str) -> OfdError {
    OfdError::Invalid(format!("invalid PDF font style: {message}"))
}

fn number(value: &Object) -> Result<f64, OfdError> {
    let value = match value {
        Object::Integer(value) => *value as f64,
        Object::Real(value) => f64::from(*value),
        _ => return Err(invalid("expected a number")),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(invalid("non-finite number"))
    }
}

fn field<'a>(
    pdf: &'a PdfDocument,
    descriptor: &'a Dictionary,
    name: &[u8],
) -> Result<Option<&'a Object>, OfdError> {
    descriptor
        .get(name)
        .ok()
        .map(|value| pdf_resolved_value(Some(pdf), value, "FontDescriptor field"))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    fn read_descriptor(descriptor: Dictionary) -> Result<FontStyle, OfdError> {
        FontStyle::read(
            &PdfDocument::new(),
            &dictionary! {"FontDescriptor" => descriptor},
        )
    }

    #[test]
    fn descriptor_style_resolves_indirect_fields_and_cid_descendant() {
        let mut pdf = PdfDocument::new();
        let weight = pdf.add_object(Object::Real(600.0));
        let angle = pdf.add_object(Object::Real(-12.0));
        let flags = pdf.add_object(Object::Integer(64));
        let descriptor = pdf.add_object(dictionary! {
            "FontWeight" => weight, "ItalicAngle" => angle, "Flags" => flags
        });
        let descendant = pdf.add_object(dictionary! {"FontDescriptor" => descriptor});
        let descendants = pdf.add_object(Object::Array(vec![descendant.into()]));
        let font = dictionary! {"Subtype" => Object::Name(b"Type0".to_vec()), "DescendantFonts" => descendants};
        assert_eq!(
            FontStyle::read(&pdf, &font).unwrap(),
            FontStyle {
                weight: 600,
                italic: true
            }
        );
    }

    #[test]
    fn descriptor_italic_angle_and_flag_are_independent_and_forcebold_is_not_weight() {
        for descriptor in [
            dictionary! {"ItalicAngle" => -12},
            dictionary! {"Flags" => 64},
            dictionary! {"ItalicAngle" => 0, "Flags" => 64},
        ] {
            assert!(read_descriptor(descriptor).unwrap().italic);
        }
        assert_eq!(
            read_descriptor(dictionary! {"Flags" => 262144}).unwrap(),
            FontStyle::default()
        );
        for weight in (100..=900).step_by(100) {
            assert_eq!(
                read_descriptor(dictionary! {"FontWeight" => weight})
                    .unwrap()
                    .weight,
                weight as u16
            );
        }
    }

    #[test]
    fn malformed_style_fields_fail_instead_of_silently_changing_font() {
        for value in [
            0.into(),
            1000.into(),
            750.into(),
            Object::Real(f32::NAN),
            Object::Real(f32::INFINITY),
            Object::Null,
        ] {
            assert!(read_descriptor(dictionary! {"FontWeight" => value}).is_err());
        }
        for value in [
            (-1).into(),
            Object::Integer(i64::from(u32::MAX) + 1),
            Object::Real(64.0),
            Object::Null,
        ] {
            assert!(read_descriptor(dictionary! {"Flags" => value}).is_err());
        }
        for value in [
            Object::Real(f32::NAN),
            Object::Real(f32::INFINITY),
            Object::Null,
        ] {
            assert!(read_descriptor(dictionary! {"ItalicAngle" => value}).is_err());
        }
        let mut pdf = PdfDocument::new();
        let cycle = pdf.add_object(Object::Reference((1, 0)));
        for font in [
            dictionary! {"FontDescriptor" => cycle},
            dictionary! {"FontDescriptor" => dictionary! {"FontWeight" => cycle}},
            dictionary! {"FontDescriptor" => Object::Reference((999, 0))},
        ] {
            assert!(FontStyle::read(&pdf, &font).is_err());
        }
    }

    #[test]
    fn font_program_reports_actual_style() {
        for (bytes, expected) in [
            (
                include_bytes!("../../tests/fixtures/dejavu-sans.ttf").as_slice(),
                FontStyle::default(),
            ),
            (
                include_bytes!("../../tests/fixtures/dejavu-sans-bold-oblique.ttf").as_slice(),
                FontStyle {
                    weight: 700,
                    italic: true,
                },
            ),
        ] {
            assert_eq!(
                FontStyle::from_face(&ttf_parser::Face::parse(bytes, 0).unwrap()),
                expected
            );
        }
    }
}
