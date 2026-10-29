//! PDF 32000-1, 8.7.4 and 11.6.7: shading pattern paint sources.
//!
//! Evaluate in untransformed shading space, then transform the complete paint.
//! This preserves affine radial gradients and axial isocolor lines. A single
//! nonrepeating cell covers the inverse image of the page's visible rectangle.

use super::*;
use crate::{PatternCell, PatternReflectMethod, PatternRelativeTo, PatternStyle};

pub(super) struct Source {
    shading: Object,
    resources: Object,
    parent: Arc<GraphicsState>,
    matrix: [f64; 6],
    ext_gstate: Option<lopdf::Dictionary>,
    cells: std::sync::Mutex<HashMap<[u64; 4], String>>,
}

fn invalid(message: &str) -> OfdError {
    OfdError::Invalid(format!("PDF shading Pattern {message}"))
}

impl Source {
    pub(super) fn resolve(
        page: &PdfPageContext<'_>,
        value: &Object,
        parent: Arc<GraphicsState>,
    ) -> Result<Self, OfdError> {
        let dict = value
            .as_dict()
            .map_err(|_| invalid("must be a dictionary"))?;
        let shading = pdf_resolved_value(
            Some(page.pdf),
            dict.get(b"Shading")
                .map_err(|_| invalid("has no Shading"))?,
            "Pattern Shading",
        )?;
        if !matches!(shading, Object::Dictionary(_) | Object::Stream(_)) {
            return Err(invalid("Shading is not a dictionary or stream"));
        }
        let matrix = match dict.get(b"Matrix") {
            Ok(value) => pdf_numeric_array(page.pdf, value, "Pattern Matrix", 6)?
                .try_into()
                .map_err(|_| invalid("has invalid Matrix"))?,
            Err(_) => [1., 0., 0., 1., 0., 0.],
        };
        if crate::inverse_matrix(matrix).is_none() {
            return Err(invalid("has a singular Matrix"));
        }
        let ext_gstate = dict
            .get(b"ExtGState")
            .ok()
            .map(|value| {
                pdf_resolved_value(Some(page.pdf), value, "Pattern ExtGState")?
                    .as_dict()
                    .cloned()
                    .map_err(|_| invalid("ExtGState is not a dictionary"))
            })
            .transpose()?;
        Ok(Self {
            shading: shading.clone(),
            resources: page.resources.clone(),
            parent,
            matrix,
            ext_gstate,
            cells: std::sync::Mutex::new(HashMap::new()),
        })
    }

    fn graphics(&self, pdf: &PdfDocument) -> Result<(GraphicsState, f64), OfdError> {
        let mut state = (*self.parent).clone();
        state.ctm = [1., 0., 0., 1., 0., 0.];
        state.clips.clear();
        state.fill_alpha = 1.;
        state.stroke_alpha = 1.;
        state.blend_mode = "Normal";
        state.fill_pattern = None;
        state.stroke_pattern = None;
        if let Some(ext) = &self.ext_gstate {
            // Parameters affecting path stroking/text do not affect `sh`.
            let mut relevant = lopdf::Dictionary::new();
            for (key, value) in ext.iter() {
                if !matches!(
                    key.as_slice(),
                    b"Font" | b"LW" | b"LC" | b"LJ" | b"ML" | b"D" | b"FL" | b"CA" | b"SA" | b"TK"
                ) {
                    relevant.set(key.clone(), value.clone());
                }
            }
            apply_ext_gstate_dictionary(pdf, &relevant, &mut state, &mut TextState::default())?;
        }
        if state.blend_mode != "Normal" {
            return Err(invalid("with backdrop-dependent blending is unsupported"));
        }
        // Apply source alpha to the pattern as a whole, not independently to
        // its background and shading. A shading pattern is a knockout group.
        let alpha = state.fill_alpha;
        state.fill_alpha = 1.;
        Ok((state, alpha))
    }

    pub(super) fn require_opaque(&self, pdf: &PdfDocument) -> Result<(), OfdError> {
        let (_, alpha) = self.graphics(pdf)?;
        if alpha != 1. {
            return Err(invalid(
                "requires opaque content in this transparency group",
            ));
        }
        Ok(())
    }

