//! PDF graphics-state soft masks, sampled on a bounded transparent canvas.
use super::*;
use lopdf::dictionary;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};

const MAX_PIXELS: f64 = 4_000_000.;
const MAX_WORK: u64 = 256_000_000;

struct Raster {
    width: u32,
    height: u32,
    alpha: Vec<u8>,
}

pub(super) struct Source {
    group: lopdf::Stream,
    id: Option<ObjectId>,
    resources: Object,
    parent: GraphicsState,
    luminosity: bool,
    background: [u8; 3],
    transfer: [u8; 256],
    rasters: Mutex<HashMap<[u64; 5], Arc<Raster>>>,
    work: AtomicU64,
}

fn invalid(message: &str) -> OfdError {
    OfdError::Invalid(format!("PDF soft mask (SMask) {message}"))
}

fn entry<'a>(
    pdf: &'a PdfDocument,
    dict: &'a lopdf::Dictionary,
    key: &[u8],
) -> Result<&'a Object, OfdError> {
    pdf_resolved_value(
        Some(pdf),
        dict.get(key).map_err(|_| {
            invalid(&format!(
                "is missing required {}",
                String::from_utf8_lossy(key)
            ))
        })?,
        "soft mask entry",
    )
}

impl Source {
    pub(super) fn resolve(
        pdf: &PdfDocument,
        resources: &Object,
        value: &Object,
        state: &GraphicsState,
    ) -> Result<Self, OfdError> {
        let dict = value
            .as_dict()
            .map_err(|_| invalid("is not a dictionary"))?;
        if dict.get(b"Type").is_ok() && entry(pdf, dict, b"Type")?.as_name().ok() != Some(b"Mask") {
            return Err(invalid("has invalid Type"));
        }
        let luminosity = match entry(pdf, dict, b"S")?.as_name().ok() {
            Some(b"Alpha") => false,
            Some(b"Luminosity") => true,
            _ => return Err(invalid("has invalid subtype")),
        };
        let (id, group) = pdf
            .dereference(dict.get(b"G").map_err(|_| invalid("has no group"))?)
            .map_err(|_| invalid("has invalid group reference"))?;
        if state.active_soft_masks.len() >= 32
            || id.is_some_and(|id| state.active_soft_masks.contains(&Some(id)))
        {
            return Err(invalid("has cyclic or excessive nesting"));
        }
        let mut group = group
            .as_stream()
            .map_err(|_| invalid("group is not a stream"))?
            .clone();
        if entry(pdf, &group.dict, b"Subtype")?.as_name().ok() != Some(b"Form") {
            return Err(invalid("group is not a Form"));
        }
        let attributes = entry(pdf, &group.dict, b"Group")?
            .as_dict()
            .map_err(|_| invalid("group attributes are invalid"))?;
        if entry(pdf, attributes, b"S")?.as_name().ok() != Some(b"Transparency") {
            return Err(invalid("group is not a transparency group"));
        }
        if attributes.get(b"Type").is_ok()
            && entry(pdf, attributes, b"Type")?.as_name().ok() != Some(b"Group")
        {
            return Err(invalid("has invalid group Type"));
        }
        let knockout = match attributes.get(b"K") {
            Ok(_) => entry(pdf, attributes, b"K")?
                .as_bool()
                .map_err(|_| invalid("has invalid knockout flag"))?,
            Err(_) => false,
        };
        let isolated = match attributes.get(b"I") {
            Ok(_) => entry(pdf, attributes, b"I")?
                .as_bool()
                .map_err(|_| invalid("has invalid isolation flag"))?,
            Err(_) => false,
        };
        let mut background = [0; 3];
        if luminosity {
            if entry(pdf, attributes, b"CS")?.as_name().ok() != Some(b"DeviceRGB") {
                return Err(invalid("luminosity group requires DeviceRGB"));
            }
            if let Ok(value) = dict.get(b"BC") {
                let values = pdf_numeric_array(pdf, value, "soft mask BC", 3)?;
                background =
                    std::array::from_fn(|i| (values[i].clamp(0., 1.) * 255.).round() as u8);
            }
        }
        // Alpha ignores group colors. A transparent initial backdrop computes
        // group alpha for both isolated and non-isolated source groups.
        if !knockout && (!luminosity || isolated) {
            group.dict.set(
                "Group",
                lopdf::dictionary! {
                    "S" => "Transparency", "CS" => "DeviceRGB", "I" => true, "K" => false,
                },
            );
        } else if !knockout {
            // Luminosity's non-isolated group blends directly against BC.
            group.dict.remove(b"Group");
        }
        let mut transfer = std::array::from_fn(|i| i as u8);
        if let Ok(value) = dict.get(b"TR") {
            let value = pdf_resolved_value(Some(pdf), value, "soft mask TR")?;
            if value.as_name().ok() != Some(b"Identity") {
                let functions =
                    scalar_function::ScalarFunctions::parse(pdf, std::slice::from_ref(value))?;
                for (i, result) in transfer.iter_mut().enumerate() {
                    *result = (functions.evaluate(i as f64 / 255.)?.values[0].clamp(0., 1.) * 255.)
                        .round() as u8;
                }
            }
        }
        let mut parent = state.clone();
        parent.soft_mask = None;
        parent.clips.clear();
        parent.fill_alpha = 1.;
        parent.stroke_alpha = 1.;
        parent.blend_mode = "Normal";
        parent.isolated_group = true;
        parent.active_soft_masks.push(id);
        Ok(Self {
            group,
            id,
            resources: resources.clone(),
            parent,
            luminosity,
            background,
            transfer,
            rasters: Mutex::new(HashMap::new()),
            work: AtomicU64::new(0),
        })
    }

