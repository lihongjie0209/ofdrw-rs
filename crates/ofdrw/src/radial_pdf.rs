//! Paint general radial tiles through vector stencils, without PDF tiling.
use super::*;
use std::ops::Range;

struct Field {
    image_id: String,
    transform: String,
    bounds: [f64; 4],
}

fn edits(source: &str, mut edits: Vec<(Range<usize>, String)>) -> Result<String, OfdError> {
    edits.sort_unstable_by_key(|edit| std::cmp::Reverse(edit.0.start));
    let mut result = source.to_owned();
    let mut limit = source.len();
    for (range, replacement) in edits {
        if range.end > limit || range.start > range.end {
            return Err(OfdError::Invalid("overlapping radial SVG edits".into()));
        }
        limit = range.start;
        result.replace_range(range, &replacement);
    }
    Ok(result)
}

fn attributes(
    source: &str,
    node: Node<'_, '_>,
    values: &[(&str, Option<&str>)],
) -> Result<String, OfdError> {
    let range = node.range();
    let local = &source[range.clone()];
    let mut updates = Vec::new();
    let mut added = String::new();
    for (name, value) in values {
        let replacement = value.map(|value| format!("{name}=\"{}\"", escape_xml(value)));
        if let Some(attribute) = node.attribute_node(*name) {
            let a = attribute.range();
            updates.push((
                a.start - range.start..a.end - range.start,
                replacement.unwrap_or_default(),
            ));
        } else if let Some(value) = replacement {
            added.push(' ');
            added.push_str(&value);
        }
    }
    if !added.is_empty() {
        let mut end = local
            .find('>')
            .ok_or_else(|| OfdError::Invalid("invalid radial SVG element".into()))?;
        if local.as_bytes()[end - 1] == b'/' {
            end -= 1;
        }
        updates.push((end..end, added));
    }
    edits(local, updates)
}

fn field<'a>(
    node: Node<'_, '_>,
    name: &str,
    fields: &'a HashMap<String, Field>,
) -> Option<&'a Field> {
    let id = node
        .attribute(name)?
        .strip_prefix("url(#")?
        .strip_suffix(')')?;
    fields.get(id)
}

