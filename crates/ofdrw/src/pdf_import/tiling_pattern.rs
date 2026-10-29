//! PDF 32000-1, 8.7.2–8.7.3: vector tiling patterns.
//!
//! Pattern coordinates map to the parent's initial user space, independently
//! of subsequent `cm` operators. Each cell is imported once into a composite
//! resource; overlapping cells are normalized into one OFD repeat period.

use super::*;
use crate::{PatternCell, PatternReflectMethod, PatternRelativeTo, PatternStyle};

pub(super) struct Source {
    id: Option<ObjectId>,
    stream: lopdf::Stream,
    parent: Arc<GraphicsState>,
    bbox: [f64; 4],
    matrix: [f64; 6],
    step: [f64; 2],
    uncolored: bool,
    cells: std::sync::Mutex<HashMap<[u8; 3], String>>,
}

fn invalid(message: &str) -> OfdError {
    OfdError::Invalid(format!("PDF tiling Pattern {message}"))
}

impl Source {
    pub(super) fn resolve(
        page: &PdfPageContext<'_>,
        name: &[u8],
        parent: Arc<GraphicsState>,
        uncolored: bool,
    ) -> Result<Self, OfdError> {
        let (id, value) = resource(page.pdf, page.resources, b"Pattern", name)?;
        let stream = value.as_stream().map_err(|_| invalid("is not a stream"))?;
        let scalar = |key: &[u8]| -> Result<f64, OfdError> {
            let value = stream
                .dict
                .get(key)
                .map_err(|_| invalid("is missing a required entry"))?;
            let value = pdf_resolved_value(Some(page.pdf), value, "Pattern number")?;
            let number = f64::from(
                value
                    .as_float()
                    .map_err(|_| invalid("entry is not numeric"))?,
            );
            if !number.is_finite() {
                return Err(invalid("entry is not finite"));
            }
            Ok(number)
        };
        let integer = |key: &[u8]| -> Result<i64, OfdError> {
            let value = stream
                .dict
                .get(key)
                .map_err(|_| invalid("is missing a required integer"))?;
            pdf_resolved_value(Some(page.pdf), value, "Pattern integer")?
                .as_i64()
                .map_err(|_| invalid("entry is not an integer"))
        };
        if integer(b"PatternType")? != 1 {
            return Err(invalid("has unsupported PatternType"));
        }
        if integer(b"PaintType")? != if uncolored { 2 } else { 1 } {
            return Err(invalid("PaintType does not match its color space"));
        }
        if ![1, 2, 3].contains(&integer(b"TilingType")?) {
            return Err(invalid("has invalid TilingType"));
        }
        let array = |key: &[u8], count| -> Result<Vec<f64>, OfdError> {
            let value = stream
                .dict
                .get(key)
                .map_err(|_| invalid("is missing a required array"))?;
            let value = pdf_resolved_value(Some(page.pdf), value, "Pattern array")?;
            let values = value
                .as_array()
                .map_err(|_| invalid("entry is not an array"))?;
            let values = values
                .iter()
                .map(|value| {
                    pdf_resolved_value(Some(page.pdf), value, "Pattern array number").cloned()
                })
                .collect::<Result<Vec<_>, _>>()?;
            numbers(&Operation::new("Pattern array", values), count)
        };
        let bbox: [f64; 4] = array(b"BBox", 4)?
            .try_into()
            .map_err(|_| invalid("has invalid BBox"))?;
        if bbox[2] <= bbox[0] || bbox[3] <= bbox[1] || bbox.iter().any(|v| v.abs() > 1e9) {
            return Err(invalid("has invalid BBox"));
        }
        let step = [scalar(b"XStep")?, scalar(b"YStep")?].map(f64::abs);
        if step.iter().any(|v| *v == 0. || *v > 1e9) {
            return Err(invalid("has invalid step"));
        }
        let matrix = if stream.dict.get(b"Matrix").is_ok() {
            array(b"Matrix", 6)?
                .try_into()
                .map_err(|_| invalid("has invalid Matrix"))?
        } else {
            [1., 0., 0., 1., 0., 0.]
        };
        if crate::inverse_matrix(matrix).is_none() {
            return Err(invalid("has a singular Matrix"));
        }
        let resources = stream
            .dict
            .get(b"Resources")
            .map_err(|_| invalid("has no Resources"))?;
        pdf_resolved_value(Some(page.pdf), resources, "Pattern Resources")?
            .as_dict()
            .map_err(|_| invalid("Resources is not a dictionary"))?;
        // Bound overlapping copies before importing any cell content.
        overlap_counts(bbox, step)?;
        Ok(Self {
            id,
            stream: stream.clone(),
            parent,
            bbox,
            matrix,
            step,
            uncolored,
            cells: std::sync::Mutex::new(HashMap::new()),
        })
    }