    pub(super) fn style(
        &self,
        output: &mut OfdDocument,
        page: &PdfPageContext<'_>,
        current: &GraphicsState,
        alpha: f64,
    ) -> Result<PatternStyle, OfdError> {
        if current.active_patterns.len() >= 32 {
            return Err(OfdError::Limit("PDF Pattern nesting is too deep".into()));
        }
        let mut placement = (*self.parent).clone();
        placement.concat(self.matrix);
        let inverse = crate::inverse_matrix(placement.ctm)
            .ok_or_else(|| invalid("parent transform is singular"))?;
        let [a, b, c, d, e, f] = inverse;
        let corners = [
            (page.box_[0], page.box_[1]),
            (page.box_[2], page.box_[1]),
            (page.box_[2], page.box_[3]),
            (page.box_[0], page.box_[3]),
        ]
        .map(|(x, y)| (a * x + c * y + e, b * x + d * y + f));
        if corners
            .iter()
            .any(|(x, y)| !x.is_finite() || !y.is_finite() || x.abs() > 1e9 || y.abs() > 1e9)
        {
            return Err(OfdError::Limit(
                "PDF shading Pattern coverage exceeds coordinate range".into(),
            ));
        }
        let bbox = [
            corners.iter().map(|v| v.0).fold(f64::INFINITY, f64::min),
            corners.iter().map(|v| v.1).fold(f64::INFINITY, f64::min),
            corners
                .iter()
                .map(|v| v.0)
                .fold(f64::NEG_INFINITY, f64::max),
            corners
                .iter()
                .map(|v| v.1)
                .fold(f64::NEG_INFINITY, f64::max),
        ];
        let width = (bbox[2] - bbox[0]) * MM_PER_PT;
        let height = (bbox[3] - bbox[1]) * MM_PER_PT;
        let (state, source_alpha) = self.graphics(page.pdf)?;
        let key = page.box_.map(f64::to_bits);
        let cached = self
            .cells
            .lock()
            .map_err(|_| invalid("cell cache is unavailable"))?
            .get(&key)
            .cloned();
        let resource = if let Some(id) = cached {
            id
        } else {
            let id = self.cell_resource(output, page, bbox, width, height, &state)?;
            self.cells
                .lock()
                .map_err(|_| invalid("cell cache is unavailable"))?
                .insert(key, id.clone());
            id
        };
        let origin = placement.point(bbox[0], bbox[3], page.box_);
        let [a, b, c, d, _, _] = placement.ctm;
        let opacity = alpha * source_alpha;
        Ok(PatternStyle {
            width,
            height,
            x_step: None,
            y_step: None,
            reflect: PatternReflectMethod::Normal,
            relative_to: PatternRelativeTo::Object,
            ctm: Some([a, -b, -c, d, origin.0, origin.1]),
            alpha: (opacity < 1.).then_some((opacity * 255.).round() as u8),
            thumbnail: None,
            cells: vec![PatternCell::Composite {
                resource_id: resource,
                boundary: [0., 0., width, height],
            }],
        })
    }

    fn cell_resource(
        &self,
        output: &mut OfdDocument,
        page: &PdfPageContext<'_>,
        bbox: [f64; 4],
        width: f64,
        height: f64,
        state: &GraphicsState,
    ) -> Result<String, OfdError> {
        let scratch = output.pages()?.len();
        output.add_blank_page(width, height)?;
        let path = output.pages()?[scratch].path.clone();
        let context = PdfPageContext {
            pdf: page.pdf,
            box_: bbox,
            index: scratch,
            resources: &self.resources,
        };
        self.background(output, &context, state)?;
        import_pdf_shading_source(output, &context, state, &self.shading, true)?;
        let xml = {
            let bytes = output.required_entry(&path)?;
            let parsed = parse_xml(&path, bytes)?;
            let source =
                std::str::from_utf8(bytes).map_err(|_| invalid("cell XML is not UTF-8"))?;
            direct_page_objects(parsed.root_element())
                .map(|node| &source[node.range()])
                .collect::<String>()
        };
        let id =
            output.add_composite_graphic_resource(page.index, [width, height], |_, _, _| xml)?;
        output.remove_page(scratch)?;
        output.remove_entry(&path)?;
        Ok(id)
    }