    fn dpi(page: &PdfPageContext<'_>) -> Result<f64, OfdError> {
        let width = (page.box_[2] - page.box_[0]) * MM_PER_PT;
        let height = (page.box_[3] - page.box_[1]) * MM_PER_PT;
        let dpi = (406.4 * page.raster_scale)
            .min(1200.)
            .min((MAX_PIXELS / (width * height)).sqrt() * 25.4 * 0.99)
            .min(MAX_PIXELS / width * 25.4 * 0.99)
            .min(MAX_PIXELS / height * 25.4 * 0.99);
        if !dpi.is_finite() || dpi < 1. {
            return Err(OfdError::Limit(
                "PDF soft mask canvas exceeds raster limits".into(),
            ));
        }
        Ok(dpi)
    }

    fn raster(
        &self,
        output: &mut OfdDocument,
        page: &PdfPageContext<'_>,
        dpi: f64,
    ) -> Result<Arc<Raster>, OfdError> {
        let key = [page.box_[0], page.box_[1], page.box_[2], page.box_[3], dpi].map(f64::to_bits);
        if let Some(raster) = self
            .rasters
            .lock()
            .map_err(|_| invalid("cache is unavailable"))?
            .get(&key)
        {
            return Ok(Arc::clone(raster));
        }
        let width = (page.box_[2] - page.box_[0]) * MM_PER_PT;
        let height = (page.box_[3] - page.box_[1]) * MM_PER_PT;
        let scratch = output.pages()?.len();
        output.add_blank_page(width, height)?;
        let path = output.pages()?[scratch].path.clone();
        if self.luminosity {
            output.add_path_object(
                scratch,
                &format!("M 0 0 L {width} 0 L {width} {height} L 0 {height} C"),
                [0., 0., width, height],
                PathStyle {
                    fill: true,
                    stroke: false,
                    fill_color: Some(self.background),
                    ..PathStyle::default()
                },
            )?;
        }
        // Capture fallback resources at gs, before a later Form changes scope.
        let mut stream = self.group.clone();
        if stream.dict.get(b"Resources").is_err() {
            stream.dict.set("Resources", self.resources.clone());
        }
        let resources = Object::Dictionary(lopdf::dictionary! {
            "XObject" => lopdf::dictionary! { "SoftMaskGroup" => Object::Stream(stream) }
        });
        let context = PdfPageContext {
            index: scratch,
            resources: &resources,
            ..*page
        };
        let mut forms = self.id.into_iter().collect();
        // G derives a mask rather than painting the XObject through Do.
        // Optional marked content inside G still uses the normal interpreter.
        import_form(output, &context, &self.parent, b"SoftMaskGroup", &mut forms)?;
        let svg = output.page_svg_for_output(
            scratch,
            crate::RenderOptions::default(),
            false,
            0,
            false,
        )?;
        let rendered = output.render_page_svg(scratch, dpi, &knockout_svg(&svg)?, false)?;
        output.remove_page(scratch)?;
        output.remove_entry(&path)?;
        let image = image::load_from_memory(&rendered)
            .map_err(|_| invalid("cannot decode group raster"))?
            .to_rgba8();
        let alpha = image
            .pixels()
            .map(|p| {
                let value = if self.luminosity {
                    (0.3 * f64::from(p[0]) + 0.59 * f64::from(p[1]) + 0.11 * f64::from(p[2]))
                        .round() as u8
                } else {
                    p[3]
                };
                self.transfer[usize::from(value)]
            })
            .collect();
        let raster = Arc::new(Raster {
            width: image.width(),
            height: image.height(),
            alpha,
        });
        let mut cache = self
            .rasters
            .lock()
            .map_err(|_| invalid("cache is unavailable"))?;
        if cache.len() >= 8 {
            cache.clear();
        }
        cache.insert(key, Arc::clone(&raster));
        Ok(raster)
    }