    fn cell_resource(
        &self,
        output: &mut OfdDocument,
        page: &PdfPageContext<'_>,
        current: &GraphicsState,
        rgb: [u8; 3],
    ) -> Result<String, OfdError> {
        if current.active_patterns.len() >= 32 {
            return Err(OfdError::Limit("PDF Pattern nesting is too deep".into()));
        }
        if self.id.is_some() && current.active_patterns.contains(&self.id) {
            return Err(invalid("resource cycle"));
        }
        let key = if self.uncolored { rgb } else { [0; 3] };
        if let Some(id) = self
            .cells
            .lock()
            .map_err(|_| invalid("cell cache is unavailable"))?
            .get(&key)
        {
            return Ok(id.clone());
        }
        let resources = pdf_resolved_value(
            Some(page.pdf),
            self.stream
                .dict
                .get(b"Resources")
                .map_err(|_| invalid("has no Resources"))?,
            "Pattern Resources",
        )?;
        let bytes = pdf_byte_stream_data(&self.stream, Some(page.pdf), "Pattern content")?;
        let content = decode_pdf_content_with_inline_images(&bytes, page.pdf, resources)?;
        let width = (self.bbox[2] - self.bbox[0]) * MM_PER_PT;
        let height = (self.bbox[3] - self.bbox[1]) * MM_PER_PT;
        let scratch = output.pages()?.len();
        output.add_blank_page(width, height)?;
        let scratch_path = output.pages()?[scratch].path.clone();
        let mut state = (*self.parent).clone();
        state.ctm = [1., 0., 0., 1., 0., 0.];
        state.clips.clear();
        // PDF 32000-1, 11.6.7: a pattern starts with default transparency
        // parameters even when its parent Form inherited alpha or blending.
        state.fill_alpha = 1.;
        state.stroke_alpha = 1.;
        state.blend_mode = "Normal";
        state.active_patterns = current.active_patterns.clone();
        state.active_patterns.push(self.id);
        state.uncolored_pattern = self.uncolored;
        if self.uncolored {
            state.fill_space = PdfColorSpace::Rgb;
            state.stroke_space = PdfColorSpace::Rgb;
            state.fill_color = rgb;
            state.stroke_color = rgb;
            state.fill_components = rgb.map(|v| f64::from(v) / 255.).to_vec();
            state.stroke_components = state.fill_components.clone();
            state.fill_pattern = None;
            state.stroke_pattern = None;
        }
        let mut clip = PathBuilder::default();
        clip.move_to((0., 0.));
        clip.line_to((width, 0.))?;
        clip.line_to((width, height))?;
        clip.line_to((0., height))?;
        clip.close();
        install_clip(&mut state, &clip, false)?;
        let cell_page = PdfPageContext {
            pdf: page.pdf,
            box_: self.bbox,
            index: scratch,
            resources,
        };
        import_content(output, &cell_page, content, state, &mut Vec::new())?;
        let cell_xml = {
            let bytes = output.required_entry(&scratch_path)?;
            let parsed = parse_xml(&scratch_path, bytes)?;
            let source =
                std::str::from_utf8(bytes).map_err(|_| invalid("cell XML is not UTF-8"))?;
            direct_page_objects(parsed.root_element())
                .map(|node| &source[node.range()])
                .collect::<String>()
        };
        let resource_id =
            output
                .add_composite_graphic_resource(page.index, [width, height], |_, _, _| cell_xml)?;
        output.remove_page(scratch)?;
        output.remove_entry(&scratch_path)?;
        self.cells
            .lock()
            .map_err(|_| invalid("cell cache is unavailable"))?
            .insert(key, resource_id.clone());
        Ok(resource_id)
    }