    fn background(
        &self,
        output: &mut OfdDocument,
        page: &PdfPageContext<'_>,
        state: &GraphicsState,
    ) -> Result<(), OfdError> {
        let dict = match &self.shading {
            Object::Dictionary(dict) => dict,
            Object::Stream(stream) => &stream.dict,
            _ => return Err(invalid("Shading is not a dictionary or stream")),
        };
        let Ok(background) = dict.get(b"Background") else {
            return Ok(());
        };
        let space = parse_color_space(
            Some(page.pdf),
            Some(page.resources),
            dict.get(b"ColorSpace")
                .map_err(|_| invalid("Shading has no ColorSpace"))?,
            0,
        )?;
        if matches!(space, PdfColorSpace::Pattern { .. }) {
            return Err(invalid("Shading cannot use Pattern color"));
        }
        let values =
            pdf_numeric_array(page.pdf, background, "shading Background", space.channels())?;
        let rgb = space.color_with_intent(&values, state.rendering_intent)?;
        if space.nonpainting() {
            return Ok(());
        }
        let bbox = match dict.get(b"BBox") {
            Ok(value) => {
                let values = pdf_numeric_array(page.pdf, value, "shading BBox", 4)?;
                [values[0], values[1], values[2], values[3]]
            }
            Err(_) => page.box_,
        };
        if bbox[2] <= bbox[0] || bbox[3] <= bbox[1] {
            return Err(invalid("has invalid BBox"));
        }
        let corners = [
            (bbox[0], bbox[1]),
            (bbox[2], bbox[1]),
            (bbox[2], bbox[3]),
            (bbox[0], bbox[3]),
        ];
        let mut path = PathBuilder::default();
        path.move_to(state.point(corners[0].0, corners[0].1, page.box_));
        for (x, y) in &corners[1..] {
            path.line_to(state.point(*x, *y, page.box_))?;
        }
        path.close();
        let width = (page.box_[2] - page.box_[0]) * MM_PER_PT;
        let height = (page.box_[3] - page.box_[1]) * MM_PER_PT;
        output.add_path_object(
            page.index,
            &path.commands,
            [0., 0., width, height],
            PathStyle {
                fill: true,
                stroke: false,
                fill_color: Some(rgb),
                ..PathStyle::default()
            },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    const AXIAL: &[u8] = include_bytes!("../../tests/fixtures/shading-pattern-axial.pdf");

    fn changed(change: impl FnOnce(&mut PdfDocument)) -> Vec<u8> {
        let mut pdf = PdfDocument::load_mem(AXIAL).unwrap();
        change(&mut pdf);
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        bytes
    }

    fn pattern(pdf: &mut PdfDocument) -> &mut lopdf::Dictionary {
        pdf.get_object_mut((5, 0)).unwrap().as_dict_mut().unwrap()
    }

    fn rejects(bytes: &[u8], text: &str) {
        let error =
            OfdDocument::from_pdf_bytes(bytes).expect_err("invalid shading pattern was accepted");
        assert!(error.to_string().contains(text), "{error}: expected {text}");
    }

    fn context(pdf: &PdfDocument) -> PdfPageContext<'_> {
        let resources = pdf
            .get_dictionary((3, 0))
            .unwrap()
            .get(b"Resources")
            .unwrap();
        PdfPageContext {
            pdf,
            resources,
            index: 0,
            box_: [0., 0., 96., 64.],
        }
    }

    #[test]
    fn shading_patterns_require_dictionary_shading_and_valid_optional_dictionaries() {
        rejects(
            &changed(|pdf| {
                pattern(pdf).remove(b"Shading");
            }),
            "no Shading",
        );
        for value in [Object::Null, Object::Integer(3), Object::Array(Vec::new())] {
            rejects(
                &changed(|pdf| pattern(pdf).set("Shading", value)),
                "Shading is not",
            );
        }
        for value in [
            Object::Null,
            Object::Name(b"Normal".to_vec()),
            Object::Integer(0),
        ] {
            rejects(
                &changed(|pdf| pattern(pdf).set("ExtGState", value)),
                "ExtGState",
            );
        }
        let bytes = changed(|pdf| {
            *pdf.get_object_mut((5, 0)).unwrap() = Object::Stream(lopdf::Stream::new(
                dictionary! { "PatternType"=>2, "Shading"=>Object::Reference((6,0)) },
                Vec::new(),
            ));
        });
        rejects(&bytes, "must be a dictionary");
    }

    #[test]
    fn shading_patterns_reject_singular_and_malformed_matrices() {
        for value in [
            Object::Null,
            Object::Array(vec![0.into(); 6]),
            Object::Array(vec![1.into(); 5]),
        ] {
            rejects(&changed(|pdf| pattern(pdf).set("Matrix", value)), "Matrix");
        }
        rejects(
            &changed(|pdf| pattern(pdf).set("Matrix", Object::Reference((99, 0)))),
            "Pattern Matrix",
        );
    }

    #[test]
    fn shading_patterns_reject_an_underlying_pattern_color_space() {
        let bytes = changed(|pdf| {
            let page = pdf.get_object_mut((3, 0)).unwrap().as_dict_mut().unwrap();
            let resources = page.get_mut(b"Resources").unwrap().as_dict_mut().unwrap();
            resources.set("ColorSpace", dictionary! { "C" => vec![Object::Name(b"Pattern".to_vec()),Object::Name(b"DeviceGray".to_vec())] });
            pdf.get_object_mut((4, 0))
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .set_plain_content(b"/C cs 0 /P scn 4 4 88 56 re f".to_vec());
        });
        rejects(&bytes, "underlying color space");
    }

    #[test]
    fn shading_pattern_alpha_and_rendering_intent_start_in_the_correct_state() {
        let bytes = changed(|pdf| {
            pattern(pdf).set(
                "ExtGState",
                dictionary! {
                    "ca"=>Object::Real(0.5), "CA"=>Object::Real(0.1), "LW"=>100,
                    "Font"=>vec![Object::Reference((8,0)),12.into()], "RI"=>"Saturation",
                },
            )
        });
        let pdf = PdfDocument::load_mem(&bytes).unwrap();
        let parent = GraphicsState {
            fill_alpha: 0.25,
            stroke_alpha: 0.25,
            blend_mode: "Multiply",
            rendering_intent: RenderingIntent::Perceptual,
            ..GraphicsState::default()
        };
        let source = Source::resolve(
            &context(&pdf),
            pdf.get_object((5, 0)).unwrap(),
            Arc::new(parent),
        )
        .unwrap();
        let (state, alpha) = source.graphics(&pdf).unwrap();
        assert_eq!(alpha, 0.5);
        assert_eq!(state.fill_alpha, 1.);
        assert_eq!(state.stroke_alpha, 1.);
        assert_eq!(state.blend_mode, "Normal");
        assert_eq!(state.line_width, 1.);
        assert_eq!(state.rendering_intent, RenderingIntent::Saturation);
        assert!(source.require_opaque(&pdf).is_err());
    }

    #[test]
    fn shading_pattern_transparency_checks_reject_unsupported_or_invalid_parameters() {
        for alpha in [-0.1, 1.1] {
            rejects(
                &changed(|pdf| {
                    pattern(pdf).set("ExtGState", dictionary! { "ca"=>Object::Real(alpha) })
                }),
                "alpha is out of range",
            );
        }
        rejects(
            &changed(|pdf| pattern(pdf).set("ExtGState", dictionary! { "BM"=>"Multiply" })),
            "backdrop-dependent",
        );
        rejects(
            &changed(|pdf| pattern(pdf).set("ExtGState", dictionary! { "SMask"=>dictionary!{} })),
            "SMask",
        );
        let bytes = changed(|pdf| {
            pdf.get_object_mut((3, 0))
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set(
                    "Group",
                    dictionary! { "S"=>"Transparency", "CS"=>"DeviceRGB", "I"=>true },
                );
            pattern(pdf).set("ExtGState", dictionary! { "ca"=>Object::Real(0.5) });
        });
        rejects(&bytes, "requires opaque content");
    }

    #[test]
    fn shading_pattern_background_validates_components_and_clipping_box() {
        for value in [
            Object::Null,
            Object::Array(vec![0.into(); 2]),
            Object::Array(vec![2.into(); 3]),
        ] {
            let bytes = changed(|pdf| {
                pdf.get_object_mut((6, 0))
                    .unwrap()
                    .as_dict_mut()
                    .unwrap()
                    .set("Background", value)
            });
            assert!(OfdDocument::from_pdf_bytes(&bytes).is_err());
        }
        let bytes = changed(|pdf| {
            let shader = pdf.get_object_mut((6, 0)).unwrap().as_dict_mut().unwrap();
            shader.set("Background", vec![0.into(); 3]);
            shader.set("BBox", vec![0.into(); 4]);
        });
        rejects(&bytes, "BBox");
    }

    #[test]
    fn shading_pattern_coverage_and_recursion_limits_leave_the_document_unchanged() {
        let pdf = PdfDocument::load_mem(AXIAL).unwrap();
        let page = context(&pdf);
        let mut output = OfdDocument::new();
        output
            .add_blank_page(96. * MM_PER_PT, 64. * MM_PER_PT)
            .unwrap();
        let before = output.to_bytes().unwrap();
        let state = GraphicsState {
            active_patterns: vec![None; 32],
            ..GraphicsState::default()
        };
        let source = Source::resolve(
            &page,
            pdf.get_object((5, 0)).unwrap(),
            Arc::new(GraphicsState::default()),
        )
        .unwrap();
        assert!(source.style(&mut output, &page, &state, 1.).is_err());
        let source = Source::resolve(
            &page,
            pdf.get_object((5, 0)).unwrap(),
            Arc::new(GraphicsState {
                ctm: [1., 0., 0., 1., 2e9, 0.],
                ..GraphicsState::default()
            }),
        )
        .unwrap();
        assert!(matches!(
            source.style(&mut output, &page, &GraphicsState::default(), 1.),
            Err(OfdError::Limit(_))
        ));
        assert_eq!(output.to_bytes().unwrap(), before);
    }

    #[test]
    fn opaque_shading_patterns_are_accepted_in_opaque_transparency_groups() {
        let bytes = changed(|pdf| {
            pdf.get_object_mut((3, 0))
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set(
                    "Group",
                    dictionary! { "S"=>"Transparency", "CS"=>"DeviceRGB", "I"=>true },
                );
            pattern(pdf).set("ExtGState", dictionary! { "CA"=>Object::Real(0.5) });
        });
        assert!(OfdDocument::from_pdf_bytes(&bytes).is_ok());
    }
}
