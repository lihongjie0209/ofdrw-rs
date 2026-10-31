//! Preserve imported and native CLICK sequences as PDF Next actions.
use super::*;

pub(super) fn object_links(
    actions: &[PageObjectActionInfo],
    document_index: usize,
    pages: &[PageInfo],
    bookmarks: &[BookmarkInfo],
    attachments: &HashMap<(usize, String), Ref>,
    geometry: ([f64; 4], [f64; 4]),
) -> Result<Vec<(PdfLinkOutputRegion, PdfLinkTarget)>, OfdError> {
    let (boundary, physical_box) = geometry;
    let supported = actions
        .iter()
        .filter_map(|action| {
            pdf_link_target(action, document_index, pages, bookmarks, attachments)
                .map(|target| (action, target))
        })
        .take(257)
        .collect::<Vec<_>>();
    let Some((first, _)) = supported.first() else {
        return Ok(Vec::new());
    };
    if supported.len() > 256 {
        return Err(OfdError::Limit(
            "PDF object sequence exceeds 256 actions".into(),
        ));
    }
    if supported
        .iter()
        .all(|(action, _)| action.region == first.region)
    {
        let target = combine_targets(supported.iter().map(|(_, target)| target.clone()).collect())?;
        return pdf_link_output_regions(
            first,
            boundary,
            physical_box,
            !matches!(target, PdfLinkTarget::Attachment(_)),
        )?
        .into_iter()
        .map(|region| Ok((region, target.clone())))
        .collect();
    }
    // Different rectangles can overlap. Partition their union so each PDF
    // click region executes exactly the native actions active in that cell.
    let mut rectangles = Vec::new();
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut rectangle_count = 0usize;
    for (action, _) in &supported {
        let shapes = pdf_action_shapes(action, boundary)?;
        rectangle_count = rectangle_count.saturating_add(shapes.len());
        if rectangle_count > 1_000 {
            return Err(OfdError::Limit(
                "PDF object action partition exceeds 1000 rectangles".into(),
            ));
        }
        let mut bounds = Vec::new();
        for shape in shapes {
            if shape.quad.is_some() {
                return Err(OfdError::Invalid(
                    "different PDF object trigger regions must be rectangular".into(),
                ));
            }
            let [x, y, width, height] = shape.boundary;
            let bounds_ = [x, y, x + width, y + height];
            xs.extend([bounds_[0], bounds_[2]]);
            ys.extend([bounds_[1], bounds_[3]]);
            bounds.push(bounds_);
        }
        rectangles.push(bounds);
    }
    xs.sort_by(f64::total_cmp);
    xs.dedup();
    ys.sort_by(f64::total_cmp);
    ys.dedup();
    if xs
        .len()
        .saturating_sub(1)
        .saturating_mul(ys.len().saturating_sub(1))
        > 10_000
    {
        return Err(OfdError::Limit(
            "PDF object action partition exceeds 10000 cells".into(),
        ));
    }
    let mut links = Vec::new();
    let mut output_actions = 0usize;
    let mut output_bytes = 0usize;
    for x in xs.windows(2) {
        for y in ys.windows(2) {
            let targets = supported
                .iter()
                .zip(&rectangles)
                .filter(|(_, rectangles)| {
                    rectangles.iter().any(|bounds| {
                        x[0] >= bounds[0]
                            && x[1] <= bounds[2]
                            && y[0] >= bounds[1]
                            && y[1] <= bounds[3]
                    })
                })
                .map(|((_, target), _)| target)
                .collect::<Vec<_>>();
            if targets.is_empty() {
                continue;
            }
            output_actions = output_actions.saturating_add(targets.len());
            output_bytes = targets.iter().fold(output_bytes, |bytes, target| {
                bytes.saturating_add(target_bytes(target))
            });
            if output_actions > 10_000 {
                return Err(OfdError::Limit(
                    "PDF object action partition exceeds 10000 output actions".into(),
                ));
            }
            if output_bytes > 1_000_000 {
                return Err(OfdError::Limit(
                    "PDF object action partition exceeds 1000000 target bytes".into(),
                ));
            }
            let target = combine_targets(targets.into_iter().cloned().collect())?;
            links.push((
                PdfLinkOutputRegion {
                    rect: pdf_link_rect([x[0], y[0], x[1] - x[0], y[1] - y[0]], physical_box)?,
                    quads: None,
                },
                target,
            ));
        }
    }
    Ok(links)
}