    fn style(
        &self,
        output: &mut OfdDocument,
        page: &PdfPageContext<'_>,
        current: &GraphicsState,
        rgb: [u8; 3],
        alpha: f64,
    ) -> Result<PatternStyle, OfdError> {
        let resource_id = self.cell_resource(output, page, current, rgb)?;
        let width = (self.bbox[2] - self.bbox[0]) * MM_PER_PT;
        let height = (self.bbox[3] - self.bbox[1]) * MM_PER_PT;
        let [nx, ny] = overlap_counts(self.bbox, self.step)?;
        let [dx, dy] = self.step.map(|value| value * MM_PER_PT);
        let mut cells = Vec::with_capacity(nx * ny);
        for j in (0..ny).rev() {
            for i in (0..nx).rev() {
                cells.push(PatternCell::Composite {
                    resource_id: resource_id.clone(),
                    boundary: [-(i as f64) * dx, -(j as f64) * dy, width, height],
                });
            }
        }
        let mut placement = (*self.parent).clone();
        placement.concat(self.matrix);
        let origin = placement.point(self.bbox[0], self.bbox[3], page.box_);
        // Derive the linear terms analytically. Subtracting transformed points
        // introduces cancellation, which can round an exact 24-pixel SVG tile
        // just above 24 and make the rasterizer allocate a 25-pixel texture.
        let [a, b, c, d, _, _] = placement.ctm;
        let matrix = [a, -b, -c, d, origin.0, origin.1];
        if matrix.iter().any(|v| !v.is_finite()) {
            return Err(OfdError::Limit("PDF Pattern transform is too large".into()));
        }
        Ok(PatternStyle {
            width: dx,
            height: dy,
            x_step: None,
            y_step: None,
            reflect: PatternReflectMethod::Normal,
            relative_to: PatternRelativeTo::Object,
            ctm: Some(matrix),
            alpha: (alpha < 1.).then_some((alpha * 255.).round() as u8),
            thumbnail: None,
            cells,
        })
    }
}

fn overlap_counts(bbox: [f64; 4], step: [f64; 2]) -> Result<[usize; 2], OfdError> {
    let x = ((bbox[2] - bbox[0]) / step[0]).ceil().max(1.);
    let y = ((bbox[3] - bbox[1]) / step[1]).ceil().max(1.);
    if !x.is_finite() || !y.is_finite() || x * y > 1000. {
        return Err(OfdError::Limit(
            "PDF Pattern requires too many overlapping cells".into(),
        ));
    }
    Ok([x as usize, y as usize])
}

pub(super) fn require_selected(
    state: &GraphicsState,
    fill: bool,
    stroke: bool,
) -> Result<(), OfdError> {
    if (fill
        && matches!(state.fill_space, PdfColorSpace::Pattern { .. })
        && state.fill_pattern.is_none())
        || (stroke
            && matches!(state.stroke_space, PdfColorSpace::Pattern { .. })
            && state.stroke_pattern.is_none())
    {
        return Err(invalid("has no selected pattern"));
    }
    Ok(())
}

