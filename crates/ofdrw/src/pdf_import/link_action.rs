//! Ordered URI/GoTo action trees, flattened into native CLICK actions.
use super::*;

enum Target {
    Uri(String),
    Goto(DestinationInfo),
}

pub(super) fn import(
    output: &mut OfdDocument,
    page_index: usize,
    pdf: &PdfDocument,
    action: &Object,
    pages: &[PdfPageImport],
    output_page_ids: &[String],
    geometry: ([f64; 4], &[[[f64; 2]; 4]]),
) -> Result<String, OfdError> {
    let targets = read(pdf, action, pages, output_page_ids)?;
    let (boundary, quads) = geometry;
    let id = match &targets[0] {
        Target::Uri(uri) => {
            output.add_uri_link_annotation_with_quads(page_index, uri, boundary, quads)?
        }
        Target::Goto(destination) => output.add_goto_link_annotation_with_quads(
            page_index,
            destination.clone(),
            boundary,
            quads,
        )?,
    };
    if targets.len() == 1 {
        return Ok(id);
    }
    output.edit_annotation_xml(page_index, &id, |source, annotation| {
        let actions = annotation
            .descendants()
            .find(|node| is_ofd_element(*node, "Actions"))
            .ok_or_else(|| invalid("native Link has no Actions"))?;
        let tag = qualified_tag(source, actions)?;
        let prefix = tag
            .strip_suffix("Actions")
            .ok_or_else(|| invalid("native Actions tag is invalid"))?;
        let region = actions
            .descendants()
            .find(|node| is_ofd_element(*node, "Region"))
            .map_or("", |region| &source[region.range()]);
        let mut xml = format!("<{tag}>");
        for target in &targets {
            let target = match target {
                Target::Uri(uri) => format!("<{prefix}URI URI=\"{}\"/>", escape_xml(uri)),
                Target::Goto(destination) => format!(
                    "<{prefix}Goto>{}</{prefix}Goto>",
                    crate::destination_xml(destination, prefix)
                ),
            };
            xml.push_str(&format!(
                "<{prefix}Action Event=\"CLICK\">{region}{target}</{prefix}Action>"
            ));
        }
        xml.push_str(&format!("</{tag}>"));
        let mut result = source.to_owned();
        result.replace_range(actions.range(), &xml);
        Ok(result)
    })?;
    output.set_annotation_parameter(page_index, &id, "PDFActionSequence", "true")?;
    Ok(id)
}

fn read(
    pdf: &PdfDocument,
    root: &Object,
    pages: &[PdfPageImport],
    output_page_ids: &[String],
) -> Result<Vec<Target>, OfdError> {
    enum Visit<'a> {
        Action(&'a Object, usize),
        Leave(ObjectId),
    }
    let mut stack = vec![Visit::Action(root, 0)];
    let mut active = HashSet::new();
    let mut targets = Vec::new();
    let mut uri_bytes = 0usize;
    while let Some(visit) = stack.pop() {
        let (value, depth) = match visit {
            Visit::Leave(id) => {
                active.remove(&id);
                continue;
            }
            Visit::Action(value, depth) => (value, depth),
        };
        if depth >= 32 || targets.len() >= 256 {
            return Err(OfdError::Limit(
                "PDF Link action tree exceeds 32 levels or 256 actions".into(),
            ));
        }
        let (id, value) = pdf
            .dereference(value)
            .map_err(|error| invalid(format!("action reference: {error}")))?;
        if let Some(id) = id {
            if !active.insert(id) {
                return Err(invalid("action tree contains a cycle"));
            }
            stack.push(Visit::Leave(id));
        }
        let action = value
            .as_dict()
            .map_err(|_| invalid("action is not a dictionary"))?;
        let kind = action
            .get(b"S")
            .map_err(|_| invalid("action has no type"))?;
        let kind = pdf_resolved_value(Some(pdf), kind, "Link action type")?
            .as_name()
            .map_err(|_| invalid("action type is not a name"))?;
        targets.push(match kind {
            b"URI" => {
                if let Ok(map) = action.get(b"IsMap") {
                    let map = pdf_resolved_value(Some(pdf), map, "URI IsMap")?
                        .as_bool()
                        .map_err(|_| invalid("URI IsMap is not boolean"))?;
                    if map {
                        return Err(invalid("URI IsMap mouse coordinates are unsupported"));
                    }
                }
                let uri = action
                    .get(b"URI")
                    .map_err(|_| invalid("URI action has no URI"))?;
                let raw = pdf_resolved_value(Some(pdf), uri, "Link URI")?
                    .as_str()
                    .map_err(|_| invalid("URI is not a string"))?;
                if raw.len().saturating_add(uri_bytes) > 1_000_000 {
                    return Err(OfdError::Limit(
                        "PDF Link action URIs exceed 1000000 bytes".into(),
                    ));
                }
                let uri = uri_action::resolve(pdf, &decode_pdf_link_uri(raw)?)?;
                uri_bytes = uri_bytes.saturating_add(uri.len());
                if uri_bytes > 1_000_000 {
                    return Err(OfdError::Limit(
                        "PDF Link action URIs exceed 1000000 bytes".into(),
                    ));
                }
                if uri.trim().is_empty() {
                    return Err(invalid("URI is empty"));
                }
                Target::Uri(uri)
            }
            b"GoTo" => {
                let value = action
                    .get(b"D")
                    .map_err(|_| invalid("GoTo action has no destination"))?;
                Target::Goto(pdf_link_destination(pdf, value, pages, output_page_ids)?)
            }
            _ => return Err(invalid("action type is not supported")),
        });
        if let Ok(next) = action.get(b"Next") {
            let next = pdf_resolved_value(Some(pdf), next, "Link Next")?;
            match next {
                Object::Array(actions) => {
                    if actions.len() > 256 {
                        return Err(OfdError::Limit("PDF Link Next exceeds 256 actions".into()));
                    }
                    stack.extend(
                        actions
                            .iter()
                            .rev()
                            .map(|action| Visit::Action(action, depth + 1)),
                    );
                }
                Object::Dictionary(_) => stack.push(Visit::Action(
                    action.get(b"Next").map_err(|_| invalid("missing Next"))?,
                    depth + 1,
                )),
                _ => return Err(invalid("Next is not an action dictionary or array")),
            }
        }
    }
    Ok(targets)
}