fn combine_targets(mut targets: Vec<PdfLinkTarget>) -> Result<PdfLinkTarget, OfdError> {
    if targets.iter().fold(0usize, |bytes, target| {
        bytes.saturating_add(target_bytes(target))
    }) > 1_000_000
    {
        return Err(OfdError::Limit(
            "PDF object sequence exceeds 1000000 target bytes".into(),
        ));
    }
    if targets.len() == 1 {
        return Ok(targets.swap_remove(0));
    }
    if targets
        .iter()
        .any(|target| matches!(target, PdfLinkTarget::Attachment(_)))
    {
        return Err(OfdError::Invalid(
            "attachment activation cannot be combined with other PDF object actions".into(),
        ));
    }
    Ok(PdfLinkTarget::Sequence(targets))
}

fn target_bytes(target: &PdfLinkTarget) -> usize {
    match target {
        PdfLinkTarget::Uri(uri) => uri.len(),
        PdfLinkTarget::Goto(destination) => destination
            .page_id
            .len()
            .saturating_add(destination.kind.len()),
        _ => 0,
    }
}

pub(super) fn sequence(
    annotation: &AnnotationInfo,
    fallback: PdfLinkTarget,
    document_index: usize,
    pages: &[PageInfo],
    bookmarks: &[BookmarkInfo],
    attachments: &HashMap<(usize, String), Ref>,
) -> Result<PdfLinkTarget, OfdError> {
    if annotation
        .parameters
        .get("PDFActionSequence")
        .map(String::as_str)
        != Some("true")
    {
        return Ok(fallback);
    }
    if annotation.actions.len() > 256 {
        return Err(OfdError::Limit(
            "PDF Link sequence exceeds 256 actions".into(),
        ));
    }
    let mut targets = Vec::new();
    for action in &annotation.actions {
        if action.region != annotation.actions[0].region {
            return Err(OfdError::Invalid(
                "PDF Link sequence actions have different trigger regions".into(),
            ));
        }
        let target = pdf_link_target(action, document_index, pages, bookmarks, attachments)
            .filter(|target| !matches!(target, PdfLinkTarget::Attachment(_)))
            .ok_or_else(|| {
                OfdError::Invalid("PDF Link sequence has an unsupported action".into())
            })?;
        targets.push(target);
    }
    Ok(PdfLinkTarget::Sequence(targets))
}

pub(super) fn write(
    mut action: pdf_writer::writers::Action<'_>,
    target: &PdfLinkTarget,
    document_index: usize,
    pages: &[PageInfo],
    page_refs: &[Ref],
) -> Result<(), OfdError> {
    let (first, rest) = match target {
        PdfLinkTarget::Sequence(targets) => targets
            .split_first()
            .ok_or_else(|| OfdError::Invalid("PDF Link sequence is empty".into()))?,
        target => (target, &[][..]),
    };
    match first {
        PdfLinkTarget::Uri(uri) => {
            action.action_type(ActionType::Uri).uri(Str(uri.as_bytes()));
        }
        PdfLinkTarget::Goto(destination) => {
            let index = resolve_page_id(pages, document_index, &destination.page_id)?;
            action.action_type(ActionType::GoTo);
            write_pdf_destination(
                action.destination(),
                destination,
                &pages[index],
                page_refs[index],
            )?;
        }
        _ => return Err(OfdError::Invalid("invalid PDF Link sequence action".into())),
    }
    // Flattened Next arrays retain execution order without shared references.
    if !rest.is_empty() {
        let mut next = action.insert(Name(b"Next")).array();
        for target in rest {
            if matches!(
                target,
                PdfLinkTarget::Sequence(_) | PdfLinkTarget::Attachment(_)
            ) {
                return Err(OfdError::Invalid(
                    "invalid nested PDF Link sequence action".into(),
                ));
            }
            write(
                next.push().start(),
                target,
                document_index,
                pages,
                page_refs,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edited_sequence_rejects_incompatible_regions_and_unsupported_actions() {
        let document =
            OfdDocument::from_pdf_bytes(include_bytes!("../tests/fixtures/link-action-tree.pdf"))
                .unwrap();
        let pages = document.pages().unwrap();
        for mode in 0..3 {
            let mut annotation = document.annotations().unwrap().remove(0);
            match mode {
                0 => annotation.actions[1].region = None,
                1 => annotation.actions[1].kind = "Sound".into(),
                _ => annotation
                    .actions
                    .resize(257, annotation.actions[0].clone()),
            }
            assert!(sequence(
                &annotation,
                PdfLinkTarget::Uri("https://example.org/".into()),
                0,
                &pages,
                &[],
                &HashMap::new()
            )
            .is_err());
        }
    }
}