pub(super) fn attach(
    output: &mut OfdDocument,
    page: &PdfPageContext<'_>,
    object_id: &str,
    state: &GraphicsState,
    fill: bool,
    stroke: bool,
) -> Result<(), OfdError> {
    require_selected(state, fill, stroke)?;
    for (enabled, source, rgb, alpha, is_fill) in [
        (
            fill,
            state.fill_pattern.as_ref(),
            state.fill_color,
            state.fill_alpha,
            true,
        ),
        (
            stroke,
            state.stroke_pattern.as_ref(),
            state.stroke_color,
            state.stroke_alpha,
            false,
        ),
    ] {
        let Some(source) = source.filter(|_| enabled) else {
            continue;
        };
        let mut style = source.style(output, page, state, rgb, alpha)?;
        // Freeze page coordinates into the object's space. Later page rotation
        // must rotate the pattern together with the object, rather than cancel it.
        let object_matrix = {
            let path = &output.pages()?[page.index].path;
            let parsed = parse_xml(path, output.required_entry(path)?)?;
            let node = direct_page_objects(parsed.root_element())
                .find(|node| node.attribute("ID") == Some(object_id))
                .ok_or_else(|| invalid("paint object is missing"))?;
            let [x, y, _, _] = node
                .attribute("Boundary")
                .and_then(parse_box)
                .ok_or_else(|| invalid("paint boundary is invalid"))?;
            let mut matrix = match node.attribute("CTM") {
                Some(value) => parse_ctm(value).ok_or_else(|| invalid("paint CTM is invalid"))?,
                None => [1., 0., 0., 1., 0., 0.],
            };
            matrix[4] += x;
            matrix[5] += y;
            matrix
        };
        let inverse = crate::inverse_matrix(object_matrix)
            .ok_or_else(|| invalid("paint object has a singular CTM"))?;
        style.ctm = Some(crate::multiply_matrix(
            inverse,
            style.ctm.unwrap_or([1., 0., 0., 1., 0., 0.]),
        ));
        if is_fill {
            output.set_page_object_pattern_fill(page.index, object_id, &style)?;
        } else {
            output.set_page_object_pattern_stroke(page.index, object_id, &style)?;
        }
    }
    Ok(())
}

pub(super) fn validate_operator(
    operation: &Operation,
    state: &GraphicsState,
    page: &PdfPageContext<'_>,
) -> Result<(), OfdError> {
    if state.uncolored_pattern
        && matches!(
            operation.operator.as_str(),
            "g" | "G"
                | "rg"
                | "RG"
                | "k"
                | "K"
                | "cs"
                | "CS"
                | "sc"
                | "SC"
                | "scn"
                | "SCN"
                | "sh"
                | "ri"
        )
    {
        return Err(invalid("uncolored content specifies a color"));
    }
    if state.uncolored_pattern && operation.operator == "gs" {
        let [Object::Name(name)] = operation.operands.as_slice() else {
            return Err(invalid("uncolored gs has no resource name"));
        };
        let (_, value) = resource(page.pdf, page.resources, b"ExtGState", name)?;
        let dict = value
            .as_dict()
            .map_err(|_| invalid("ExtGState is not a dictionary"))?;
        if [
            b"TR".as_slice(),
            b"TR2",
            b"BG",
            b"BG2",
            b"UCR",
            b"UCR2",
            b"HT",
        ]
        .iter()
        .any(|key| dict.get(key).is_ok())
        {
            return Err(invalid("uncolored ExtGState specifies a color parameter"));
        }
    }
    Ok(())
}