    pub(super) fn apply(
        &self,
        output: &mut OfdDocument,
        page: &PdfPageContext<'_>,
        first_id: usize,
        blend: &str,
    ) -> Result<(), OfdError> {
        let path = output.pages()?[page.index].path.clone();
        let original = output.required_entry(&path)?.to_vec();
        let parsed = parse_xml(&path, &original)?;
        let mut selected = Vec::new();
        let mut others = Vec::new();
        for object in direct_page_objects(parsed.root_element()) {
            if object
                .attribute("ID")
                .and_then(|s| s.parse::<usize>().ok())
                .is_some_and(|id| id >= first_id)
            {
                selected.push(object.range());
            } else {
                others.push(object.range());
            }
        }
        if selected.is_empty() {
            return Ok(());
        }
        let dpi = Self::dpi(page)?;
        let mask = self.raster(output, page, dpi)?;
        let pixels = u64::from(mask.width) * u64::from(mask.height);
        if self
            .work
            .fetch_add(pixels, Ordering::Relaxed)
            .saturating_add(pixels)
            > MAX_WORK
        {
            return Err(OfdError::Limit(
                "PDF soft mask exceeds pixel work limit".into(),
            ));
        }
        let mut isolated = parsed.input_text().to_owned();
        for range in others.into_iter().rev() {
            isolated.replace_range(range, "");
        }
        output.set_entry(&path, isolated.into_bytes())?;
        let rendered = (|| {
            let svg = output.page_svg_for_output(
                page.index,
                crate::RenderOptions::default(),
                false,
                0,
                false,
            )?;
            output.render_page_svg(page.index, dpi, &knockout_svg(&svg)?, false)
        })();
        output.set_entry(&path, original.clone())?;
        let mut image = image::load_from_memory(&rendered?)
            .map_err(|_| invalid("cannot decode painted raster"))?
            .to_rgba8();
        if image.width() != mask.width || image.height() != mask.height {
            return Err(invalid("raster geometry mismatch"));
        }
        for (pixel, mask) in image.pixels_mut().zip(&mask.alpha) {
            pixel[3] = ((u16::from(pixel[3]) * u16::from(*mask) + 127) / 255) as u8;
        }
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|_| invalid("cannot encode painted raster"))?;
        let mut remaining = parsed.input_text().to_owned();
        for range in selected.into_iter().rev() {
            remaining.replace_range(range, "");
        }
        output.set_entry(&path, remaining.into_bytes())?;
        let width = (page.box_[2] - page.box_[0]) * MM_PER_PT;
        let height = (page.box_[3] - page.box_[1]) * MM_PER_PT;
        let id = output.add_image_object(page.index, &png, "png", [0., 0., width, height])?;
        attach_blend_mode(output, page.index, &id, blend)
    }
}