fn invalid(message: impl std::fmt::Display) -> OfdError {
    OfdError::Invalid(format!("PDF Link {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    fn uri(name: &str) -> Object {
        Object::Dictionary(
            dictionary! {"S" => "URI", "URI" => Object::string_literal(format!("https://example.org/{name}"))},
        )
    }

    fn targets(pdf: &PdfDocument, root: &Object) -> Result<Vec<String>, OfdError> {
        read(pdf, root, &[], &[]).map(|targets| {
            targets
                .into_iter()
                .map(|target| match target {
                    Target::Uri(value) => value,
                    Target::Goto(_) => panic!("unexpected GoTo"),
                })
                .collect()
        })
    }

    #[test]
    fn branches_execute_depth_first_and_shared_actions_are_not_deduplicated() {
        let mut pdf = PdfDocument::with_version("1.7");
        let shared = pdf.add_object(uri("shared"));
        let mut branch = uri("branch");
        branch.as_dict_mut().unwrap().set("Next", shared);
        let branch = pdf.add_object(branch);
        let mut root = uri("root");
        let array = pdf.add_object(Object::Array(vec![
            branch.into(),
            shared.into(),
            uri("last"),
        ]));
        root.as_dict_mut().unwrap().set("Next", array);
        assert_eq!(
            targets(&pdf, &root).unwrap(),
            ["root", "branch", "shared", "shared", "last"]
                .map(|name| format!("https://example.org/{name}"))
        );
    }

    #[test]
    fn action_cycles_are_rejected() {
        let mut pdf = PdfDocument::with_version("1.7");
        let root = pdf.add_object(uri("root"));
        let child = pdf.add_object(uri("child"));
        pdf.get_object_mut(root)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Next", child);
        pdf.get_object_mut(child)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Next", root);
        assert!(
            matches!(targets(&pdf, &root.into()), Err(OfdError::Invalid(message)) if message.contains("cycle"))
        );
    }

    #[test]
    fn malformed_or_unsupported_following_actions_are_not_dropped() {
        let pdf = PdfDocument::with_version("1.7");
        for next in [
            Object::Integer(1),
            Object::Reference((99, 0)),
            Object::Array(vec![Object::Integer(1)]),
            Object::Dictionary(dictionary! {"S" => "JavaScript"}),
        ] {
            let mut root = uri("root");
            root.as_dict_mut().unwrap().set("Next", next);
            assert!(targets(&pdf, &root).is_err());
        }
        let mut root = uri("root");
        root.as_dict_mut()
            .unwrap()
            .set("Next", Object::Array(vec![]));
        assert_eq!(targets(&pdf, &root).unwrap().len(), 1);
    }

    #[test]
    fn tree_depth_and_expanded_action_count_are_bounded() {
        let pdf = PdfDocument::with_version("1.7");
        for count in [32, 33] {
            let mut root = uri("last");
            for _ in 1..count {
                let mut parent = uri("parent");
                parent.as_dict_mut().unwrap().set("Next", root);
                root = parent;
            }
            let result = targets(&pdf, &root);
            if count == 32 {
                assert_eq!(result.unwrap().len(), count);
            } else {
                assert!(matches!(result, Err(OfdError::Limit(_))));
            }
        }
        for count in [256, 257] {
            let mut root = uri("root");
            root.as_dict_mut().unwrap().set(
                "Next",
                Object::Array((1..count).map(|_| uri("child")).collect()),
            );
            let result = targets(&pdf, &root);
            if count == 256 {
                assert_eq!(result.unwrap().len(), count);
            } else {
                assert!(matches!(result, Err(OfdError::Limit(_))));
            }
        }
    }

    #[test]
    fn uri_mouse_mapping_cannot_silently_become_a_static_link() {
        let mut pdf = PdfDocument::with_version("1.7");
        let map = pdf.add_object(Object::Boolean(false));
        let mut root = uri("root");
        root.as_dict_mut().unwrap().set("IsMap", map);
        assert_eq!(targets(&pdf, &root).unwrap().len(), 1);
        for value in [Object::Boolean(true), Object::Integer(1)] {
            root.as_dict_mut().unwrap().set("IsMap", value);
            assert!(targets(&pdf, &root).is_err());
        }
    }

    #[test]
    fn repeated_uri_payloads_have_a_total_budget() {
        let mut pdf = PdfDocument::with_version("1.7");
        let shared = pdf.add_object(uri(&"a".repeat(600_000)));
        let mut root = uri("root");
        root.as_dict_mut()
            .unwrap()
            .set("Next", Object::Array(vec![shared.into(), shared.into()]));
        assert!(matches!(targets(&pdf, &root), Err(OfdError::Limit(_))));
    }
}
