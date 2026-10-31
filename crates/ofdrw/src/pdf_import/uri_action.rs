//! Preserve the target of relative PDF URI actions when a catalog base exists.
use super::*;
use oxiri::{Iri, IriRef};

pub(super) fn resolve(pdf: &PdfDocument, target: &str) -> Result<String, OfdError> {
    // An absolute target does not depend on the catalog's optional URI metadata.
    if Iri::parse(target).is_ok() {
        return Ok(target.to_owned());
    }
    let catalog = pdf
        .catalog()
        .map_err(|error| invalid(format!("catalog: {error}")))?;
    let Ok(dictionary) = catalog.get(b"URI") else {
        return Ok(target.to_owned());
    };
    let dictionary = pdf_resolved_value(Some(pdf), dictionary, "URI dictionary")?
        .as_dict()
        .map_err(|_| invalid("catalog URI is not a dictionary"))?;
    let Ok(base) = dictionary.get(b"Base") else {
        // Byte imports have no retrieval URL from which to infer a default base.
        return Ok(target.to_owned());
    };
    let raw = pdf_resolved_value(Some(pdf), base, "URI Base")?
        .as_str()
        .map_err(|_| invalid("Base is not a string"))?;
    if raw.len().saturating_add(target.len()) > 1_000_000 {
        return Err(OfdError::Limit(
            "PDF URI resolution exceeds 1000000 bytes".into(),
        ));
    }
    let base = decode_pdf_link_uri(raw)?;
    if base.len().saturating_add(target.len()) > 1_000_000 {
        return Err(OfdError::Limit(
            "PDF URI resolution exceeds 1000000 bytes".into(),
        ));
    }
    let base = Iri::parse(base).map_err(|error| invalid(format!("invalid Base: {error}")))?;
    let reference = IriRef::parse(target)
        .map_err(|error| invalid(format!("invalid relative target: {error}")))?;
    base.resolve(&reference)
        .map(Iri::into_inner)
        .map_err(|error| invalid(format!("cannot resolve target: {error}")))
}

fn invalid(message: impl std::fmt::Display) -> OfdError {
    OfdError::Invalid(format!("PDF URI Link {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    fn document(uri: Object) -> PdfDocument {
        let mut pdf = PdfDocument::with_version("2.0");
        let root = pdf.add_object(dictionary! {"Type" => "Catalog", "URI" => uri});
        pdf.trailer.set("Root", root);
        pdf
    }

    #[test]
    fn resolves_paths_queries_fragments_and_authorities() {
        let pdf = document(Object::Dictionary(
            dictionary! {"Base" => Object::string_literal("https://example.org/a/b.pdf?old=1")},
        ));
        for (target, expected) in [
            ("next", "https://example.org/a/next"),
            ("../next", "https://example.org/next"),
            ("/next", "https://example.org/next"),
            ("//other.example/next", "https://other.example/next"),
            ("?q=2", "https://example.org/a/b.pdf?q=2"),
            ("#part", "https://example.org/a/b.pdf?old=1#part"),
            ("", "https://example.org/a/b.pdf?old=1"),
            ("../../../../next", "https://example.org/next"),
            (
                "./x/../next?q=/../#./",
                "https://example.org/a/next?q=/../#./",
            ),
            ("%2E%2E/next", "https://example.org/a/%2E%2E/next"),
            ("报告?字=值#段", "https://example.org/a/报告?字=值#段"),
            ("mailto:user@example.org", "mailto:user@example.org"),
        ] {
            assert_eq!(resolve(&pdf, target).unwrap(), expected, "{target}");
        }
    }

    #[test]
    fn resolves_indirect_dictionary_and_utf16_base() {
        let mut pdf = document(Object::Null);
        let mut raw = vec![0xfe, 0xff];
        for unit in "https://example.org/文档/".encode_utf16() {
            raw.extend_from_slice(&unit.to_be_bytes());
        }
        let base = pdf.add_object(Object::String(raw, lopdf::StringFormat::Hexadecimal));
        let uri = pdf.add_object(dictionary! {"Base" => base});
        pdf.catalog_mut().unwrap().set("URI", uri);
        assert_eq!(resolve(&pdf, "页").unwrap(), "https://example.org/文档/页");
    }

    #[test]
    fn absent_base_and_absolute_targets_keep_their_original_bytes() {
        for uri in [Object::Dictionary(dictionary! {}), Object::Integer(1)] {
            let pdf = document(uri);
            assert_eq!(
                resolve(&pdf, "https://Example.org/a/../b").unwrap(),
                "https://Example.org/a/../b"
            );
        }
        let pdf = document(Object::Dictionary(dictionary! {}));
        assert_eq!(resolve(&pdf, "../relative").unwrap(), "../relative");
        let mut pdf = pdf;
        pdf.catalog_mut().unwrap().remove(b"URI");
        assert_eq!(resolve(&pdf, "../relative").unwrap(), "../relative");
    }

    #[test]
    fn rejects_invalid_base_dictionary_references_and_resolution_input() {
        for uri in [
            Object::Integer(1),
            Object::Reference((99, 0)),
            Object::Dictionary(dictionary! {"Base" => 1}),
            Object::Dictionary(dictionary! {"Base" => Object::Reference((99, 0))}),
            Object::Dictionary(dictionary! {"Base" => Object::string_literal("../relative")}),
            Object::Dictionary(dictionary! {"Base" => Object::string_literal("https://bad host/")}),
        ] {
            assert!(resolve(&document(uri), "relative").is_err());
        }
        let pdf = document(Object::Dictionary(
            dictionary! {"Base" => Object::string_literal("https://example.org/")},
        ));
        assert!(resolve(&pdf, "bad%escape").is_err());
        assert!(matches!(
            resolve(&pdf, &"x".repeat(1_000_000)),
            Err(OfdError::Limit(_))
        ));
    }
}