pub(super) fn svg(source: String) -> Result<String, OfdError> {
    if !source.contains("<pattern id=\"ofd-shading-") {
        return Ok(source);
    }
    let parsed = roxmltree::Document::parse(&source)
        .map_err(|error| OfdError::Invalid(format!("invalid radial SVG: {error}")))?;
    let mut fields = HashMap::new();
    let mut updates = Vec::new();
    for pattern in parsed
        .descendants()
        .filter(|node| node.tag_name().name() == "pattern")
    {
        let Some(id) = pattern
            .attribute("id")
            .filter(|id| id.starts_with("ofd-shading-"))
        else {
            continue;
        };
        let transform = pattern.attribute("patternTransform").unwrap_or_default();
        let [a, _, _, d, x, y] = transform
            .strip_prefix("matrix(")
            .and_then(|s| s.strip_suffix(')'))
            .and_then(parse_numbers::<6>)
            .ok_or_else(|| OfdError::Invalid("invalid radial tile transform".into()))?;
        let width = svg_number(pattern.attribute("width"), f64::NAN)? * a;
        let height = svg_number(pattern.attribute("height"), f64::NAN)? * d;
        let image = pattern
            .children()
            .find(|node| node.tag_name().name() == "image")
            .ok_or_else(|| OfdError::Invalid("radial tile has no image".into()))?;
        let image_id = format!("{id}-image");
        updates.push((
            pattern.range().end..pattern.range().end,
            attributes(&source, image, &[("id", Some(&image_id))])?,
        ));
        fields.insert(
            id.to_owned(),
            Field {
                image_id,
                transform: transform.to_owned(),
                bounds: [x, y, width, height],
            },
        );
    }
    let mut serial = 0;
    for node in parsed
        .descendants()
        .filter(|node| matches!(node.tag_name().name(), "path" | "text"))
    {
        let fill = field(node, "fill", &fields);
        let stroke = field(node, "stroke", &fields);
        if fill.is_none() && stroke.is_none() {
            continue;
        }
        // Existing glyph-outline export already has an invisible semantic run.
        if node.ancestors().any(|parent| {
            parent
                .attribute("opacity")
                .and_then(|s| s.parse::<f64>().ok())
                == Some(0.)
        }) {
            updates.push((
                node.range(),
                attributes(
                    &source,
                    node,
                    &[("fill", Some("black")), ("stroke", Some("none"))],
                )?,
            ));
            continue;
        }
        let moved = ["transform", "opacity", "clip-path", "mask", "filter"];
        let group = moved
            .iter()
            .filter_map(|name| node.attribute_node(*name))
            .map(|attribute| format!(" {}", &source[attribute.range()]))
            .collect::<String>();
        let base: Vec<_> = moved.iter().map(|name| (*name, None)).collect();
        let mut result = format!("<g{group}>");
        let mut any_native = false;
        let paints = if node
            .attribute("paint-order")
            .is_some_and(|value| value.starts_with("stroke"))
        {
            [("stroke", stroke), ("fill", fill)]
        } else {
            [("fill", fill), ("stroke", stroke)]
        };
        for (name, field) in paints {
            let other = if name == "fill" { "stroke" } else { "fill" };
            let mut values = base.clone();
            if name == "fill" {
                values.push(("id", None));
            }
            values.push((other, Some("none")));
            if let Some(field) = field {
                values.push((name, Some("white")));
                let shape = attributes(&source, node, &values)?;
                let id = format!("ofd-radial-stencil-{serial}");
                serial += 1;
                let [x, y, width, height] = field.bounds;
                result.push_str(&format!("<defs><mask id=\"{id}\" maskUnits=\"userSpaceOnUse\" maskContentUnits=\"userSpaceOnUse\" x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" mask-type=\"alpha\">{shape}</mask></defs><g mask=\"url(#{id})\"><use href=\"#{}\" transform=\"{}\"/></g>", field.image_id, escape_xml(&field.transform)));
            } else if node.attribute(name).is_some_and(|paint| paint != "none") {
                any_native = true;
                result.push_str(&attributes(&source, node, &values)?);
            }
        }
        if node.tag_name().name() == "text" && !any_native {
            let mut values = base;
            values.extend([
                ("id", None),
                ("fill", Some("black")),
                ("fill-opacity", Some("1")),
                ("stroke", Some("none")),
            ]);
            result.push_str(&format!(
                "<g opacity=\"0\">{}</g>",
                attributes(&source, node, &values)?
            ));
        }
        result.push_str("</g>");
        updates.push((node.range(), result));
    }
    let result = edits(&source, updates)?;
    let parsed = roxmltree::Document::parse(&result)
        .map_err(|error| OfdError::Invalid(format!("invalid radial SVG stencil: {error}")))?;
    let unused = parsed
        .descendants()
        .filter(|node| node.tag_name().name() == "pattern")
        .filter(|node| {
            node.attribute("id").is_some_and(|id| {
                fields.contains_key(id) && !result.contains(&format!("url(#{id})"))
            })
        })
        .map(|node| (node.range(), String::new()))
        .collect();
    edits(&result, unused)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(body: &str) -> String {
        let png = STANDARD.encode(include_bytes!(
            "../tests/fixtures/canvas-images-java/source.png"
        ));
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\"><defs><pattern id=\"ofd-shading-0\" width=\"10\" height=\"10\" patternTransform=\"matrix(1 0 0 1 0 0)\"><image width=\"10\" height=\"10\" href=\"data:image/png;base64,{png}\"/></pattern></defs>{body}</svg>")
    }

    #[test]
    fn ordinary_svg_is_unchanged() {
        let source =
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M0 0L1 1\"/></svg>".to_owned();
        assert_eq!(svg(source.clone()).unwrap(), source);
    }

    #[test]
    fn mixed_text_paints_keep_one_native_semantic_run_and_xml_space() {
        let output = svg(source("<text xml:space=\"preserve\" transform=\"translate(2 3)\" opacity=\"0.5\" fill=\"url(#ofd-shading-0)\" stroke=\"red\">a &amp; b</text>")).unwrap();
        let parsed = roxmltree::Document::parse(&output).unwrap();
        assert!(!output.contains("url(#ofd-shading-0)"));
        assert!(!parsed
            .descendants()
            .any(|node| node.tag_name().name() == "pattern"));
        let text: Vec<_> = parsed
            .descendants()
            .filter(|node| node.tag_name().name() == "text")
            .collect();
        assert_eq!(text.len(), 2);
        assert!(text.iter().all(|node| node
            .attribute(("http://www.w3.org/XML/1998/namespace", "space"))
            == Some("preserve")
            && node.text() == Some("a & b")));
        let native: Vec<_> = text
            .into_iter()
            .filter(|node| {
                !node
                    .ancestors()
                    .any(|parent| parent.tag_name().name() == "mask")
            })
            .collect();
        assert_eq!(native.len(), 1);
        assert_eq!(native[0].attribute("stroke"), Some("red"));
        assert_eq!(native[0].attribute("fill"), Some("none"));
        let group = native[0].parent().unwrap();
        assert_eq!(group.attribute("transform"), Some("translate(2 3)"));
        assert_eq!(group.attribute("opacity"), Some("0.5"));
        assert!(native[0].attribute("transform").is_none());
    }

    #[test]
    fn existing_invisible_semantics_are_not_duplicated() {
        let output = svg(source(
            "<g opacity=\"0\"><text fill=\"url(#ofd-shading-0)\">logical</text></g>",
        ))
        .unwrap();
        let parsed = roxmltree::Document::parse(&output).unwrap();
        let text: Vec<_> = parsed
            .descendants()
            .filter(|node| node.tag_name().name() == "text")
            .collect();
        assert_eq!(text.len(), 1);
        assert_eq!(text[0].attribute("fill"), Some("black"));
        assert_eq!(text[0].text(), Some("logical"));
        assert!(!parsed
            .descendants()
            .any(|node| node.tag_name().name() == "mask"));
    }

    #[test]
    fn stroke_stencils_keep_hairlines_dashes_and_reuse_the_image() {
        let output = svg(source("<path id=\"ofd-device-hairline-7\" fill=\"red\" stroke=\"url(#ofd-shading-0)\" stroke-width=\"1\" stroke-dasharray=\"2 3\" d=\"M0 0L10 10\"/><path fill=\"url(#ofd-shading-0)\" stroke=\"none\" fill-rule=\"evenodd\" d=\"M1 1L9 1L9 9Z\"/>")).unwrap();
        let parsed = roxmltree::Document::parse(&output).unwrap();
        let ids: Vec<_> = parsed
            .descendants()
            .filter_map(|node| node.attribute("id"))
            .collect();
        assert_eq!(ids.len(), ids.iter().copied().collect::<HashSet<_>>().len());
        let hairline = parsed
            .descendants()
            .find(|node| node.attribute("id") == Some("ofd-device-hairline-7"))
            .unwrap();
        assert_eq!(hairline.attribute("stroke"), Some("white"));
        assert_eq!(hairline.attribute("fill"), Some("none"));
        assert_eq!(hairline.attribute("stroke-dasharray"), Some("2 3"));
        assert_eq!(
            parsed
                .descendants()
                .filter(|node| node.tag_name().name() == "image")
                .count(),
            1
        );
        assert_eq!(
            parsed
                .descendants()
                .filter(|node| node.tag_name().name() == "use")
                .count(),
            2
        );
        assert!(parsed
            .descendants()
            .any(|node| node.tag_name().name() == "path"
                && node.attribute("fill-rule") == Some("evenodd")
                && node.attribute("fill") == Some("white")));
    }
}
