//! Composite RGB fallback for PDF DeviceN colorants.

use super::{
    multi_function::Function, parse_color_space_inner, pdf_resolved_value, separation_color, Arc,
    Object, OfdError, PdfColorSpace, PdfDocument, RenderingIntent,
};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) struct Tint {
    alternate: PdfColorSpace,
    function: Function,
    remaining: AtomicUsize,
}

impl Tint {
    pub(super) fn color(
        &self,
        values: &[f64],
        intent: RenderingIntent,
    ) -> Result<[u8; 3], OfdError> {
        self.remaining
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(self.function.work_cost())
            })
            .map_err(|_| {
                OfdError::Limit("PDF DeviceN tint exceeds 100 million work units".into())
            })?;
        self.alternate
            .color_with_intent(&self.function.evaluate(values)?, intent)
    }
}

fn invalid(reason: &str) -> OfdError {
    OfdError::Invalid(format!("invalid PDF DeviceN: {reason}"))
}

pub(super) fn parse(
    pdf: Option<&PdfDocument>,
    resources: Option<&Object>,
    values: &[Object],
    depth: usize,
    remap_device: bool,
) -> Result<PdfColorSpace, OfdError> {
    if !(4..=5).contains(&values.len()) {
        return Err(invalid("expected four or five entries"));
    }
    let names = pdf_resolved_value(pdf, &values[1], "DeviceN names")?
        .as_array()
        .map_err(|_| invalid("names must be an array"))?;
    if names.is_empty() || names.len() > 32 {
        return Err(invalid("requires one through 32 components"));
    }
    let mut unique = BTreeSet::new();
    let mut all_none = true;
    let mut has_none = false;
    for name in names {
        let name = pdf_resolved_value(pdf, name, "DeviceN colorant")?
            .as_name()
            .map_err(|_| invalid("colorant must be a name"))?;
        if name.len() > 1024 {
            return Err(OfdError::Limit(
                "PDF DeviceN colorant name exceeds 1024 bytes".into(),
            ));
        }
        if name == b"All" {
            return Err(invalid("All is forbidden"));
        }
        has_none |= name == b"None";
        all_none &= name == b"None";
        if name != b"None" && !unique.insert(name) {
            return Err(invalid("duplicate colorant"));
        }
    }
    if let Some(attributes) = values.get(4) {
        let attributes = pdf_resolved_value(pdf, attributes, "DeviceN attributes")?
            .as_dict()
            .map_err(|_| invalid("attributes must be a dictionary"))?;
        if let Ok(subtype) = attributes.get(b"Subtype") {
            let subtype = pdf_resolved_value(pdf, subtype, "DeviceN Subtype")?
                .as_name()
                .map_err(|_| invalid("Subtype must be a name"))?;
            if !matches!(subtype, b"DeviceN" | b"NChannel") {
                return Err(invalid("Subtype"));
            }
            if subtype == b"NChannel" && has_none {
                return Err(invalid("NChannel cannot include None"));
            }
        }
    }
    if all_none {
        return Ok(PdfColorSpace::DeviceN {
            channels: names.len(),
            tint: None,
        });
    }
    // With one input the existing bounded scalar importer also supports the
    // exponential and stitching function types, which require exactly one input.
    if names.len() == 1 {
        let name = pdf_resolved_value(pdf, &names[0], "DeviceN colorant")?.clone();
        return separation_color::parse(
            pdf,
            resources,
            &[
                Object::Name(b"Separation".to_vec()),
                name,
                values[2].clone(),
                values[3].clone(),
            ],
            depth,
            remap_device,
        );
    }
    let alternate = parse_color_space_inner(pdf, resources, &values[2], depth + 1, remap_device)?;
    if matches!(
        alternate,
        PdfColorSpace::Pattern { .. }
            | PdfColorSpace::Indexed { .. }
            | PdfColorSpace::Separation { .. }
            | PdfColorSpace::SeparationAll
            | PdfColorSpace::SeparationNone
            | PdfColorSpace::DeviceN { .. }
    ) {
        return Err(invalid("alternate must be a device or CIE color space"));
    }
    let empty;
    let pdf = match pdf {
        Some(pdf) => pdf,
        None => {
            empty = PdfDocument::new();
            &empty
        }
    };
    let function = Function::parse(pdf, &values[3], names.len(), alternate.channels())?;
    Ok(PdfColorSpace::DeviceN {
        channels: names.len(),
        tint: Some(Arc::new(Tint {
            alternate,
            function,
            remaining: AtomicUsize::new(100_000_000),
        })),
    })
}

