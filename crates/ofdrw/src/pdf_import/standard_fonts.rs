//! Adobe core-font advances, without relying on installed substitute fonts.

use super::standard_font_data as data;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StandardFont(usize);

impl StandardFont {
    pub fn style(self) -> super::font_style::FontStyle {
        let name = data::FONTS[self.0].0;
        super::font_style::FontStyle {
            weight: if name.contains("Bold") { 700 } else { 400 },
            italic: name.ends_with("Oblique") || name.ends_with("Italic"),
        }
    }

    pub fn from_name(name: &[u8]) -> Option<Self> {
        let name =
            if name.len() > 7 && name[..6].iter().all(u8::is_ascii_uppercase) && name[6] == b'+' {
                &name[7..]
            } else {
                name
            };
        data::FONTS
            .iter()
            .position(|(font, _)| font.as_bytes() == name)
            .map(Self)
    }

    pub fn glyph_width(self, name: &[u8]) -> f64 {
        let glyphs = data::FONTS[self.0].1;
        glyphs
            .binary_search_by(|(glyph, _)| glyph.as_bytes().cmp(name))
            .ok()
            .map_or(0.0, |index| f64::from(glyphs[index].1))
    }

    pub fn character_width(self, character: char) -> f64 {
        let character = match character {
            '\u{a0}' => ' ',
            '\u{ad}' => '-',
            value => value,
        };
        let mut buffer = [0; read_fonts::ps::agl::MAX_NAME_LEN];
        read_fonts::ps::agl::char_to_name(character as u32, &mut buffer)
            .map_or(0.0, |name| self.glyph_width(name.as_bytes()))
    }

    pub fn encoding_names(self) -> Option<&'static [Option<&'static str>; 256]> {
        match data::FONTS[self.0].0 {
            "Symbol" => Some(&data::SYMBOL_CODES),
            "ZapfDingbats" => Some(&data::DINGBATS_CODES),
            _ => None,
        }
    }

    pub fn is_dingbats(self) -> bool {
        data::FONTS[self.0].0 == "ZapfDingbats"
    }
}

pub(super) fn dingbat_character(name: &str) -> Option<char> {
    data::DINGBATS
        .binary_search_by_key(&name, |(glyph, _)| *glyph)
        .ok()
        .map(|index| data::DINGBATS[index].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_font_tables_cover_all_fourteen_faces_and_known_advances() {
        assert_eq!(data::FONTS.len(), 14);
        for (name, glyph, width) in [
            ("Courier", "A", 600),
            ("Courier-Bold", "A", 600),
            ("Courier-Oblique", "A", 600),
            ("Courier-BoldOblique", "A", 600),
            ("Helvetica", "A", 667),
            ("Helvetica-Bold", "A", 722),
            ("Helvetica-Oblique", "A", 667),
            ("Helvetica-BoldOblique", "A", 722),
            ("Times-Roman", "A", 722),
            ("Times-Bold", "A", 722),
            ("Times-Italic", "A", 611),
            ("Times-BoldItalic", "A", 667),
            ("Symbol", "Alpha", 722),
            ("ZapfDingbats", "a1", 974),
        ] {
            let font = StandardFont::from_name(name.as_bytes()).unwrap();
            assert_eq!(
                font.glyph_width(glyph.as_bytes()),
                f64::from(width),
                "{name}"
            );
            assert_eq!(font.glyph_width(b"unknown"), 0.0);
        }
        for (_, glyphs) in data::FONTS {
            assert!(glyphs.len() > 100);
            assert!(glyphs.windows(2).all(|pair| pair[0].0 < pair[1].0));
            assert!(glyphs.iter().all(|(_, width)| *width <= 10000));
        }
    }

    #[test]
    fn standard_fonts_resolve_subset_names_and_unicode_spacing_aliases() {
        let font = StandardFont::from_name(b"Helvetica").unwrap();
        assert_eq!(StandardFont::from_name(b"ABCDEF+Helvetica"), Some(font));
        for name in [
            b"abcdef+Helvetica".as_slice(),
            b"ABCDE+Helvetica",
            b"Unknown",
            b"",
        ] {
            assert_eq!(StandardFont::from_name(name), None);
        }
        assert_eq!(font.character_width('\u{a0}'), 278.0);
        assert_eq!(font.character_width('\u{ad}'), 333.0);
        assert_eq!(font.character_width('中'), 0.0);
        assert_eq!(dingbat_character("a1"), Some('✁'));
        assert_eq!(dingbat_character("unknown"), None);
        assert!(StandardFont::from_name(b"Symbol")
            .unwrap()
            .encoding_names()
            .is_some());
        assert!(StandardFont::from_name(b"ZapfDingbats")
            .unwrap()
            .is_dingbats());
        assert!(font.encoding_names().is_none());
    }
}
