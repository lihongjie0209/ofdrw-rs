//! Single-byte PDF encodings with Adobe glyph-name overrides.

use super::{
    pdf_resolved_value,
    standard_fonts::{self, StandardFont},
    Object, OfdError, PdfDocument,
};

pub(super) struct SimpleEncoding {
    characters: [Option<char>; 256],
    dingbats: bool,
    glyph_names: [Option<Vec<u8>>; 256],
}

fn invalid(message: &str) -> OfdError {
    OfdError::Invalid(format!("PDF font encoding {message}"))
}

impl SimpleEncoding {
    pub fn parse(
        pdf: &PdfDocument,
        font: &lopdf::Dictionary,
        standard_builtin: bool,
    ) -> Result<Self, OfdError> {
        let encoding = font
            .get(b"Encoding")
            .ok()
            .map(|value| pdf_resolved_value(Some(pdf), value, "font Encoding"))
            .transpose()?;
        #[derive(Clone, Copy)]
        enum Base<'a> {
            Named(&'a [u8]),
            Builtin(StandardFont),
        }
        let builtin = standard_builtin
            .then(|| {
                font.get(b"BaseFont")
                    .ok()
                    .and_then(|value| pdf_resolved_value(Some(pdf), value, "BaseFont").ok())
                    .and_then(|value| value.as_name().ok())
                    .and_then(StandardFont::from_name)
            })
            .flatten();
        let dingbats = builtin.is_some_and(StandardFont::is_dingbats);
        let default = builtin
            .filter(|font| font.encoding_names().is_some())
            .map_or(Base::Named(b"StandardEncoding"), Base::Builtin);
        let (base, differences) = match encoding {
            Some(Object::Name(name)) => (Base::Named(name.as_slice()), None),
            Some(Object::Dictionary(dictionary)) => {
                if let Ok(kind) = dictionary.get(b"Type") {
                    if pdf_resolved_value(Some(pdf), kind, "Encoding Type")?
                        .as_name()
                        .ok()
                        != Some(b"Encoding".as_slice())
                    {
                        return Err(invalid("Type is invalid"));
                    }
                }
                let base = match dictionary.get(b"BaseEncoding") {
                    Ok(value) => Base::Named(
                        pdf_resolved_value(Some(pdf), value, "BaseEncoding")?
                            .as_name()
                            .map_err(|_| invalid("BaseEncoding is not a name"))?,
                    ),
                    Err(_) if standard_builtin => default,
                    Err(_) => return Err(invalid("requires an explicit BaseEncoding")),
                };
                (base, dictionary.get(b"Differences").ok())
            }
            None if standard_builtin => (default, None),
            None => return Err(invalid("requires an explicit Encoding or a ToUnicode map")),
            _ => return Err(invalid("is not a name or dictionary")),
        };
        let mut result = match base {
            Base::Builtin(font) => {
                let names = font
                    .encoding_names()
                    .ok_or_else(|| invalid("built-in encoding is unavailable"))?;
                Self {
                    dingbats,
                    characters: std::array::from_fn(|index| {
                        names[index].and_then(|name| glyph_character(name.as_bytes(), dingbats))
                    }),
                    glyph_names: std::array::from_fn(|index| {
                        names[index].map(|name| name.as_bytes().to_vec())
                    }),
                }
            }
            Base::Named(base) => {
                if !matches!(
                    base,
                    b"StandardEncoding"
                        | b"WinAnsiEncoding"
                        | b"MacRomanEncoding"
                        | b"MacExpertEncoding"
                ) {
                    return Err(invalid("BaseEncoding is unsupported"));
                }
                let mut base_font = lopdf::Dictionary::new();
                base_font.set("Type", Object::Name(b"Font".to_vec()));
                base_font.set("Encoding", Object::Name(base.to_vec()));
                let lopdf::Encoding::OneByteEncoding(table) = base_font
                    .get_font_encoding(pdf)
                    .map_err(|error| invalid(&format!("cannot decode BaseEncoding: {error}")))?
                else {
                    return Err(invalid("BaseEncoding is not single-byte"));
                };
                Self {
                    dingbats,
                    characters: std::array::from_fn(|index| {
                        table[index].and_then(|value| char::from_u32(u32::from(value)))
                    }),
                    glyph_names: std::array::from_fn(|_| None),
                }
            }
        };
        if let Some(differences) = differences {
            let differences = pdf_resolved_value(Some(pdf), differences, "Encoding Differences")?
                .as_array()
                .map_err(|_| invalid("Differences is not an array"))?;
            if differences.len() > 4096 {
                return Err(OfdError::Limit(
                    "PDF Encoding Differences has too many entries".into(),
                ));
            }
            let mut next = None;
            for value in differences {
                match pdf_resolved_value(Some(pdf), value, "Encoding Differences entry")? {
                    Object::Integer(code) if (0..=255).contains(code) => {
                        next = Some(*code as usize)
                    }
                    Object::Name(name) => {
                        let code = next
                            .filter(|code| *code < 256)
                            .ok_or_else(|| invalid("Differences character code is invalid"))?;
                        if name.len() > 1024 {
                            return Err(OfdError::Limit(
                                "PDF Encoding glyph name is too long".into(),
                            ));
                        }
                        result.characters[code] = glyph_character(name, dingbats);
                        result.glyph_names[code] = Some(name.clone());
                        next = Some(code + 1);
                    }
                    _ => return Err(invalid("Differences entry is invalid")),
                }
            }
        }
        Ok(result)
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<String, OfdError> {
        let mut text = String::new();
        for code in bytes {
            let index = usize::from(*code);
            if let Some(name) = &self.glyph_names[index] {
                let decoded = self.characters[index]
                    .map(|ch| ch.to_string())
                    .unwrap_or_else(|| glyph_sequence(name, self.dingbats));
                if decoded.is_empty() {
                    return Err(invalid("character has no Unicode mapping"));
                }
                text.push_str(&decoded);
            } else {
                text.push(
                    self.characters[index]
                        .ok_or_else(|| invalid("character has no Unicode mapping"))?,
                );
            }
        }
        Ok(text)
    }

    pub fn width(&self, font: StandardFont, code: u8) -> f64 {
        match &self.glyph_names[usize::from(code)] {
            Some(name) => font.glyph_width(name),
            None => self.characters[usize::from(code)]
                .map_or(0.0, |character| font.character_width(character)),
        }
    }

    pub fn glyph_ids(&self, face: &ttf_parser::Face<'_>, codes: &[u8]) -> Option<Vec<u16>> {
        codes
            .iter()
            .map(|code| {
                match &self.glyph_names[usize::from(*code)] {
                    Some(name) => std::str::from_utf8(name)
                        .ok()
                        .and_then(|name| face.glyph_index_by_name(name)),
                    None => self.characters[usize::from(*code)].and_then(|ch| face.glyph_index(ch)),
                }
                .map(|glyph| glyph.0)
            })
            .collect()
    }
}

fn glyph_character(name: &[u8], dingbats: bool) -> Option<char> {
    let sequence = glyph_sequence(name, dingbats);
    let mut chars = sequence.chars();
    let first = chars.next()?;
    chars.next().is_none().then_some(first)
}

// Adobe Glyph List specification §2: suffix removal, component expansion,
// then concatenation. A malformed component contributes an empty sequence.
fn glyph_sequence(name: &[u8], dingbats: bool) -> String {
    let Ok(name) = std::str::from_utf8(name) else {
        return String::new();
    };
    let mut result = String::new();
    for component in name.split('.').next().unwrap_or_default().split('_') {
        if let Some(ch) = dingbats
            .then(|| standard_fonts::dingbat_character(component))
            .flatten()
        {
            result.push(ch);
            continue;
        }
        let uppercase_hex = |digits: &str| {
            !digits.is_empty()
                && digits
                    .bytes()
                    .all(|value| value.is_ascii_digit() || (b'A'..=b'F').contains(&value))
        };
        if let Some(digits) = component
            .strip_prefix("uni")
            .filter(|digits| uppercase_hex(digits))
        {
            if digits.len() % 4 == 0 {
                let values: Option<String> = digits
                    .as_bytes()
                    .chunks_exact(4)
                    .map(|chunk| {
                        let digits = std::str::from_utf8(chunk).ok()?;
                        char::from_u32(u32::from_str_radix(digits, 16).ok()?)
                    })
                    .collect();
                if let Some(values) = values {
                    result.push_str(&values);
                }
            }
        } else if let Some(digits) = component
            .strip_prefix('u')
            .filter(|digits| uppercase_hex(digits))
        {
            if (4..=6).contains(&digits.len()) {
                if let Some(ch) = u32::from_str_radix(digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    result.push(ch);
                }
            }
        } else {
            result.extend(read_fonts::ps::agl::name_to_chars(component));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    fn font(differences: Vec<Object>) -> lopdf::Dictionary {
        dictionary! {"Encoding" => dictionary! {
            "BaseEncoding" => Object::Name(b"WinAnsiEncoding".to_vec()),
            "Differences" => Object::Array(differences),
        }}
    }

    #[test]
    fn adobe_glyph_names_and_unicode_names_preserve_single_scalars() {
        for (name, expected) in [
            ("Euro", '€'),
            ("Aacute", 'Á'),
            ("u", 'u'),
            ("uacute", 'ú'),
            ("universal", '∀'),
            ("union", '∪'),
            ("underscore", '_'),
            ("uni4E2D", '中'),
            ("u1F600", '😀'),
            ("A.swash", 'A'),
            ("unknown_A", 'A'),
            ("uniD800_A", 'A'),
        ] {
            assert_eq!(
                glyph_character(name.as_bytes(), false),
                Some(expected),
                "{name}"
            );
        }
    }

    #[test]
    fn unknown_multiple_and_invalid_unicode_glyph_names_have_no_single_mapping() {
        for name in [
            "unknown",
            ".notdef",
            "A_B",
            "uni00410042",
            "uni20ac",
            "uniD800",
            "uD800",
            "u110000",
            "u1f600",
            "uniD83DDE00",
            "dalethatafpatah",
        ] {
            assert_eq!(glyph_character(name.as_bytes(), false), None, "{name}");
        }
        assert_eq!(glyph_character(&[255], false), None);
    }

    #[test]
    fn adobe_sequences_expand_components_and_discard_invalid_components() {
        for (name, expected) in [
            ("A_B", "AB"),
            ("uni20AC0308", "€\u{0308}"),
            ("Lcommaaccent_uni20AC0308_u1040C.alternate", "Ļ€\u{0308}𐐌"),
            ("uniD801DC0C_A", "A"),
            ("unknown_f_i", "fi"),
            ("dalethatafpatah", "דֲ"),
        ] {
            assert_eq!(glyph_sequence(name.as_bytes(), false), expected, "{name}");
        }
        let pdf = PdfDocument::new();
        let definition = font(vec![65.into(), Object::Name(b"uni00660069".to_vec())]);
        let encoding = SimpleEncoding::parse(&pdf, &definition, false).unwrap();
        assert_eq!(encoding.decode(b"ABA").unwrap(), "fiBfi");
        let definition = dictionary! {
            "BaseFont" => Object::Name(b"ZapfDingbats".to_vec()),
            "Encoding" => dictionary! {
                "Differences" => Object::Array(vec![65.into(), Object::Name(b"a1_a2".to_vec())]),
            },
        };
        let encoding = SimpleEncoding::parse(&pdf, &definition, true).unwrap();
        assert_eq!(encoding.decode(b"A").unwrap(), "✁✂");
    }

    #[test]
    fn differences_retain_base_codes_and_apply_later_overrides() {
        let pdf = PdfDocument::new();
        let definition = font(vec![
            65.into(),
            Object::Name(b"B".to_vec()),
            Object::Name(b"A".to_vec()),
            65.into(),
            Object::Name(b"Euro".to_vec()),
            255.into(),
            Object::Name(b"u1F600".to_vec()),
        ]);
        let encoding = SimpleEncoding::parse(&pdf, &definition, true).unwrap();
        assert_eq!(encoding.decode(&[65, 66, 67, 255]).unwrap(), "€AC😀");
        for (name, expected) in [("WinAnsiEncoding", "€"), ("MacRomanEncoding", "Ä")] {
            let definition = dictionary! {"Encoding" => dictionary! {
                "BaseEncoding" => Object::Name(name.as_bytes().to_vec()), "Differences" => Object::Array(vec![]),
            }};
            assert_eq!(
                SimpleEncoding::parse(&pdf, &definition, true)
                    .unwrap()
                    .decode(&[128])
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn differences_resolve_indirect_containers_and_members() {
        let mut pdf = PdfDocument::new();
        let base = pdf.add_object(Object::Name(b"WinAnsiEncoding".to_vec()));
        let start = pdf.add_object(Object::Integer(65));
        let glyph = pdf.add_object(Object::Name(b"uni4E2D".to_vec()));
        let differences = pdf.add_object(Object::Array(vec![
            Object::Reference(start),
            Object::Reference(glyph),
        ]));
        let kind = pdf.add_object(Object::Name(b"Encoding".to_vec()));
        let definition = pdf.add_object(dictionary! {"Type" => Object::Reference(kind),
            "BaseEncoding" => Object::Reference(base), "Differences" => Object::Reference(differences)});
        let definition = dictionary! {"Encoding" => Object::Reference(definition)};
        assert_eq!(
            SimpleEncoding::parse(&pdf, &definition, true)
                .unwrap()
                .decode(b"AB")
                .unwrap(),
            "中B"
        );
    }

    #[test]
    fn differences_validate_codes_types_and_missing_base_encodings() {
        let pdf = PdfDocument::new();
        assert!(SimpleEncoding::parse(&pdf, &lopdf::Dictionary::new(), false).is_err());
        for values in [
            vec![(-1).into()],
            vec![256.into()],
            vec![Object::Real(65.0)],
            vec![Object::Name(b"A".to_vec())],
            vec![
                255.into(),
                Object::Name(b"A".to_vec()),
                Object::Name(b"B".to_vec()),
            ],
            vec![65.into(), Object::Boolean(false)],
            vec![Object::Reference((999, 0))],
        ] {
            assert!(SimpleEncoding::parse(&pdf, &font(values), true).is_err());
        }
        let definition = dictionary! {"Encoding" => dictionary! {"Differences" => Object::Array(vec![65.into(), Object::Name(b"B".to_vec())])}};
        assert_eq!(
            SimpleEncoding::parse(&pdf, &definition, true)
                .unwrap()
                .decode(b"AB")
                .unwrap(),
            "BB"
        );
        assert!(SimpleEncoding::parse(&pdf, &definition, false).is_err());
        for name in [
            b"StandardEncoding".as_slice(),
            b"WinAnsiEncoding".as_slice(),
            b"MacRomanEncoding".as_slice(),
        ] {
            let definition = dictionary! {"Encoding" => Object::Name(name.to_vec())};
            assert_eq!(
                SimpleEncoding::parse(&pdf, &definition, false)
                    .unwrap()
                    .decode(b"AB")
                    .unwrap(),
                "AB"
            );
        }
        for encoding in [
            Object::Integer(1),
            Object::Name(b"Unknown".to_vec()),
            Object::Dictionary(dictionary! {"Type" => Object::Name(b"Wrong".to_vec())}),
            Object::Dictionary(dictionary! {"BaseEncoding" => 1}),
            Object::Dictionary(dictionary! {"Differences" => 1}),
        ] {
            assert!(
                SimpleEncoding::parse(&pdf, &dictionary! {"Encoding" => encoding}, true).is_err()
            );
        }
    }

    #[test]
    fn macexpert_encoding_keeps_private_use_small_caps_and_differences() {
        let pdf = PdfDocument::new();
        let definition = dictionary! {"Encoding" => Object::Name(b"MacExpertEncoding".to_vec())};
        assert_eq!(
            SimpleEncoding::parse(&pdf, &definition, false)
                .unwrap()
                .decode(&[32, 33, 97])
                .unwrap(),
            " \u{f721}\u{f761}"
        );
        let definition = dictionary! {"Encoding" => dictionary! {
            "BaseEncoding" => Object::Name(b"MacExpertEncoding".to_vec()),
            "Differences" => Object::Array(vec![97.into(), Object::Name(b"uni4E2D".to_vec())]),
        }};
        assert_eq!(
            SimpleEncoding::parse(&pdf, &definition, false)
                .unwrap()
                .decode(&[32, 33, 97])
                .unwrap(),
            " \u{f721}中"
        );
    }

    #[test]
    fn unknown_unused_glyph_names_do_not_reject_other_text() {
        let pdf = PdfDocument::new();
        let definition = font(vec![
            1.into(),
            Object::Name(b"unknown".to_vec()),
            Object::Name(b"A_B".to_vec()),
        ]);
        let encoding = SimpleEncoding::parse(&pdf, &definition, true).unwrap();
        assert_eq!(encoding.decode(b"AB").unwrap(), "AB");
        assert!(encoding.decode(&[1]).is_err());
        assert_eq!(encoding.decode(&[2]).unwrap(), "AB");
    }

    #[test]
    fn differences_reject_cycles_and_limit_repeated_declarations() {
        let mut pdf = PdfDocument::new();
        let cycle = pdf.add_object(Object::Reference((1, 0)));
        assert!(SimpleEncoding::parse(
            &pdf,
            &dictionary! {"Encoding" => Object::Reference(cycle)},
            true
        )
        .is_err());
        for key in ["BaseEncoding", "Differences"] {
            let definition =
                dictionary! {"Encoding" => dictionary! {key => Object::Reference(cycle)}};
            assert!(SimpleEncoding::parse(&pdf, &definition, true).is_err());
        }
        let definition = font(vec![Object::Reference(cycle)]);
        assert!(SimpleEncoding::parse(&pdf, &definition, true).is_err());
        let definition = font(vec![0.into(); 4096]);
        assert!(SimpleEncoding::parse(&pdf, &definition, true).is_ok());
        assert!(matches!(
            SimpleEncoding::parse(&pdf, &font(vec![0.into(); 4097]), true),
            Err(OfdError::Limit(_))
        ));
        assert!(matches!(
            SimpleEncoding::parse(
                &pdf,
                &font(vec![0.into(), Object::Name(vec![b'A'; 1025])]),
                true
            ),
            Err(OfdError::Limit(_))
        ));
    }

    #[test]
    fn embedded_font_glyphs_follow_source_encoding_and_require_actual_variant_names() {
        let pdf = PdfDocument::new();
        let face =
            ttf_parser::Face::parse(include_bytes!("../../tests/fixtures/dejavu-sans.ttf"), 0)
                .unwrap();
        let encoding = SimpleEncoding::parse(
            &pdf,
            &font(vec![65.into(), Object::Name(b"B".to_vec())]),
            false,
        )
        .unwrap();
        assert_eq!(
            encoding.glyph_ids(&face, b"A"),
            Some(vec![face.glyph_index('B').unwrap().0])
        );
        let variant = SimpleEncoding::parse(
            &pdf,
            &font(vec![65.into(), Object::Name(b"A.swash".to_vec())]),
            false,
        )
        .unwrap();
        assert!(variant.glyph_ids(&face, b"A").is_none());
    }

    #[test]
    fn symbol_and_dingbats_differences_inherit_their_native_bases() {
        let pdf = PdfDocument::new();
        for (base, glyph, expected) in [("Symbol", "Beta", "ΒΒ"), ("ZapfDingbats", "a2", "✂✂")]
        {
            let definition = dictionary! {"BaseFont" => Object::Name(base.as_bytes().to_vec()),
            "Encoding" => dictionary! {"Differences" => Object::Array(vec![
                if base == "Symbol" {65.into()} else {33.into()}, Object::Name(glyph.as_bytes().to_vec())])}};
            let encoding = SimpleEncoding::parse(&pdf, &definition, true).unwrap();
            let codes = if base == "Symbol" { b"AB" } else { b"!\"" };
            assert_eq!(encoding.decode(codes).unwrap(), expected);
        }
    }
}
