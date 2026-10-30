//! Import the selected normal appearance using the ordinary Form interpreter.
use super::*;

fn invalid(message: &str) -> OfdError {
    OfdError::Invalid(format!("PDF annotation appearance {message}"))
}

fn normal<'a>(
    pdf: &'a PdfDocument,
    annotation: &'a lopdf::Dictionary,
) -> Result<&'a Object, OfdError> {
    let appearance = pdf_resolved_value(
        Some(pdf),
        annotation.get(b"AP").map_err(|_| invalid("is missing"))?,
        "annotation AP",
    )?
    .as_dict()
    .map_err(|_| invalid("AP is not a dictionary"))?;
    let normal = appearance
        .get(b"N")
        .map_err(|_| invalid("has no normal state"))?;
    let value = pdf_resolved_value(Some(pdf), normal, "annotation normal appearance")?;
    let selected = if let Object::Dictionary(states) = value {
        let state = pdf_resolved_value(
            Some(pdf),
            annotation
                .get(b"AS")
                .map_err(|_| invalid("state selection AS is missing"))?,
            "annotation AS",
        )?
        .as_name()
        .map_err(|_| invalid("AS is not a name"))?;
        states
            .get(state)
            .map_err(|_| invalid("selected state is missing"))?
    } else {
        normal
    };
    let stream = pdf_resolved_value(Some(pdf), selected, "annotation selected appearance")?
        .as_stream()
        .map_err(|_| invalid("normal state is not a stream"))?;
    if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form".as_slice()) {
        return Err(invalid("normal state is not a Form XObject"));
    }
    Ok(selected)
}

fn coordinates<const N: usize>(pdf: &PdfDocument, value: &Object) -> Result<[f64; N], OfdError> {
    let values = pdf_resolved_value(Some(pdf), value, "annotation appearance coordinates")?
        .as_array()
        .map_err(|_| invalid("coordinates are not an array"))?;
    if values.len() != N {
        return Err(invalid("coordinate count is invalid"));
    }
    let mut result = [0.; N];
    for (target, value) in result.iter_mut().zip(values) {
        *target = pdf_link_number(pdf, value)?;
    }
    Ok(result)
}