#[cfg(test)]
mod tests {
    use super::super::parse_color_space;
    use super::*;
    use lopdf::{dictionary, Stream};

    fn space(names: &[&[u8]]) -> Object {
        Object::Array(vec![
            Object::Name(b"DeviceN".to_vec()),
            Object::Array(
                names
                    .iter()
                    .map(|name| Object::Name(name.to_vec()))
                    .collect(),
            ),
            Object::Name(b"DeviceRGB".to_vec()),
            Object::Stream(Stream::new(
                dictionary! {"FunctionType"=>4,"Domain"=>names.iter().flat_map(|_| [0.into(),1.into()]).collect::<Vec<Object>>(),"Range"=>vec![0.into(),1.into(),0.into(),1.into(),0.into(),1.into()]},
                b"{ 1 exch sub exch 1 exch sub exch 0 }".to_vec(),
            )),
        ])
    }

    #[test]
    fn multicolor_tints_use_alternate_rgb_and_subtractive_initial_components() {
        let color = parse_color_space(None, None, &space(&[b"Orange", b"Green"]), 0).unwrap();
        assert_eq!(color.channels(), 2);
        assert_eq!(color.initial_components(), [1., 1.]);
        assert_eq!(color.color(&[0.25, 0.75]).unwrap(), [191, 64, 0]);
        assert!(color.color(&[0.25]).is_err());
        assert!(color.color(&[0., f64::INFINITY]).is_err());
        assert!(color.color(&[-1., 0.]).is_err());
        assert!(!color.nonpainting());
    }

    #[test]
    fn all_none_suppresses_paint_but_mixed_none_passes_its_tint_to_the_function() {
        let none = parse_color_space(None, None, &space(&[b"None", b"None"]), 0).unwrap();
        assert!(none.nonpainting());
        assert_eq!(none.channels(), 2);
        let mixed = parse_color_space(None, None, &space(&[b"None", b"Green"]), 0).unwrap();
        assert!(!mixed.nonpainting());
        assert_eq!(mixed.color(&[0.25, 0.75]).unwrap(), [191, 64, 0]);
    }

    #[test]
    fn duplicate_all_and_invalid_nchannel_names_return_errors() {
        for names in [
            vec![b"All".as_slice()],
            vec![b"Red".as_slice(), b"Red"],
            vec![],
            vec![b"x".as_slice(); 33],
        ] {
            assert!(parse_color_space(None, None, &space(&names), 0).is_err());
        }
        let mut object = space(&[b"None", b"Green"]);
        object
            .as_array_mut()
            .unwrap()
            .push(Object::Dictionary(dictionary! {"Subtype"=>"NChannel"}));
        assert!(parse_color_space(None, None, &object, 0).is_err());
    }

    #[test]
    fn single_component_spaces_reuse_exponential_tint_conversion() {
        let mut object = space(&[b"Orange"]);
        object.as_array_mut().unwrap()[3] = Object::Dictionary(dictionary! {
            "FunctionType"=>2, "Domain"=>vec![0.into(),1.into()],
            "C0"=>vec![1.into(),1.into(),1.into()], "C1"=>vec![1.into(),0.into(),0.into()], "N"=>1
        });
        let color = parse_color_space(None, None, &object, 0).unwrap();
        assert_eq!(color.channels(), 1);
        assert_eq!(color.initial_components(), [1.]);
        assert_eq!(color.color(&[0.25]).unwrap(), [255, 191, 191]);
    }

    #[test]
    fn special_spaces_cannot_replace_default_device_colors() {
        let pdf = PdfDocument::new();
        let resources = Object::Dictionary(dictionary! {"ColorSpace"=>dictionary! {
            "DefaultRGB"=>space(&[b"Orange",b"Green",b"Blue"])
        }});
        assert!(parse_color_space(
            Some(&pdf),
            Some(&resources),
            &Object::Name(b"DeviceRGB".to_vec()),
            0
        )
        .is_err());
    }

    #[test]
    fn cumulative_tint_work_is_bounded() {
        let color = parse_color_space(None, None, &space(&[b"Orange", b"Green"]), 0).unwrap();
        let PdfColorSpace::DeviceN {
            tint: Some(tint), ..
        } = &color
        else {
            panic!("missing DeviceN transform")
        };
        tint.remaining.store(0, Ordering::Relaxed);
        assert!(matches!(color.color(&[0.5, 0.5]), Err(OfdError::Limit(_))));
    }
}