pub(super) fn paint_mask(
    output: &mut OfdDocument,
    page: &PdfPageContext<'_>,
    state: &GraphicsState,
    image: &lopdf::Stream,
) -> Result<(), OfdError> {
    require_selected(state, true, false)?;
    if state.fill_space.nonpainting() {
        return Ok(());
    }
    let (mask, _) = decode_stencil_mask(image, page.pdf)?;
    let (width, height) = mask.dimensions();
    let mut path = PathBuilder::default();
    let mut runs = 0_usize;
    for y in 0..height {
        let mut x = 0;
        while x < width {
            if mask.get_pixel(x, y)[0] == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < width && mask.get_pixel(x, y)[0] != 0 {
                x += 1;
            }
            runs += 1;
            if runs > 100_000 {
                return Err(OfdError::Limit(
                    "PDF Pattern stencil has too many runs".into(),
                ));
            }
            let x0 = f64::from(start) / f64::from(width);
            let x1 = f64::from(x) / f64::from(width);
            let y0 = 1. - f64::from(y) / f64::from(height);
            let y1 = 1. - f64::from(y + 1) / f64::from(height);
            path.move_to(state.point(x0, y0, page.box_));
            path.line_to(state.point(x1, y0, page.box_))?;
            path.line_to(state.point(x1, y1, page.box_))?;
            path.line_to(state.point(x0, y1, page.box_))?;
            path.close();
        }
    }
    paint(
        output,
        page,
        &mut path,
        state,
        PdfPaint {
            fill: true,
            stroke: false,
            even_odd: false,
        },
        true,
    )
}

