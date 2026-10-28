//! Validated, bounded width tables for single-byte PDF fonts.

use super::{pdf_resolved_value, OfdError, PdfDocument};

pub(super) struct SimpleWidths {
    first: u8,
    values: Vec<f64>,
}

impl SimpleWidths {
    pub fn parse(pdf: &PdfDocument, font: &lopdf::Dictionary) -> Result<Option<Self>, OfdError> {
        let (first, widths) = match (font.get(b"FirstChar"), font.get(b"Widths")) {
            (Ok(first), Ok(widths)) => (first, widths),
            (Err(_), Err(_)) => return Ok(None),
            _ => return Err(OfdError::Invalid("PDF font widths are incomplete".into())),
        };
        let first = pdf_resolved_value(Some(pdf), first, "FirstChar")?
            .as_i64()
            .map_err(|_| OfdError::Invalid("PDF FirstChar is invalid".into()))?;
        if !(0..=255).contains(&first) {
            return Err(OfdError::Invalid("PDF FirstChar is invalid".into()));
        }
        let last = font
            .get(b"LastChar")
            .map_err(|_| OfdError::Invalid("PDF LastChar is invalid".into()))?;
        let last = pdf_resolved_value(Some(pdf), last, "LastChar")?
            .as_i64()
            .map_err(|_| OfdError::Invalid("PDF LastChar is invalid".into()))?;
        if !(first..=255).contains(&last) {
            return Err(OfdError::Invalid("PDF LastChar is invalid".into()));
        }
        let widths = pdf_resolved_value(Some(pdf), widths, "Widths")?
            .as_array()
            .map_err(|_| OfdError::Invalid("PDF Widths are not an array".into()))?;
        if widths.len() != (last - first + 1) as usize {
            return Err(OfdError::Invalid("PDF Widths length is invalid".into()));
        }
        let values = widths
            .iter()
            .map(|value| {
                let value = pdf_resolved_value(Some(pdf), value, "glyph width")?
                    .as_float()
                    .map(f64::from)
                    .map_err(|_| OfdError::Invalid("PDF glyph width is invalid".into()))?;
                if !value.is_finite() || !(0.0..=10_000.0).contains(&value) {
                    return Err(OfdError::Invalid("PDF glyph width is invalid".into()));
                }
                Ok(value)
            })
            .collect::<Result<Vec<_>, OfdError>>()?;
        Ok(Some(Self {
            first: first as u8,
            values,
        }))
    }

    pub fn get(&self, code: u8) -> Result<f64, OfdError> {
        usize::from(code)
            .checked_sub(usize::from(self.first))
            .and_then(|index| self.values.get(index))
            .copied()
            .ok_or_else(|| OfdError::Invalid("PDF glyph width is invalid".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Object};

    #[test]
    fn indirect_simple_width_tables_resolve_bounds_arrays_and_values() {
        let mut pdf = PdfDocument::new();
        let first = pdf.add_object(Object::Integer(65));
        let last = pdf.add_object(Object::Integer(66));
        let width = pdf.add_object(Object::Real(600.5));
        let widths = pdf.add_object(Object::Array(vec![Object::Reference(width), 0.into()]));
        let font = dictionary! {"FirstChar" => Object::Reference(first), "LastChar" => Object::Reference(last),
        "Widths" => Object::Reference(widths)};
        let widths = SimpleWidths::parse(&pdf, &font).unwrap().unwrap();
        assert_eq!(widths.get(65).unwrap(), 600.5);
        assert_eq!(widths.get(66).unwrap(), 0.0);
        assert!(widths.get(64).is_err());
        assert!(widths.get(67).is_err());
    }

    #[test]
    fn simple_widths_accept_full_byte_ranges_and_omitted_tables() {
        let pdf = PdfDocument::new();
        let font = dictionary! {"FirstChar" => 0, "LastChar" => 255, "Widths" => Object::Array(vec![10000.into(); 256])};
        let widths = SimpleWidths::parse(&pdf, &font).unwrap().unwrap();
        assert_eq!(widths.get(0).unwrap(), 10000.0);
        assert_eq!(widths.get(255).unwrap(), 10000.0);
        assert!(SimpleWidths::parse(&pdf, &lopdf::Dictionary::new())
            .unwrap()
            .is_none());
    }

    #[test]
    fn simple_widths_reject_malformed_bounds_counts_and_types() {
        let pdf = PdfDocument::new();
        let original = dictionary! {"FirstChar" => 65, "LastChar" => 66, "Widths" => Object::Array(vec![600.into(), 500.into()])};
        for (key, value) in [
            ("FirstChar", (-1).into()),
            ("FirstChar", 256.into()),
            ("FirstChar", Object::Real(65.0)),
            ("LastChar", 64.into()),
            ("LastChar", 256.into()),
            ("LastChar", Object::Real(66.0)),
            ("Widths", 1.into()),
            ("Widths", Object::Array(vec![])),
            ("Widths", Object::Array(vec![500.into()])),
            ("Widths", Object::Array(vec![500.into(); 257])),
        ] {
            let mut font = original.clone();
            font.set(key, value);
            assert!(SimpleWidths::parse(&pdf, &font).is_err(), "{key}");
        }
        for key in [b"FirstChar".as_slice(), b"LastChar", b"Widths"] {
            let mut font = original.clone();
            font.remove(key);
            assert!(SimpleWidths::parse(&pdf, &font).is_err());
        }
    }

    #[test]
    fn simple_widths_validate_unused_values_and_reject_reference_cycles() {
        let mut pdf = PdfDocument::new();
        let cycle = pdf.add_object(Object::Reference((1, 0)));
        for value in [
            Object::Integer(-1),
            Object::Integer(10001),
            Object::Real(f32::NAN),
            Object::Real(f32::INFINITY),
            Object::Name(b"bad".to_vec()),
            Object::Reference(cycle),
            Object::Reference((999, 0)),
        ] {
            let id = pdf.add_object(value);
            let font = dictionary! {"FirstChar" => 65, "LastChar" => 66,
            "Widths" => Object::Array(vec![600.into(), Object::Reference(id)])};
            assert!(SimpleWidths::parse(&pdf, &font).is_err());
        }
        for key in ["FirstChar", "LastChar", "Widths"] {
            for value in [Object::Reference(cycle), Object::Reference((999, 0))] {
                let mut font = dictionary! {"FirstChar" => 65, "LastChar" => 66, "Widths" => Object::Array(vec![600.into(), 500.into()])};
                font.set(key, value);
                assert!(SimpleWidths::parse(&pdf, &font).is_err());
            }
        }
    }
}