// ISO 32000-1 12.5.5: map the matrix-transformed BBox, rather than the
// untransformed BBox, into the annotation's normalized Rect.
fn placement(
    pdf: &PdfDocument,
    annotation: &lopdf::Dictionary,
    stream: &lopdf::Stream,
) -> Result<[f64; 6], OfdError> {
    let [x0, y0, x1, y1] = coordinates(
        pdf,
        stream
            .dict
            .get(b"BBox")
            .map_err(|_| invalid("has no BBox"))?,
    )?;
    if x1 <= x0 || y1 <= y0 {
        return Err(invalid("BBox is empty or reversed"));
    }
    let [a, b, c, d, e, f] = stream
        .dict
        .get(b"Matrix")
        .ok()
        .map(|value| coordinates(pdf, value))
        .transpose()?
        .unwrap_or([1., 0., 0., 1., 0., 0.]);
    let corners = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
        .map(|(x, y)| (a * x + c * y + e, b * x + d * y + f));
    let min_x = corners.iter().map(|v| v.0).fold(f64::INFINITY, f64::min);
    let max_x = corners
        .iter()
        .map(|v| v.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = corners.iter().map(|v| v.1).fold(f64::INFINITY, f64::min);
    let max_y = corners
        .iter()
        .map(|v| v.1)
        .fold(f64::NEG_INFINITY, f64::max);
    if max_x <= min_x || max_y <= min_y || a * d - b * c == 0. {
        return Err(invalid("Matrix collapses the BBox"));
    }
    let [rx0, ry0, rx1, ry1] = coordinates(
        pdf,
        annotation
            .get(b"Rect")
            .map_err(|_| invalid("has no Rect"))?,
    )?;
    let sx = (rx1 - rx0).abs() / (max_x - min_x);
    let sy = (ry1 - ry0).abs() / (max_y - min_y);
    let matrix = [
        sx,
        0.,
        0.,
        sy,
        rx0.min(rx1) - sx * min_x,
        ry0.min(ry1) - sy * min_y,
    ];
    if matrix.iter().any(|v| !v.is_finite() || v.abs() > 1e9) || sx <= 0. || sy <= 0. {
        return Err(invalid("placement is not representable"));
    }
    Ok(matrix)
}

pub(super) fn import(
    output: &mut OfdDocument,
    page_index: usize,
    pdf: &PdfDocument,
    page: &PdfPageImport,
    annotation: &lopdf::Dictionary,
    boundary: [f64; 4],
    existing: Option<&str>,
) -> Result<String, OfdError> {
    let selected = normal(pdf, annotation)?;
    let stream = pdf_resolved_value(Some(pdf), selected, "annotation selected appearance")?
        .as_stream()
        .map_err(|_| invalid("normal state is not a stream"))?;
    let matrix = placement(pdf, annotation, stream)?;
    let empty = Object::Dictionary(lopdf::Dictionary::new());
    let inherited = inherited_object(pdf, page.id, b"Resources")?.unwrap_or(&empty);
    let mut resources = pdf_resolved_value(Some(pdf), inherited, "annotation resources")?
        .as_dict()
        .map_err(|_| invalid("page resources are not a dictionary"))?
        .clone();
    let mut xobjects = match resources.get(b"XObject") {
        Ok(value) => pdf_resolved_value(Some(pdf), value, "annotation XObjects")?
            .as_dict()
            .map_err(|_| invalid("XObjects are not a dictionary"))?
            .clone(),
        Err(_) => lopdf::Dictionary::new(),
    };
    let mut name = "OfdNormalAppearance".to_owned();
    while xobjects.has(name.as_bytes()) {
        name.push('_');
    }
    xobjects.set(name.clone(), selected.clone());
    resources.set("XObject", xobjects);
    let resources = Object::Dictionary(resources);
    let mut graphics = GraphicsState {
        ctm: [page.user_unit, 0., 0., page.user_unit, 0., 0.],
        ..GraphicsState::default()
    };
    let gray = selected_color_space(pdf, &resources, b"DeviceGray")?;
    graphics.fill_color = gray.color(&[0.])?;
    graphics.stroke_color = graphics.fill_color;
    graphics.fill_space = gray.clone();
    graphics.stroke_space = gray;
    graphics.concat(matrix);
    let scratch = output.pages()?.len();
    output.add_blank_page(page.width_mm, page.height_mm)?;
    let path = output.pages()?[scratch].path.clone();
    let context = PdfPageContext {
        pdf,
        box_: page.box_,
        index: scratch,
        resources: &resources,
        raster_scale: 1.,
    };
    import_form(
        output,
        &context,
        &graphics,
        name.as_bytes(),
        &mut Vec::new(),
    )?;
    rotate_imported_page(
        output,
        scratch,
        page.rotation,
        page.width_mm,
        page.height_mm,
    )?;
    let children = {
        let parsed = parse_xml(&path, output.required_entry(&path)?)?;
        let source = parsed.input_text();
        direct_page_objects(parsed.root_element())
            .map(|node| &source[node.range()])
            .collect::<String>()
    };
    output.remove_page(scratch)?;
    output.remove_entry(&path)?;
    let size = if matches!(page.rotation, 90 | 270) {
        [page.height_mm, page.width_mm]
    } else {
        [page.width_mm, page.height_mm]
    };
    let resource = output.add_composite_graphic_resource(page_index, size, |_, _, _| children)?;
    let graphic = |prefix: &str, id: usize, _: f64, _: f64| {
        format!("<{prefix}CompositeObject ID=\"{id}\" ResourceID=\"{resource}\" Boundary=\"{} {} {} {}\"/>",-boundary[0],-boundary[1],size[0],size[1])
    };
    if let Some(id) = existing {
        // Use the annotation writer to allocate its object ID and update MaxUnitID,
        // then append it beside the invisible Link hit region and its action.
        let temporary = output.add_annotation_graphic(page_index, boundary, "Path", graphic)?;
        let annotations = output.annotations()?;
        let temp = annotations
            .iter()
            .find(|a| a.id == temporary)
            .ok_or_else(|| invalid("temporary annotation is missing"))?;
        let bytes = output.required_entry(&temp.source_path)?;
        let parsed = parse_xml(&temp.source_path, bytes)?;
        let node = parsed
            .descendants()
            .find(|n| is_ofd_element(*n, "Annot") && n.attribute("ID") == Some(temporary.as_str()))
            .ok_or_else(|| invalid("temporary annotation is missing"))?;
        let appearance = node
            .children()
            .find(|n| is_ofd_element(*n, "Appearance"))
            .ok_or_else(|| invalid("temporary Appearance is missing"))?;
        let xml = appearance
            .children()
            .map(|n| &parsed.input_text()[n.range()])
            .collect::<String>();
        output.remove_annotation(page_index, &temporary)?;
        output.edit_annotation_xml(page_index, id, |source, node| {
            let appearance = node
                .children()
                .find(|n| is_ofd_element(*n, "Appearance"))
                .ok_or_else(|| invalid("Link Appearance is missing"))?;
            crate::insert_xml_child(source, appearance, &xml)
        })?;
        Ok(id.to_owned())
    } else {
        output.add_annotation_graphic(page_index, boundary, "Stamp", graphic)
    }
}