// Keep the existing opaque transparency-group contract when a pattern hides
// nested forms, image masks, or ExtGState alpha inside its cell content.
pub(super) fn validate_opaque(
    pdf: &PdfDocument,
    resources: &Object,
    name: &[u8],
    active: &mut Vec<ObjectId>,
) -> Result<(), OfdError> {
    let (id, value) = resource(pdf, resources, b"Pattern", name)?;
    if active.len() >= 32 {
        return Err(OfdError::Limit("PDF Pattern nesting is too deep".into()));
    }
    if id.is_some_and(|id| active.contains(&id)) {
        return Err(invalid("resource cycle"));
    }
    let stream = value.as_stream().map_err(|_| invalid("is not a stream"))?;
    let resources = pdf_resolved_value(
        Some(pdf),
        stream
            .dict
            .get(b"Resources")
            .map_err(|_| invalid("has no Resources"))?,
        "Pattern Resources",
    )?;
    let bytes = pdf_byte_stream_data(stream, Some(pdf), "Pattern content")?;
    let content = decode_pdf_content_with_inline_images(&bytes, pdf, resources)?;
    if let Some(id) = id {
        active.push(id);
    }
    let result = validate_opaque_form_content(pdf, resources, &content, active);
    if id.is_some() {
        active.pop();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    const COLORED: &[u8] = include_bytes!("../../tests/fixtures/tiling-colored.pdf");
    const UNCOLORED: &[u8] = include_bytes!("../../tests/fixtures/tiling-uncolored-rgb.pdf");

    fn changed(source: &[u8], change: impl FnOnce(&mut PdfDocument)) -> Vec<u8> {
        let mut pdf = PdfDocument::load_mem(source).unwrap();
        change(&mut pdf);
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        bytes
    }

    fn stream(pdf: &mut PdfDocument, id: u32) -> &mut lopdf::Stream {
        pdf.get_object_mut((id, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
    }

    fn rejects(source: &[u8], message: &str) {
        let error = OfdDocument::from_pdf_bytes(source).expect_err("invalid pattern was accepted");
        assert!(
            error.to_string().contains(message),
            "{error}: expected {message}"
        );
    }

    #[test]
    fn tiling_rejects_missing_and_mistyped_dictionary_entries() {
        for key in [
            b"PatternType".as_slice(),
            b"PaintType",
            b"TilingType",
            b"BBox",
            b"XStep",
            b"YStep",
            b"Resources",
        ] {
            let bytes = changed(COLORED, |pdf| {
                stream(pdf, 5).dict.remove(key);
            });
            rejects(&bytes, "Pattern");
        }
        for key in [b"PatternType".as_slice(), b"PaintType", b"TilingType"] {
            let bytes = changed(COLORED, |pdf| {
                stream(pdf, 5).dict.set(key, Object::Real(1.5));
            });
            rejects(&bytes, "integer");
        }
    }

    #[test]
    fn tiling_rejects_invalid_geometry_and_excessive_overlap() {
        for (key, value) in [
            (b"XStep".as_slice(), Object::Integer(0)),
            (b"YStep".as_slice(), Object::Integer(0)),
            (b"Matrix".as_slice(), Object::Array(vec![0.into(); 6])),
            (b"BBox".as_slice(), Object::Array(vec![0.into(); 4])),
        ] {
            rejects(
                &changed(COLORED, |pdf| stream(pdf, 5).dict.set(key, value)),
                "Pattern",
            );
        }
        rejects(
            &changed(COLORED, |pdf| {
                stream(pdf, 5).dict.set("XStep", Object::Real(0.0001))
            }),
            "too many overlapping",
        );
    }

    #[test]
    fn tiling_rejects_mismatched_paint_types() {
        rejects(
            &changed(COLORED, |pdf| stream(pdf, 5).dict.set("PaintType", 2)),
            "PaintType",
        );
        rejects(
            &changed(UNCOLORED, |pdf| stream(pdf, 5).dict.set("PaintType", 1)),
            "PaintType",
        );
    }

    #[test]
    fn tiling_rejects_cycles_across_pattern_and_form_streams() {
        let bytes = changed(COLORED, |pdf| {
            stream(pdf, 5).dict.set(
                "Resources",
                dictionary! { "Pattern" => dictionary! { "P" => Object::Reference((5,0)) } },
            );
            stream(pdf, 5).set_plain_content(b"/Pattern cs /P scn 0 0 8 8 re f".to_vec());
        });
        rejects(&bytes, "cycle");
        let bytes = changed(COLORED, |pdf| {
            stream(pdf, 5).dict.set(
                "Resources",
                dictionary! { "XObject" => dictionary! { "F" => Object::Reference((8,0)) } },
            );
            stream(pdf, 5).set_plain_content(b"/F Do".to_vec());
        });
        rejects(&bytes, "cycle");
    }

    #[test]
    fn uncolored_tiling_rejects_color_operators_and_images() {
        for content in [
            "0 g",
            "0 G",
            "1 0 0 rg",
            "1 0 0 RG",
            "0 0 0 1 k",
            "0 0 0 1 K",
            "/DeviceRGB cs",
            "/DeviceRGB CS",
            "0 sc",
            "0 SC",
            "0 scn",
            "0 SCN",
            "/X sh",
            "/Perceptual ri",
            "/I Do",
        ] {
            let bytes = changed(UNCOLORED, |pdf| {
                stream(pdf, 5).dict.set(
                    "Resources",
                    dictionary! { "XObject" => dictionary! { "I" => Object::Reference((11,0)) } },
                );
                stream(pdf, 5).set_plain_content(content.as_bytes().to_vec())
            });
            rejects(&bytes, "uncolored");
        }
    }

    #[test]
    fn uncolored_tiling_rejects_color_parameters_inside_extgstate() {
        for key in ["TR", "TR2", "BG", "BG2", "UCR", "UCR2", "HT"] {
            let bytes = changed(UNCOLORED, |pdf| {
                let mut dict = lopdf::Dictionary::new();
                dict.set(key, Object::Name(b"Identity".to_vec()));
                stream(pdf, 5).dict.set(
                    "Resources",
                    dictionary! { "ExtGState" => dictionary! { "G" => dict } },
                );
                stream(pdf, 5).set_plain_content(b"/G gs 0 0 8 8 re f".to_vec());
            });
            rejects(&bytes, "uncolored ExtGState");
        }
    }

    #[test]
    fn pattern_spaces_are_forbidden_in_sampled_images_and_as_pattern_bases() {
        let nested = Object::Array(vec![
            Object::Name(b"Pattern".to_vec()),
            Object::Name(b"Pattern".to_vec()),
        ]);
        assert!(parse_color_space(None, None, &nested, 0).is_err());
        let bytes = changed(COLORED, |pdf| {
            stream(pdf, 4).set_plain_content(b"q 20 0 0 20 0 0 cm /I Do Q".to_vec());
            stream(pdf, 11)
                .dict
                .set("ColorSpace", Object::Name(b"Pattern".to_vec()));
        });
        rejects(&bytes, "image cannot use Pattern");
    }

    #[test]
    fn pattern_requires_selection_before_painting_and_a_name_after_components() {
        for content in [
            "/Pattern cs 0 0 10 10 re f",
            "/Pattern cs 0 scn",
            "/Pattern cs /P sc",
        ] {
            let bytes = changed(COLORED, |pdf| {
                stream(pdf, 4).set_plain_content(content.as_bytes().to_vec())
            });
            rejects(&bytes, "Pattern");
        }
    }

    #[test]
    fn tiling_preserves_opaque_transparency_group_validation() {
        let group = || dictionary! { "S"=>"Transparency", "CS"=>"DeviceRGB", "I"=>true };
        let bytes = changed(COLORED, |pdf| {
            pdf.get_object_mut((3, 0))
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Group", group());
        });
        assert!(OfdDocument::from_pdf_bytes(&bytes).is_ok());
        let bytes = changed(COLORED, |pdf| {
            pdf.get_object_mut((3, 0))
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Group", group());
            stream(pdf, 5).dict.set(
                "Resources",
                dictionary! { "ExtGState" => dictionary! { "A" => Object::Reference((10,0)) } },
            );
            stream(pdf, 5).set_plain_content(b"/A gs 1 0 0 rg 0 0 8 8 re f".to_vec());
        });
        rejects(&bytes, "requires opaque content");
    }

    #[test]
    fn tiling_rejects_backdrop_dependent_cell_blending() {
        let bytes = changed(COLORED, |pdf| {
            stream(pdf, 5).dict.set("Resources", dictionary! { "ExtGState" => dictionary! { "B" => dictionary! { "BM" => "Multiply" } } });
            stream(pdf, 5).set_plain_content(b"/B gs 1 0 0 rg 0 0 8 8 re f".to_vec());
        });
        rejects(&bytes, "non-Normal blending");
    }

    #[test]
    fn nonpainting_underlying_colors_do_not_legalize_pattern_sample_spaces() {
        let space = || {
            Object::Array(vec![
                Object::Name(b"Pattern".to_vec()),
                Object::Array(vec![
                    Object::Name(b"Separation".to_vec()),
                    Object::Name(b"None".to_vec()),
                    Object::Name(b"DeviceGray".to_vec()),
                    Object::Null,
                ]),
            ])
        };
        for source in [
            include_bytes!("../../tests/fixtures/plane-calculator-rgb.pdf").as_slice(),
            include_bytes!("../../tests/fixtures/mesh-free-rgb.pdf").as_slice(),
        ] {
            let bytes = changed(source, |pdf| {
                let object = pdf.get_object_mut((5, 0)).unwrap();
                let dict = match object {
                    Object::Dictionary(dict) => dict,
                    Object::Stream(stream) => &mut stream.dict,
                    _ => panic!("fixture has no shading dictionary"),
                };
                dict.set("ColorSpace", space());
            });
            rejects(&bytes, "Pattern color");
        }
    }

    #[test]
    fn overlapping_cells_are_bounded_without_clamping_the_repeat_period() {
        assert_eq!(
            overlap_counts([0., 0., 12., 10.], [4., 6.]).unwrap(),
            [3, 2]
        );
        assert_eq!(
            overlap_counts([2., 3., 10., 11.], [12., 12.]).unwrap(),
            [1, 1]
        );
        assert!(overlap_counts([0., 0., 1., 1.], [1e-300, 1.]).is_err());
    }
}
