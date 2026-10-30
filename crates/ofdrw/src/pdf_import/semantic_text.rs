//! Retain logical text when a PDF paint is replaced by a soft-mask raster.
use super::*;
use crate::{combine_text_transform, has_ofd_namespace, resource_id_key, visible_graphic_alpha};
use roxmltree::Node;

pub(super) struct Text {
    start: String,
    end: String,
}

impl Text {
    pub(super) fn with_id(&self, id: usize) -> String {
        format!("{}{id}{}", self.start, self.end)
    }
}

pub(super) fn from_composite(
    output: &OfdDocument,
    page_index: usize,
    object: Node<'_, '_>,
) -> Result<Vec<Text>, OfdError> {
    let pages = output.pages()?;
    let page = pages.get(page_index).ok_or(OfdError::PageIndex {
        index: page_index,
        count: pages.len(),
    })?;
    let units =
        output.page_resource_elements(&page.path, page.document_index, "CompositeGraphicUnit")?;
    let mut text = Vec::new();
    collect(object, &units, [1., 0., 0., 1., 0., 0.], 0, &mut text)?;
    Ok(text)
}

fn collect(
    node: Node<'_, '_>,
    units: &HashMap<String, String>,
    transform: [f64; 6],
    depth: usize,
    output: &mut Vec<Text>,
) -> Result<(), OfdError> {
    if depth >= 16 {
        return Err(OfdError::Limit(
            "composite object nesting exceeds 16".into(),
        ));
    }
    if node.is_element() && !has_ofd_namespace(node) {
        return Ok(());
    }
    if is_ofd_element(node, "Clip") || is_ofd_element(node, "Pattern") {
        return Ok(());
    }
    if ["TextObject", "PathObject", "ImageObject", "CompositeObject"]
        .iter()
        .any(|name| is_ofd_element(node, name))
        && visible_graphic_alpha(node)?.is_none()
    {
        return Ok(());
    }
    if is_ofd_element(node, "TextObject") {
        let transform = combine_text_transform(transform, node)?;
        let boundary = node
            .attribute("Boundary")
            .and_then(parse_box)
            .ok_or_else(|| OfdError::Invalid("semantic text has no valid Boundary".into()))?;
        let source = node.document().input_text();
        let tag = qualified_tag(source, node)?;
        let mut attrs = String::new();
        for attr in node.attributes() {
            if attr.namespace().is_none()
                && matches!(attr.name(), "ID" | "Fill" | "Stroke" | "Boundary" | "CTM")
            {
                continue;
            }
            attrs.push(' ');
            attrs.push_str(&source[attr.range()]);
        }
        let ctm = transform
            .iter()
            .map(f64::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        let children = node
            .children()
            .map(|n| &source[n.range()])
            .collect::<String>();
        // These fragments originate in this importer's native OFD resources,
        // which share the host page's ofd namespace. Keep glyph mappings and
        // local clipping but flatten placement, without editing shared units.
        output.push(Text {
            start: format!("<{tag} ID=\""),
            end: format!("\" Fill=\"false\" Stroke=\"false\" Boundary=\"0 0 {} {}\" CTM=\"{ctm}\"{attrs}>{children}</{tag}>", boundary[2], boundary[3]),
        });
        return Ok(());
    }
    if is_ofd_element(node, "CompositeObject") {
        let transform = combine_text_transform(transform, node)?;
        let id = node
            .attribute("ResourceID")
            .ok_or_else(|| OfdError::Invalid("CompositeObject has no ResourceID".into()))?;
        let xml = units
            .get(&resource_id_key(id))
            .ok_or_else(|| OfdError::Invalid(format!("composite resource {id} is not defined")))?;
        let parsed = parse_xml("semantic composite graphic unit", xml.as_bytes())?;
        let content = parsed
            .descendants()
            .find(|n| is_ofd_element(*n, "Content"))
            .ok_or_else(|| OfdError::Invalid(format!("composite resource {id} has no Content")))?;
        for child in content.children() {
            collect(child, units, transform, depth + 1, output)?;
        }
        return Ok(());
    }
    for child in node.children() {
        collect(child, units, transform, depth, output)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrapped(content: &str) -> String {
        format!("<ofd:Page xmlns:ofd=\"http://www.ofdspec.org/2016\">{content}</ofd:Page>")
    }

    #[test]
    fn semantic_text_flattens_affine_placement_without_losing_glyph_data() {
        let source = wrapped("<ofd:CompositeObject ID=\"1\" ResourceID=\"2\" Boundary=\"1 2 50 50\" CTM=\"2 0 0 3 5 7\"/>");
        let parsed = parse_xml("page", source.as_bytes()).unwrap();
        let unit = wrapped("<ofd:Content><ofd:TextObject ID=\"4\" Font=\"9\" Size=\"4\" Boundary=\"4 5 10 10\" CTM=\"1 0 0 1 2 3\" Fill=\"true\" Stroke=\"true\"><ofd:CGTransform CodePosition=\"0\" CodeCount=\"1\" GlyphCount=\"1\"><ofd:Glyphs>27</ofd:Glyphs></ofd:CGTransform><ofd:TextCode X=\"2\" Y=\"3\" DeltaX=\"4\">A&amp;B</ofd:TextCode></ofd:TextObject></ofd:Content>");
        let units = HashMap::from([("2".into(), unit.clone())]);
        let mut text = Vec::new();
        collect(
            parsed.root_element(),
            &units,
            [1., 0., 0., 1., 0., 0.],
            0,
            &mut text,
        )
        .unwrap();
        assert_eq!(text.len(), 1);
        let xml = wrapped(&text[0].with_id(100));
        let result = parse_xml("text", xml.as_bytes()).unwrap();
        let node = result
            .descendants()
            .find(|n| is_ofd_element(*n, "TextObject"))
            .unwrap();
        assert_eq!(node.attribute("CTM"), Some("2 0 0 3 18 33"));
        assert_eq!(node.attribute("ID"), Some("100"));
        assert_eq!(node.attribute("Boundary"), Some("0 0 10 10"));
        assert_eq!(node.attribute("Font"), Some("9"));
        assert_eq!(node.attribute("Fill"), Some("false"));
        assert_eq!(node.attribute("Stroke"), Some("false"));
        assert!(xml.contains("<ofd:Glyphs>27</ofd:Glyphs>"));
        assert!(xml.contains("DeltaX=\"4\">A&amp;B"));
        assert_eq!(units["2"], unit);
    }

    #[test]
    fn semantic_text_ignores_paint_definitions_and_invisible_objects() {
        let text = "<ofd:TextObject ID=\"1\" Font=\"2\" Size=\"4\" Boundary=\"0 0 10 10\"><ofd:TextCode>A</ofd:TextCode></ofd:TextObject>";
        let xml = wrapped(&format!("<ofd:Clip>{text}</ofd:Clip><ofd:Pattern>{text}</ofd:Pattern><ofd:PageBlock Visible=\"true\"><ofd:TextObject ID=\"4\" Alpha=\"0\"/>
        <ofd:CompositeObject ID=\"5\" Visible=\"false\"/></ofd:PageBlock><foreign xmlns=\"urn:custom\">{text}</foreign>"));
        let parsed = parse_xml("page", xml.as_bytes()).unwrap();
        let mut result = Vec::new();
        collect(
            parsed.root_element(),
            &HashMap::new(),
            [1., 0., 0., 1., 0., 0.],
            0,
            &mut result,
        )
        .unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn semantic_composite_cycles_have_the_renderers_nesting_limit() {
        let composite = "<ofd:CompositeObject ID=\"1\" ResourceID=\"2\" Boundary=\"0 0 10 10\"/>";
        let source = wrapped(composite);
        let parsed = parse_xml("page", source.as_bytes()).unwrap();
        let units = HashMap::from([(
            "2".into(),
            wrapped(&format!("<ofd:Content>{composite}</ofd:Content>")),
        )]);
        let error = collect(
            parsed.root_element(),
            &units,
            [1., 0., 0., 1., 0., 0.],
            0,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(
            matches!(error, OfdError::Limit(message) if message.contains("nesting exceeds 16"))
        );
    }

    #[test]
    fn semantic_composite_rejects_missing_resources() {
        let source =
            wrapped("<ofd:CompositeObject ID=\"1\" ResourceID=\"2\" Boundary=\"0 0 10 10\"/>");
        let parsed = parse_xml("page", source.as_bytes()).unwrap();
        let error = collect(
            parsed.root_element(),
            &HashMap::new(),
            [1., 0., 0., 1., 0., 0.],
            0,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(
            matches!(error, OfdError::Invalid(message) if message.contains("resource 2 is not defined"))
        );
    }
}
