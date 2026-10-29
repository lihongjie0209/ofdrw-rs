//! Registered Adobe CID collections supply Unicode only without ToUnicode.

use std::{
    io::{Cursor, Read},
    sync::OnceLock,
};

use lopdf::{Dictionary, Encoding, Object, Stream};

use super::{pdf_resolved_value, pdf_to_unicode_map, OfdError, PdfDocument};

const COLLECTIONS: [&str; 5] = ["CNS1", "GB1", "Japan1", "KR", "Korea1"];
static MAPS: [OnceLock<CollectionMap>; 5] = [const { OnceLock::new() }; 5];

pub(super) struct CollectionMap {
    encoding: Encoding<'static>,
}

impl CollectionMap {
    pub fn units(&self, cid: u16) -> Option<Vec<u16>> {
        match &self.encoding {
            Encoding::UnicodeMapEncoding(map) => map.get(u32::from(cid), 2),
            _ => None,
        }
    }
}

fn invalid(message: &str) -> OfdError {
    OfdError::Invalid(format!("PDF CID-to-Unicode mapping: {message}"))
}

pub(super) fn for_descendant(
    pdf: &PdfDocument,
    descendant: &Dictionary,
) -> Result<&'static CollectionMap, OfdError> {
    let info = descendant
        .get(b"CIDSystemInfo")
        .map_err(|_| invalid("CIDSystemInfo is missing"))?;
    let info = pdf_resolved_value(Some(pdf), info, "CIDSystemInfo")?
        .as_dict()
        .map_err(|_| invalid("CIDSystemInfo is not a dictionary"))?;
    let field = |name: &[u8]| -> Result<&Object, OfdError> {
        let value = info
            .get(name)
            .map_err(|_| invalid("CIDSystemInfo is incomplete"))?;
        pdf_resolved_value(Some(pdf), value, "CIDSystemInfo field")
    };
    if field(b"Registry")?.as_str().ok() != Some(b"Adobe".as_slice()) {
        return Err(invalid("unregistered Registry requires ToUnicode"));
    }
    let ordering = field(b"Ordering")?
        .as_str()
        .map_err(|_| invalid("Ordering is not a byte string"))?;
    let index = COLLECTIONS
        .iter()
        .position(|name| name.as_bytes() == ordering)
        .ok_or_else(|| invalid("unsupported Ordering requires ToUnicode"))?;
    if field(b"Supplement")?
        .as_i64()
        .ok()
        .is_none_or(|value| value < 0)
    {
        return Err(invalid("Supplement must be a nonnegative integer"));
    }
    collection(index)
}

fn collection(index: usize) -> Result<&'static CollectionMap, OfdError> {
    let slot = &MAPS[index];
    if let Some(map) = slot.get() {
        return Ok(map);
    }
    let name = format!("Adobe-{}-UCS2", COLLECTIONS[index]);
    let mut archive = zip::ZipArchive::new(Cursor::new(
        include_bytes!("../../assets/cid_unicode/unicode.zip").as_slice(),
    ))
    .map_err(|_| invalid("invalid built-in resource archive"))?;
    let mut bytes = Vec::new();
    archive
        .by_name(&name)
        .map_err(|_| invalid("missing built-in resource"))?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(OfdError::Limit(
            "Adobe Unicode resource is too large".into(),
        ));
    }
    let mut font = Dictionary::new();
    font.set("Type", Object::Name(b"Font".to_vec()));
    font.set("Subtype", Object::Name(b"Type0".to_vec()));
    font.set("Encoding", Object::Name(b"Identity-H".to_vec()));
    font.set(
        "ToUnicode",
        Object::Stream(Stream::new(Dictionary::new(), bytes)),
    );
    let Encoding::UnicodeMapEncoding(map) = pdf_to_unicode_map(&font, &PdfDocument::new())? else {
        return Err(invalid("resource is not a Unicode CMap"));
    };
    // The Unicode variant owns its mappings and borrows nothing from the font.
    let _ = slot.set(CollectionMap {
        encoding: Encoding::UnicodeMapEncoding(map),
    });
    slot.get()
        .ok_or_else(|| invalid("cannot initialize built-in resource"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn every_adobe_collection_parses_and_maps_known_cids() {
        for (index, first, second, expected) in [
            (0, 661, 726, "中文"),
            (1, 4559, 3795, "中文"),
            (2, 3284, 3722, "日本"),
            (3, 1, 2, " !"),
            (4, 3296, 1204, "한국"),
        ] {
            let map = collection(index).unwrap();
            let units = [map.units(first).unwrap(), map.units(second).unwrap()].concat();
            assert_eq!(
                String::from_utf16(&units).unwrap(),
                expected,
                "{}",
                COLLECTIONS[index]
            );
        }
        let japanese = collection(2).unwrap();
        assert_eq!(
            String::from_utf16(&japanese.units(19130).unwrap()).unwrap(),
            "\u{31350}"
        );
        assert_eq!(japanese.units(230).unwrap(), [0x0030, 0xfe00]);
        assert!(japanese.units(65535).is_none());
    }

    #[test]
    fn collection_resolution_validates_indirect_metadata_without_guessing_unicode() {
        let mut pdf = PdfDocument::new();
        let registry = pdf.add_object(Object::string_literal("Adobe"));
        let ordering = pdf.add_object(Object::string_literal("GB1"));
        let supplement = pdf.add_object(Object::Integer(6));
        let info = pdf.add_object(dictionary! {"Registry" => registry, "Ordering" => ordering, "Supplement" => supplement});
        let descendant = dictionary! {"CIDSystemInfo" => info};
        assert_eq!(
            String::from_utf16(
                &for_descendant(&pdf, &descendant)
                    .unwrap()
                    .units(4559)
                    .unwrap()
            )
            .unwrap(),
            "中"
        );
        for (name, value) in [
            ("Registry", Object::string_literal("Vendor")),
            ("Registry", Object::Name(b"Adobe".to_vec())),
            ("Ordering", Object::string_literal("Identity")),
            ("Ordering", Object::Name(b"GB1".to_vec())),
            ("Supplement", (-1).into()),
            ("Supplement", Object::Real(6.0)),
            ("Supplement", Object::Reference((999, 0))),
        ] {
            let mut bad = pdf.clone();
            bad.get_object_mut(info)
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set(name, value);
            assert!(for_descendant(&bad, &descendant).is_err(), "{name}");
        }
        assert!(for_descendant(&pdf, &Dictionary::new()).is_err());
        assert!(for_descendant(&pdf, &dictionary! {"CIDSystemInfo" => Object::Null}).is_err());
    }
}