pub(super) fn image_overrides(
    pdf: &PdfDocument,
    dict: &lopdf::Dictionary,
) -> Result<bool, OfdError> {
    if dict.get(b"Mask").is_ok() || dict.get(b"SMask").is_ok() {
        return Ok(true);
    }
    let jpx = match dict.get(b"Filter") {
        Ok(value) => {
            let value = pdf_resolved_value(Some(pdf), value, "image Filter")?;
            match value {
                Object::Name(name) => name == b"JPXDecode",
                Object::Array(values) => values.iter().any(|v| {
                    pdf_resolved_value(Some(pdf), v, "image Filter")
                        .is_ok_and(|v| v.as_name().ok() == Some(b"JPXDecode"))
                }),
                _ => false,
            }
        }
        Err(_) => false,
    };
    if !jpx {
        return Ok(false);
    }
    match dict.get(b"SMaskInData") {
        Ok(value) => Ok(pdf_resolved_value(Some(pdf), value, "SMaskInData")?
            .as_i64()
            .map_err(|_| invalid("image has invalid SMaskInData"))?
            != 0),
        Err(_) => Ok(false),
    }
}

fn knockout_svg(svg: &str) -> Result<String, OfdError> {
    let parsed = parse_xml("PDF soft mask SVG", svg.as_bytes())?;
    let mut edits = Vec::new();
    for path in parsed
        .descendants()
        .filter(|node| node.is_element() && matches!(node.tag_name().name(), "path" | "text"))
    {
        if path.attribute("fill").is_none_or(|s| s == "none")
            || path.attribute("stroke").is_none_or(|s| s == "none")
            || path
                .ancestors()
                .any(|n| n.is_element() && matches!(n.tag_name().name(), "clipPath" | "mask"))
        {
            continue;
        }
        let id = format!("ofd-pdf-knockout-{}", edits.len());
        let element = |overrides: &[(&str, &str)], geometry: bool| {
            let mut attrs = String::new();
            for attr in path.attributes() {
                if overrides.iter().any(|(name, _)| *name == attr.name())
                    || (geometry && matches!(attr.name(), "transform" | "mask"))
                {
                    continue;
                }
                attrs.push_str(&format!(
                    " {}=\"{}\"",
                    attr.name(),
                    escape_xml(attr.value())
                ));
            }
            for (name, value) in overrides {
                attrs.push_str(&format!(" {name}=\"{}\"", escape_xml(value)));
            }
            let tag = path.tag_name().name();
            let content = path.children().map(|n| &svg[n.range()]).collect::<String>();
            format!("<{tag}{attrs}>{content}</{tag}>")
        };
        let white = element(
            &[
                ("fill", "white"),
                ("stroke", "none"),
                ("fill-opacity", "1"),
                ("opacity", "1"),
            ],
            true,
        );
        let black = element(
            &[
                ("fill", "none"),
                ("stroke", "black"),
                ("stroke-opacity", "1"),
                ("opacity", "1"),
            ],
            true,
        );
        let fill = element(
            &[("stroke", "none"), ("mask", &format!("url(#{id})"))],
            false,
        );
        let stroke = element(&[("fill", "none")], false);
        edits.push((path.range(), format!("<defs><mask id=\"{id}\" maskUnits=\"objectBoundingBox\" x=\"0\" y=\"0\" width=\"1\" height=\"1\">{white}{black}</mask></defs>{fill}{stroke}")));
    }
    let mut result = svg.to_owned();
    for (range, value) in edits.into_iter().rev() {
        result.replace_range(range, &value);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pdf() -> PdfDocument {
        PdfDocument::load_mem(include_bytes!("../../tests/fixtures/soft-mask-alpha.pdf")).unwrap()
    }

    fn read(pdf: &PdfDocument) -> Result<Source, OfdError> {
        Source::resolve(
            pdf,
            pdf.get_dictionary((3, 0))
                .unwrap()
                .get(b"Resources")
                .unwrap(),
            pdf.get_object((7, 0)).unwrap(),
            &GraphicsState::default(),
        )
    }

    fn import(pdf: &mut PdfDocument) -> Result<OfdDocument, OfdError> {
        let mut bytes = Vec::new();
        pdf.save_to(&mut bytes).unwrap();
        OfdDocument::from_pdf_bytes(&bytes)
    }

    #[test]
    fn malformed_required_mask_entries_are_rejected() {
        for (key, value) in [
            ("Type", Object::Name(b"Wrong".to_vec())),
            ("S", Object::Name(b"Wrong".to_vec())),
            ("G", Object::Integer(4)),
            ("TR", Object::Name(b"Wrong".to_vec())),
        ] {
            let mut pdf = pdf();
            pdf.get_object_mut((7, 0))
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set(key, value);
            assert!(read(&pdf).is_err(), "{key}");
        }
    }

    #[test]
    fn malformed_group_flags_are_rejected() {
        for key in ["I", "K", "Type"] {
            let mut pdf = pdf();
            pdf.get_object_mut((5, 0))
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .get_mut(b"Group")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set(key, Object::Name(b"Wrong".to_vec()));
            assert!(read(&pdf).is_err(), "{key}");
        }
    }

    #[test]
    fn invalid_luminosity_backdrop_dimensions_are_rejected() {
        let mut pdf = pdf();
        let mask = pdf.get_object_mut((7, 0)).unwrap().as_dict_mut().unwrap();
        mask.set("S", "Luminosity");
        mask.set("BC", vec![Object::Integer(0), Object::Integer(1)]);
        assert!(read(&pdf).is_err());
    }

    #[test]
    fn transfer_values_clamp_after_evaluation() {
        let mut pdf = pdf();
        pdf.get_object_mut((7, 0))
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("TR", (10, 0));
        let function = pdf.get_object_mut((10, 0)).unwrap().as_dict_mut().unwrap();
        function.set("C0", vec![Object::Integer(-1)]);
        function.set("C1", vec![Object::Integer(2)]);
        let source = read(&pdf).unwrap();
        assert_eq!(source.transfer[0], 0);
        assert_eq!(source.transfer[255], 255);
    }

    #[test]
    fn cyclic_mask_groups_fail_when_painted() {
        let mut pdf = pdf();
        let group = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
        group.content = b"/M gs 0 0 96 64 re f".to_vec();
        group.dict.set("Length", group.content.len() as i64);
        assert!(
            matches!(import(&mut pdf), Err(OfdError::Invalid(message)) if message.contains("cyclic"))
        );
    }

    #[test]
    fn image_mask_precedence_does_not_evaluate_unused_graphics_mask() {
        let mut pdf =
            PdfDocument::load_mem(include_bytes!("../../tests/fixtures/soft-mask-image.pdf"))
                .unwrap();
        let group = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
        group.content = b"unsupportedOperator".to_vec();
        group.dict.set("Length", group.content.len() as i64);
        assert!(import(&mut pdf).is_ok());
    }

    #[test]
    fn embedded_alpha_only_overrides_graphics_mask_for_jpx_images() {
        let pdf = PdfDocument::new();
        let mut image = lopdf::dictionary! { "SMaskInData" => 1 };
        assert!(!image_overrides(&pdf, &image).unwrap());
        image.set("Filter", "JPXDecode");
        assert!(image_overrides(&pdf, &image).unwrap());
        image.set("SMaskInData", 0);
        assert!(!image_overrides(&pdf, &image).unwrap());
        image.set("Mask", vec![Object::Integer(0), Object::Integer(1)]);
        assert!(image_overrides(&pdf, &image).unwrap());
    }

    #[test]
    fn raster_density_respects_pixel_and_dpi_limits() {
        let pdf = PdfDocument::new();
        let resources = Object::Dictionary(lopdf::Dictionary::new());
        for (box_, scale) in [
            ([0., 0., 96., 64.], 1.),
            ([0., 0., 10000., 5000.], 10.),
            ([0., 0., 96., 64.], f64::INFINITY),
            ([0., 0., 0.0001, 1_000_000.], 1.),
        ] {
            let page = PdfPageContext {
                pdf: &pdf,
                resources: &resources,
                box_,
                index: 0,
                raster_scale: scale,
            };
            let dpi = Source::dpi(&page).unwrap();
            let pixels =
                ((box_[2] - box_[0]) / 72. * dpi).ceil() * ((box_[3] - box_[1]) / 72. * dpi).ceil();
            assert!((1. ..=1200.).contains(&dpi) && pixels <= MAX_PIXELS);
        }
    }

    #[test]
    fn pixel_work_limit_preserves_the_host_paint() {
        let pdf = pdf();
        let source = read(&pdf).unwrap();
        source.work.store(MAX_WORK, Ordering::Relaxed);
        let resources = Object::Dictionary(lopdf::Dictionary::new());
        let page = PdfPageContext {
            pdf: &pdf,
            resources: &resources,
            box_: [0., 0., 96., 64.],
            index: 0,
            raster_scale: 1.,
        };
        let mut output = OfdDocument::new();
        output
            .add_blank_page(96. * MM_PER_PT, 64. * MM_PER_PT)
            .unwrap();
        let first = output.next_id;
        output
            .add_path_object(
                0,
                "M 0 0 L 10 0 L 10 10 C",
                [0., 0., 20., 20.],
                PathStyle {
                    fill: true,
                    stroke: false,
                    ..PathStyle::default()
                },
            )
            .unwrap();
        let before = output.page_png(0, 72.).unwrap();
        assert!(matches!(
            source.apply(&mut output, &page, first, "Normal"),
            Err(OfdError::Limit(_))
        ));
        assert_eq!(output.page_png(0, 72.).unwrap(), before);
        assert_eq!(output.pages().unwrap().len(), 1);
    }
    #[test]
    fn cycles_crossing_patterns_and_masks_are_rejected() {
        let mut pdf = pdf();
        for (id, content) in [
            (
                (4, 0),
                b"/Pattern cs /P scn /M gs 0 0 96 64 re f".as_slice(),
            ),
            ((5, 0), b"/Pattern cs /P scn 0 0 96 64 re f".as_slice()),
            ((21, 0), b"/M gs 0 0 24 16 re f".as_slice()),
        ] {
            let stream = pdf.get_object_mut(id).unwrap().as_stream_mut().unwrap();
            stream.content = content.to_vec();
            stream.dict.set("Length", stream.content.len() as i64);
        }
        let Err(error) = import(&mut pdf) else {
            panic!("cycle should fail")
        };
        assert!(
            matches!(&error, OfdError::Invalid(message) if message.contains("cycl")),
            "{error}"
        );
    }

    #[test]
    fn transparent_parent_mask_is_not_accepted_as_opaque_group_state() {
        let mut pdf = PdfDocument::load_mem(include_bytes!(
            "../../tests/fixtures/soft-mask-ordinary-form.pdf"
        ))
        .unwrap();
        pdf.get_object_mut((11, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set(
                "Group",
                lopdf::dictionary! {
                    "S" => "Transparency", "CS" => "DeviceRGB", "I" => false,
                },
            );
        assert!(
            matches!(import(&mut pdf), Err(OfdError::Invalid(message)) if message.contains("requires opaque content"))
        );
    }
    #[test]
    fn invisible_and_clip_only_text_do_not_evaluate_the_mask() {
        for mode in [3, 7] {
            let mut pdf = pdf();
            let group = pdf.get_object_mut((5, 0)).unwrap().as_stream_mut().unwrap();
            group.content = b"unsupportedOperator".to_vec();
            group.dict.set("Length", group.content.len() as i64);
            let page = pdf.get_object_mut((4, 0)).unwrap().as_stream_mut().unwrap();
            page.content = format!("/M gs BT /F1 40 Tf 12 16 Td {mode} Tr (A) Tj ET").into_bytes();
            page.dict.set("Length", page.content.len() as i64);
            assert!(import(&mut pdf).is_ok(), "mode {mode}");
        }
    }
    #[test]
    fn shading_pattern_ext_mask_matches_direct_shading_mask() {
        let mut pdf = pdf();
        *pdf.get_object_mut((21, 0)).unwrap() = Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pattern", "PatternType" => 2, "Shading" => (13, 0), "ExtGState" => (6, 0),
        });
        let page = pdf.get_object_mut((4, 0)).unwrap().as_stream_mut().unwrap();
        page.content = b"1 1 0 rg 0 0 96 64 re f /Pattern cs /P scn 0 0 96 64 re f".to_vec();
        page.dict.set("Length", page.content.len() as i64);
        let document = import(&mut pdf).unwrap();
        let actual = image::load_from_memory(&document.page_png(0, 144.).unwrap())
            .unwrap()
            .to_rgb8();
        let expected = image::load_from_memory(include_bytes!(
            "../../tests/fixtures/soft-mask-shaded-paint.png"
        ))
        .unwrap()
        .to_rgb8();
        assert_eq!(actual.dimensions(), expected.dimensions());
        let error: u64 = actual
            .as_raw()
            .iter()
            .zip(expected.as_raw())
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        assert!(
            (error as f64 / actual.as_raw().len() as f64) < 2.,
            "mean error {}",
            error as f64 / actual.as_raw().len() as f64
        );
        assert!(document.validate_references().unwrap().is_empty());
    }
}
