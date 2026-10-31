//! Write imported action sequences back as PDF Next actions.
use super::*;

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
