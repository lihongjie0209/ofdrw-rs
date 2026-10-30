//! Read-only checks for the first OFD-A archival constraints.

use super::*;

impl OfdDocument {
    /// Prepare an unsigned single-body OFD for archival use and return a new package.
    ///
    /// This applies the metadata and presentation normalizations currently
    /// supported by this crate: `DocType`, permission, viewer preference, and extension declarations,
    /// image interpolation, GIF images, audio/video declarations, custom image data, misplaced public and
    /// page resources, safe horizontal text scaling, plain deep PageBlock wrappers and zero-area clips in pages
    /// and referenced templates, non-Goto actions, binary attachment summaries,
    /// plain annotation appearance blocks, annotation flags, duplicate page layer names,
    /// redundant paint attributes, stale `MaxUnitID` values, and safe removal of
    /// superseded payload entries.
    /// Call [`Self::archive_violations`]
    /// on the result to inspect rules that still need conversion.
    pub fn prepare_archive(&self) -> Result<Self, OfdError> {
        if self.is_encrypted() {
            return Err(OfdError::Invalid(
                "decrypt the package before archival preparation".into(),
            ));
        }
        if self.entries.contains_key("OFDEntries.xml") {
            return Err(OfdError::Invalid(
                "integrity-protected package cannot be prepared for archival".into(),
            ));
        }
        let root = parse_xml("OFD.xml", self.required_entry("OFD.xml")?)?;
        let ofd = root.root_element();
        if self.doc_roots()?.len() != 1 {
            return Err(OfdError::Invalid(
                "archival preparation requires one document body".into(),
            ));
        }
        if ofd
            .children()
            .filter(|node| is_ofd_element(*node, "DocBody"))
            .any(|body| {
                body.children()
                    .any(|node| is_ofd_element(node, "Signatures"))
            })
        {
            return Err(OfdError::Invalid(
                "signed package requires signature archival treatment".into(),
            ));
        }

        let mut prepared = self.clone();
        let mut obsolete_entries = HashSet::new();
        let root_source =
            std::str::from_utf8(self.required_entry("OFD.xml")?).map_err(|error| {
                OfdError::Xml {
                    path: "OFD.xml".into(),
                    message: error.to_string(),
                }
            })?;
        let mut root_edits = Vec::new();
        if let Some(attribute) = ofd.attributes().find(|attr| attr.name() == "DocType") {
            root_edits.push((attribute.range(), "DocType=\"OFD-A\"".to_owned()));
        } else {
            let tag = qualified_tag(root_source, ofd)?;
            let offset = ofd.range().start + 1 + tag.len();
            root_edits.push((offset..offset, " DocType=\"OFD-A\"".to_owned()));
        }
        prepared.set_entry(
            "OFD.xml",
            apply_archive_edits(root_source, root_edits).into_bytes(),
        )?;

        for path in self.doc_roots()? {
            let bytes = self.required_entry(&path)?;
            let source = std::str::from_utf8(bytes).map_err(|error| OfdError::Xml {
                path: path.clone(),
                message: error.to_string(),
            })?;
            let document = parse_xml(&path, bytes)?;
            let edits = document
                .root_element()
                .children()
                .filter(|node| {
                    is_ofd_element(*node, "Permissions")
                        || is_ofd_element(*node, "VPreferences")
                        || is_ofd_element(*node, "Extensions")
                })
                .map(|node| (node.range(), String::new()))
                .collect::<Vec<_>>();
            if !edits.is_empty() {
                prepared.set_entry(&path, apply_archive_edits(source, edits).into_bytes())?;
            }
        }

        for page in self.pages()? {
            normalize_archive_page_content(&mut prepared, &page.path)?;
            normalize_archive_layer_names(&mut prepared, &page.path)?;
        }
        let mut template_paths = HashSet::new();
        for page in self.pages()? {
            for source in self.page_sources(page.index)? {
                if source.template_id.is_some() {
                    template_paths.insert(source.path);
                }
            }
        }
        for path in template_paths {
            normalize_archive_page_content(&mut prepared, &path)?;
        }

        let mut resource_paths = HashSet::new();
        resource_paths.extend(self.document_multimedia_paths(0)?);
        for page in self.pages()? {
            for source in self.page_sources(page.index)? {
                resource_paths.extend(self.page_resource_paths(&source.path)?);
            }
        }
        let mut resource_paths = resource_paths.into_iter().collect::<Vec<_>>();
        resource_paths.sort();
        for path in resource_paths {
            if self.entry(&path)?.is_none() {
                continue;
            }
            let bytes = self.required_entry(&path)?;
            let source = std::str::from_utf8(bytes).map_err(|error| OfdError::Xml {
                path: path.clone(),
                message: error.to_string(),
            })?;
            let resource = parse_xml(&path, bytes)?;
            let base = parent_path(&path);
            let base = match resource.root_element().attribute("BaseLoc") {
                Some(loc) => resolve_resource_base(&base, loc)?,
                None => base,
            };
            let mut edits = Vec::new();
            for media in resource
                .descendants()
                .filter(|node| is_ofd_descendant(*node, resource.root_element(), "MultiMedia"))
            {
                let kind = media.attribute("Type").unwrap_or("");
                if kind.eq_ignore_ascii_case("Audio") || kind.eq_ignore_ascii_case("Video") {
                    for file in media
                        .children()
                        .filter(|node| is_ofd_element(*node, "MediaFile"))
                    {
                        let loc = element_text(file);
                        let loc = loc.trim();
                        if loc.is_empty() || is_external_url(loc) {
                            continue;
                        }
                        let path = resolve_path(&base, loc)?;
                        if let Some(actual) = self.matching_entry_path(&path)? {
                            obsolete_entries.insert(actual);
                        }
                    }
                    edits.push((media.range(), String::new()));
                    continue;
                }
                if kind != "Image" {
                    continue;
                }
                edits.extend(
                    media
                        .children()
                        .filter(|node| node.is_element() && !has_ofd_namespace(*node))
                        .map(|node| (node.range(), String::new())),
                );
                let Some(file) = media
                    .children()
                    .find(|node| is_ofd_element(*node, "MediaFile"))
                else {
                    continue;
                };
                let loc = element_text(file);
                let loc = loc.trim();
                if loc.is_empty() || is_external_url(loc) {
                    continue;
                }
                let image_path = resolve_path(&base, loc)?;
                let Some(actual_path) = self.matching_entry_path(&image_path)? else {
                    continue;
                };
                let image = self.required_raw_entry(&actual_path)?;
                if archive_image_format(image) != Some("GIF") {
                    continue;
                }
                let png = raster_to_png(image)?;
                let mut new_loc = format!("{loc}.archival.png");
                let mut counter = 1usize;
                while prepared
                    .matching_entry_path(&resolve_path(&base, &new_loc)?)?
                    .is_some()
                {
                    new_loc = format!("{loc}.archival-{counter}.png");
                    counter += 1;
                }
                prepared.set_entry(&resolve_path(&base, &new_loc)?, png)?;
                obsolete_entries.insert(actual_path);
                edits.push((element_content_range(file, source)?, escape_xml(&new_loc)));
                if let Some(format) = media.attributes().find(|attr| attr.name() == "Format") {
                    edits.push((format.range(), "Format=\"PNG\"".to_owned()));
                } else {
                    let tag = qualified_tag(source, media)?;
                    let offset = media.range().start + 1 + tag.len();
                    edits.push((offset..offset, " Format=\"PNG\"".to_owned()));
                }
            }
            if !edits.is_empty() {
                prepared.set_entry(&path, apply_archive_edits(source, edits).into_bytes())?;
            }
        }

        for path in self.entries.keys().filter(|path| is_xml_package_path(path)) {
            if prepared.entry(path)?.is_none() {
                continue;
            }
            let bytes = prepared.required_entry(path)?;
            let Ok(xml) = parse_xml(path, bytes) else {
                continue;
            };
            let Some(namespace) = archive_action_namespace(xml.root_element()) else {
                continue;
            };
            let source = std::str::from_utf8(bytes).map_err(|error| OfdError::Xml {
                path: path.clone(),
                message: error.to_string(),
            })?;
            let mut edits = Vec::new();
            for actions in xml
                .descendants()
                .filter(|node| is_archive_action_element(*node, "Actions", namespace))
            {
                let children = actions
                    .children()
                    .filter(|node| is_archive_action_element(*node, "Action", namespace))
                    .collect::<Vec<_>>();
                if children.is_empty() {
                    continue;
                }
                if children
                    .iter()
                    .all(|action| !archive_goto_action(*action, namespace))
                {
                    edits.push((actions.range(), String::new()));
                } else {
                    edits.extend(
                        children
                            .into_iter()
                            .filter(|action| !archive_goto_action(*action, namespace))
                            .map(|action| (action.range(), String::new())),
                    );
                }
            }
            for action in xml
                .descendants()
                .filter(|node| is_archive_action_element(*node, "Action", namespace))
            {
                if !action
                    .parent()
                    .is_some_and(|node| is_archive_action_element(node, "Actions", namespace))
                    && !archive_goto_action(action, namespace)
                {
                    edits.push((action.range(), String::new()));
                }
            }
            if !edits.is_empty() {
                prepared.set_entry(path, apply_archive_edits(source, edits).into_bytes())?;
            }
        }

        relocate_public_resources(&mut prepared)?;
        relocate_page_resources(&mut prepared)?;

        for (index, attachment) in self.attachments()?.into_iter().enumerate() {
            if attachment
                .format
                .as_deref()
                .is_some_and(|format| matches!(format.to_ascii_uppercase().as_str(), "TXT" | "XML"))
            {
                continue;
            }
            let data = self.required_raw_entry(&attachment.path)?;
            let digest = Sha256::digest(data)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let summary = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ArchivedAttachment xmlns=\"urn:ofdrw:archive:summary:1\"><OriginalName>{}</OriginalName><OriginalFormat>{}</OriginalFormat><OriginalSize>{}</OriginalSize><SHA256>{digest}</SHA256></ArchivedAttachment>",
                escape_xml(&attachment.name),
                escape_xml(attachment.format.as_deref().unwrap_or("unknown")),
                data.len(),
            );
            prepared.replace_attachment_data(index, summary.as_bytes())?;
            prepared.set_attachment_metadata(
                index,
                AttachmentMetadata {
                    format: Some("XML".into()),
                    creation_date: attachment.creation_date,
                    mod_date: attachment.mod_date,
                    visible: attachment.visible,
                    usage: Some(attachment.usage),
                },
            )?;
            prepared.rename_attachment(index, &format!("{}.summary.xml", attachment.name))?;
            obsolete_entries.insert(attachment.path);
        }

        let pages = self.pages()?;
        let mut annotation_paths = Vec::new();
        for annotation in self.annotations()? {
            let page = pages
                .iter()
                .find(|page| {
                    page.document_index == annotation.document_index
                        && page.id == annotation.page_id
                })
                .ok_or_else(|| {
                    OfdError::Invalid(format!("annotation page {} is missing", annotation.page_id))
                })?;
            prepared.set_annotation_read_only(page.index, &annotation.id, true)?;
            prepared.set_annotation_no_zoom(page.index, &annotation.id, true)?;
            prepared.set_annotation_no_rotate(page.index, &annotation.id, true)?;
            annotation_paths.push(annotation.source_path);
        }
        annotation_paths.sort();
        annotation_paths.dedup();
        for path in annotation_paths {
            normalize_archive_image_interpolation(&mut prepared, &path)?;
            normalize_archive_redundant_paint(&mut prepared, &path)?;
            flatten_archive_page_blocks(&mut prepared, &path, 0, true)?;
        }
        prune_archive_obsolete_entries(&mut prepared, obsolete_entries);
        raise_archive_max_unit_id(&mut prepared)?;
        OfdDocument::from_bytes(&prepared.to_bytes()?)
    }

    /// Check the supported OFD-A rules without modifying the package.
    ///
    /// The current rules cover document type, body count, permissions,
    /// extensions, audio/video resources, PageBlock depth, image interpolation,
    /// text size and horizontal scale, non-Goto actions, external references,
    /// attachment formats, color spaces and profiles, image formats,
    /// signatures, and encryption.
    /// An empty result does not yet establish full GB/T 42133 conformance.
    pub fn archive_violations(&self) -> Result<Vec<ArchiveViolation>, OfdError> {
        let mut issues = Vec::new();
        if self.is_encrypted() {
            issues.push(violation(
                "ENCRYPTION",
                ArchiveSeverity::Error,
                "encrypted OFD packages require decryption before archival",
                None,
                Some("encrypted"),
                Some("unencrypted"),
            ));
            return Ok(issues);
        }

        let root = parse_xml("OFD.xml", self.required_entry("OFD.xml")?)?;
        let ofd = root.root_element();
        let doc_type = ofd.attribute("DocType");
        if doc_type != Some("OFD-A") {
            issues.push(violation(
                "DOC_TYPE",
                ArchiveSeverity::Error,
                "OFD.xml DocType must be OFD-A",
                Some("OFD.xml"),
                doc_type,
                Some("OFD-A"),
            ));
        }
        let roots = self.doc_roots()?;
        if roots.len() != 1 {
            issues.push(violation(
                "SINGLE_DOC",
                ArchiveSeverity::Error,
                "OFD-A permits one document body",
                Some("OFD.xml"),
                Some(&roots.len().to_string()),
                Some("1"),
            ));
        }

        for path in &roots {
            let document = parse_xml(path, self.required_entry(path)?)?;
            let element = document.root_element();
            let namespace = element.tag_name().namespace();
            for outline in element
                .descendants()
                .filter(|node| is_archive_action_element(*node, "OutlineElem", namespace))
            {
                for action in outline
                    .descendants()
                    .filter(|node| is_archive_action_element(*node, "Action", namespace))
                {
                    if let Some(kind) = action
                        .children()
                        .find(|node| node.is_element() && node.tag_name().namespace() == namespace)
                    {
                        if !is_archive_action_element(kind, "Goto", namespace) {
                            issues.push(violation(
                                "OUTLINE_ACTION",
                                ArchiveSeverity::Error,
                                "outline action must be Goto",
                                Some(path),
                                Some(kind.tag_name().name()),
                                Some("Goto"),
                            ));
                        }
                    }
                }
            }
            if element
                .children()
                .any(|node| is_ofd_element(node, "Permissions"))
            {
                issues.push(violation(
                    "PERMISSION",
                    ArchiveSeverity::Warn,
                    "document permission declarations should be removed for OFD-A",
                    Some(path),
                    Some("present"),
                    Some("absent"),
                ));
            }
            if element
                .children()
                .any(|node| is_ofd_element(node, "VPreferences"))
            {
                issues.push(violation(
                    "V_PREFERENCES",
                    ArchiveSeverity::Warn,
                    "initial viewer preferences should be removed for OFD-A",
                    Some(path),
                    Some("present"),
                    Some("absent"),
                ));
            }
            if element
                .children()
                .any(|node| is_ofd_element(node, "Extensions"))
            {
                issues.push(violation(
                    "EXTENSION",
                    ArchiveSeverity::Warn,
                    "document extensions should be removed for OFD-A",
                    Some(path),
                    Some("present"),
                    Some("absent"),
                ));
            }
        }

        for attachment in self.attachments()? {
            if !attachment
                .format
                .as_deref()
                .is_some_and(|format| matches!(format.to_ascii_uppercase().as_str(), "TXT" | "XML"))
            {
                issues.push(violation(
                    "ATTACHMENT",
                    ArchiveSeverity::Error,
                    "OFD-A attachments should use TXT or XML format",
                    Some(&attachment.path),
                    attachment.format.as_deref(),
                    Some("TXT/XML"),
                ));
            }
        }

        let annotation_paths = self
            .annotations()?
            .into_iter()
            .map(|annotation| annotation.source_path)
            .collect::<HashSet<_>>();
        for path in &annotation_paths {
            let xml = parse_xml(path, self.required_entry(path)?)?;
            for annotation in xml
                .root_element()
                .children()
                .filter(|node| is_ofd_element(*node, "Annot"))
            {
                for flag in ["ReadOnly", "NoZoom", "NoRotate"] {
                    if annotation.attribute(flag) != Some("true") {
                        issues.push(violation(
                            "ANNOTATION",
                            ArchiveSeverity::Warn,
                            &format!("annotation {flag} should be true"),
                            Some(path),
                            annotation.attribute(flag),
                            Some("true"),
                        ));
                    }
                }
                if annotation
                    .children()
                    .filter(|node| is_ofd_element(*node, "Appearance"))
                    .any(|appearance| {
                        appearance
                            .children()
                            .any(|node| is_ofd_element(node, "PageBlock"))
                    })
                {
                    issues.push(violation(
                        "ANNOTATION",
                        ArchiveSeverity::Warn,
                        "annotation appearance contains PageBlock",
                        Some(path),
                        Some("PageBlock"),
                        Some("flattened appearance"),
                    ));
                }
            }
        }

        for body in ofd
            .children()
            .filter(|node| is_ofd_element(*node, "DocBody"))
        {
            let Some(index_loc) = child_text_ofd(body, "Signatures") else {
                continue;
            };
            let index_path = resolve_path("", &index_loc)?;
            if self.entry(&index_path)?.is_none() {
                issues.push(violation(
                    "SIGNATURE",
                    ArchiveSeverity::Error,
                    "declared signatures index is missing",
                    Some(&index_path),
                    Some("missing"),
                    Some("valid signatures index"),
                ));
                continue;
            }
            let index_bytes = self.required_entry(&index_path)?;
            let index = parse_xml(&index_path, index_bytes)?;
            for signature in index
                .root_element()
                .children()
                .filter(|node| is_ofd_element(*node, "Signature"))
            {
                let kind = signature.attribute("Type").unwrap_or("Seal");
                issues.push(violation(
                    "SIGNATURE",
                    ArchiveSeverity::Error,
                    "OFD-A requires archival treatment of digital signatures and seals",
                    Some(&index_path),
                    Some(kind),
                    Some("archival annotation"),
                ));
            }
        }

        for (path, bytes) in &self.entries {
            if !is_xml_package_path(path) {
                continue;
            }
            let Ok(xml) = parse_xml(path, bytes) else {
                continue;
            };
            let action_namespace = archive_action_namespace(xml.root_element());
            for node in xml.descendants().filter(Node::is_element) {
                for attribute in node.attributes() {
                    let value = attribute.value().trim();
                    if is_external_url(value) {
                        issues.push(violation(
                            "EXTERNAL_RESOURCE",
                            ArchiveSeverity::Error,
                            "external URL reference is not allowed in OFD-A",
                            Some(path),
                            Some(value),
                            Some("package path"),
                        ));
                    }
                }
                if [
                    "MediaFile",
                    "FontFile",
                    "Profile",
                    "FileLoc",
                    "DocRoot",
                    "DocumentRes",
                    "PublicRes",
                    "PageRes",
                ]
                .iter()
                .any(|name| is_ofd_element(node, name))
                {
                    let value = element_text(node);
                    let value = value.trim();
                    if is_external_url(value) {
                        issues.push(violation(
                            "EXTERNAL_RESOURCE",
                            ArchiveSeverity::Error,
                            "external URL reference is not allowed in OFD-A",
                            Some(path),
                            Some(value),
                            Some("package path"),
                        ));
                    }
                }
                if let Some(namespace) = action_namespace
                    .filter(|namespace| is_archive_action_element(node, "Action", *namespace))
                {
                    if let Some(action) = node.children().find(|child| {
                        child.is_element() && child.tag_name().namespace() == namespace
                    }) {
                        if !is_archive_action_element(action, "Goto", namespace) {
                            issues.push(violation(
                                "NON_GOTO_ACTION",
                                ArchiveSeverity::Error,
                                "OFD-A permits only Goto actions",
                                Some(path),
                                Some(action.tag_name().name()),
                                Some("Goto"),
                            ));
                        }
                    }
                }
            }
        }

        let mut resource_paths = HashSet::new();
        let mut public_paths = HashSet::new();
        for path in &roots {
            let document = parse_xml(path, self.required_entry(path)?)?;
            if let Some(common) = descendant(document.root_element(), "CommonData") {
                public_paths.extend(self.document_resource_paths(path, common, "PublicRes")?);
            }
        }
        for index in 0..roots.len() {
            resource_paths.extend(self.document_multimedia_paths(index)?);
        }
        let pages = self.pages()?;
        let mut page_paths = HashSet::new();
        let mut page_paths_by_document: HashMap<usize, HashSet<String>> = HashMap::new();
        for page in &pages {
            for source in self.page_sources(page.index)? {
                let paths = self.page_resource_paths(&source.path)?;
                page_paths.extend(paths.iter().cloned());
                page_paths_by_document
                    .entry(page.document_index)
                    .or_default()
                    .extend(paths.iter().cloned());
                resource_paths.extend(paths);
            }
        }
        let mut resource_paths = resource_paths.into_iter().collect::<Vec<_>>();
        resource_paths.sort();
        for path in &resource_paths {
            if self.entry(path)?.is_none() {
                continue;
            }
            let bytes = self.required_entry(path)?;
            let resource = parse_xml(path, bytes)?;
            for container in resource.root_element().children().filter(Node::is_element) {
                if !has_ofd_namespace(container) {
                    continue;
                }
                let name = container.tag_name().name();
                if public_paths.contains(path)
                    && matches!(name, "MultiMedias" | "CompositeGraphicUnits" | "DrawParams")
                {
                    issues.push(violation(
                        "RESOURCE_PLACEMENT",
                        ArchiveSeverity::Warn,
                        "resource type belongs in DocumentRes or PageRes",
                        Some(path),
                        Some(name),
                        Some("DocumentRes/PageRes"),
                    ));
                }
                if page_paths.contains(path) && matches!(name, "Fonts" | "ColorSpaces") {
                    issues.push(violation(
                        "RESOURCE_PLACEMENT",
                        ArchiveSeverity::Warn,
                        "resource type belongs in PublicRes",
                        Some(path),
                        Some(name),
                        Some("PublicRes"),
                    ));
                }
            }
            for space in resource
                .descendants()
                .filter(|node| is_ofd_descendant(*node, resource.root_element(), "ColorSpace"))
            {
                match space.attribute("Type") {
                    None => issues.push(violation(
                        "COLOR_SPACE",
                        ArchiveSeverity::Warn,
                        "color space has no type",
                        Some(path),
                        Some("absent"),
                        Some("GRAY/RGB/CMYK"),
                    )),
                    Some(kind)
                        if !matches!(
                            kind.to_ascii_uppercase().as_str(),
                            "GRAY" | "RGB" | "CMYK"
                        ) =>
                    {
                        issues.push(violation(
                            "COLOR_SPACE",
                            ArchiveSeverity::Error,
                            "unsupported color space type",
                            Some(path),
                            Some(kind),
                            Some("GRAY/RGB/CMYK"),
                        ));
                    }
                    _ => {}
                }
                if descendant(space, "Profile").is_none() {
                    issues.push(violation(
                        "COLOR_PROFILE",
                        ArchiveSeverity::Info,
                        "color space should include an ICC profile",
                        Some(path),
                        Some("absent"),
                        Some("ICC Profile"),
                    ));
                }
            }
            for font in resource
                .descendants()
                .filter(|node| is_ofd_descendant(*node, resource.root_element(), "Font"))
            {
                match descendant(font, "FontFile").and_then(nonempty_element_text) {
                    None => issues.push(violation(
                        "FONT_SUBSET",
                        ArchiveSeverity::Error,
                        "font is not embedded",
                        Some(path),
                        Some("missing"),
                        Some("embedded font file"),
                    )),
                    Some(loc) if !is_external_url(loc.trim()) => {
                        let base = parent_path(path);
                        let base = match resource.root_element().attribute("BaseLoc") {
                            Some(loc) => resolve_resource_base(&base, loc)?,
                            None => base,
                        };
                        let font_path = resolve_path(&base, loc.trim())?;
                        if self.matching_entry_path(&font_path)?.is_none() {
                            issues.push(violation(
                                "FONT_SUBSET",
                                ArchiveSeverity::Error,
                                "embedded font file is missing",
                                Some(&font_path),
                                Some("missing"),
                                Some("valid font file"),
                            ));
                        }
                    }
                    Some(_) => {}
                }
            }
            for media in resource
                .descendants()
                .filter(|node| is_ofd_descendant(*node, resource.root_element(), "MultiMedia"))
            {
                let Some(kind) = media.attribute("Type") else {
                    continue;
                };
                if kind.eq_ignore_ascii_case("Image") {
                    for child in media.children().filter(Node::is_element) {
                        if !has_ofd_namespace(child) {
                            issues.push(violation(
                                "IMAGE_EXTENSION",
                                ArchiveSeverity::Warn,
                                "image resource contains custom extension data",
                                Some(path),
                                Some(child.tag_name().name()),
                                Some("remove extension"),
                            ));
                        }
                    }
                    if let Some(loc) = descendant(media, "MediaFile")
                        .and_then(nonempty_element_text)
                        .filter(|loc| !is_external_url(loc.trim()))
                    {
                        let base = parent_path(path);
                        let base = match resource.root_element().attribute("BaseLoc") {
                            Some(loc) => resolve_resource_base(&base, loc)?,
                            None => base,
                        };
                        let image_path = resolve_path(&base, loc.trim())?;
                        match self
                            .matching_entry_path(&image_path)?
                            .map(|matched| self.required_raw_entry(&matched))
                            .transpose()?
                        {
                            None => issues.push(violation(
                                "IMAGE_FORMAT",
                                ArchiveSeverity::Error,
                                "image resource file is missing",
                                Some(&image_path),
                                Some("missing"),
                                Some("BMP/JPEG/PNG/JBIG2/JPEG2000/TIFF"),
                            )),
                            Some(image) => match archive_image_format(image) {
                                Some("GIF") => issues.push(violation(
                                    "IMAGE_FORMAT",
                                    ArchiveSeverity::Error,
                                    "GIF is not an OFD-A image format",
                                    Some(&image_path),
                                    Some("GIF"),
                                    Some("BMP/JPEG/PNG/JBIG2/JPEG2000/TIFF"),
                                )),
                                None => issues.push(violation(
                                    "IMAGE_FORMAT",
                                    ArchiveSeverity::Warn,
                                    "image format is not recognized by its file signature",
                                    Some(&image_path),
                                    Some("unknown"),
                                    Some("BMP/JPEG/PNG/JBIG2/JPEG2000/TIFF"),
                                )),
                                _ => {}
                            },
                        }
                    }
                }
                if kind.eq_ignore_ascii_case("Audio") || kind.eq_ignore_ascii_case("Video") {
                    issues.push(violation(
                        "AUDIO_VIDEO",
                        ArchiveSeverity::Error,
                        "OFD-A does not permit audio or video resources",
                        Some(path),
                        Some(kind),
                        Some("absent"),
                    ));
                }
            }
        }

        let mut image_references: HashMap<(usize, String), HashSet<usize>> = HashMap::new();
        for page in pages {
            for source in self.page_sources(page.index)? {
                let document = parse_xml(&source.path, self.required_entry(&source.path)?)?;
                for image in document
                    .descendants()
                    .filter(|node| is_ofd_element(*node, "ImageObject"))
                {
                    if let Some(id) = image.attribute("ResourceID") {
                        image_references
                            .entry((page.document_index, id.to_owned()))
                            .or_default()
                            .insert(page.index);
                    }
                }
                if source.template_id.is_none() {
                    archive_page_content_violations(&document, &page.path, &mut issues);
                    archive_layer_name_violations(&document, &page.path, &mut issues);
                }
            }
        }

        let mut extra_image_paths = annotation_paths;
        for page in self.pages()? {
            for source in self.page_sources(page.index)? {
                if source.template_id.is_some() {
                    extra_image_paths.insert(source.path);
                }
            }
        }
        for path in extra_image_paths {
            let xml = parse_xml(&path, self.required_entry(&path)?)?;
            archive_page_content_violations(&xml, &path, &mut issues);
        }

        for ((document_index, id), page_indexes) in image_references {
            if page_indexes.len() < 2 {
                continue;
            }
            let Some(paths) = page_paths_by_document.get(&document_index) else {
                continue;
            };
            for path in paths {
                if self.entry(path)?.is_none() {
                    continue;
                }
                let bytes = self.required_entry(path)?;
                let resource = parse_xml(path, bytes)?;
                if resource.descendants().any(|node| {
                    is_ofd_descendant(node, resource.root_element(), "MultiMedia")
                        && node.attribute("ID") == Some(id.as_str())
                        && node
                            .attribute("Type")
                            .is_some_and(|kind| kind.eq_ignore_ascii_case("Image"))
                }) {
                    issues.push(violation(
                        "IMAGE_RESOURCE_REG",
                        ArchiveSeverity::Warn,
                        "image used by multiple pages should be registered in DocumentRes",
                        Some(path),
                        Some("PageRes"),
                        Some("DocumentRes"),
                    ));
                }
            }
        }

        issues.sort_by(|left, right| {
            (&left.rule, &left.path, &left.message).cmp(&(&right.rule, &right.path, &right.message))
        });
        Ok(issues)
    }
}

const ARCHIVE_STROKE_ATTRIBUTES: &[&str] = &[
    "LineWidth",
    "Cap",
    "Join",
    "MiterLimit",
    "DashPattern",
    "DashOffset",
    "StrokeColor",
];

fn redundant_archive_paint_attribute(node: Node<'_, '_>, name: &str) -> bool {
    if is_ofd_element(node, "CompositeObject") {
        return ARCHIVE_STROKE_ATTRIBUTES.contains(&name)
            || matches!(name, "Stroke" | "Fill" | "FillColor");
    }
    if is_ofd_element(node, "ImageObject") {
        return matches!(name, "Fill" | "FillColor");
    }
    (is_ofd_element(node, "TextObject") || is_ofd_element(node, "PathObject"))
        && node
            .attribute("Stroke")
            .is_some_and(|value| value.eq_ignore_ascii_case("false"))
        && ARCHIVE_STROKE_ATTRIBUTES.contains(&name)
}

fn archive_page_content_violations(
    document: &XmlDocument<'_>,
    path: &str,
    issues: &mut Vec<ArchiveViolation>,
) {
    for object in document.descendants().filter(Node::is_element) {
        for attribute in object
            .attributes()
            .filter(|attribute| redundant_archive_paint_attribute(object, attribute.name()))
        {
            issues.push(violation(
                "REDUNDANT_PAINT",
                ArchiveSeverity::Warn,
                "object paint attribute has no effect and should be removed for OFD-A",
                Some(path),
                Some(attribute.name()),
                Some("absent"),
            ));
        }
    }
    for clip in document
        .descendants()
        .filter(|node| is_ofd_element(*node, "ClipArea"))
    {
        if let Some(boundary) = clip.attribute("Boundary") {
            if let Some([_, _, width, height]) = parse_box(boundary) {
                if width <= 0.0 || height <= 0.0 {
                    issues.push(violation(
                        "CLIP_AREA",
                        ArchiveSeverity::Warn,
                        "zero-area clip should be removed",
                        Some(path),
                        Some(boundary),
                        Some("Visible=false"),
                    ));
                }
            }
        }
    }
    for text in document
        .descendants()
        .filter(|node| is_ofd_element(*node, "TextObject"))
    {
        if text.attribute("Size").is_none_or(str::is_empty) {
            issues.push(violation(
                "TEXT_SIZE",
                ArchiveSeverity::Warn,
                "TextObject should declare Size",
                Some(path),
                Some("absent"),
                Some("Size"),
            ));
        }
        if text.attribute("HScale").is_none() {
            if let Some(ctm) = text.attribute("CTM") {
                if let Some([a, b, c, d, _, _]) = parse_ctm(ctm) {
                    if b.abs() < 1e-6
                        && c.abs() < 1e-6
                        && (d - 1.0).abs() < 1e-6
                        && (a - 1.0).abs() > 1e-6
                    {
                        issues.push(violation(
                            "TEXT_HSCALE",
                            ArchiveSeverity::Warn,
                            "pure horizontal text scaling should use HScale",
                            Some(path),
                            Some(ctm),
                            Some("HScale"),
                        ));
                    }
                }
            }
        }
    }
    for image in document
        .descendants()
        .filter(|node| is_ofd_element(*node, "ImageObject"))
    {
        if image
            .attribute("Interpolate")
            .is_some_and(|value| value.eq_ignore_ascii_case("true"))
        {
            issues.push(violation(
                "IMAGE_INTERPOLATE",
                ArchiveSeverity::Warn,
                "image interpolation should be disabled for OFD-A",
                Some(path),
                Some("true"),
                Some("false"),
            ));
        }
    }
    for block in document
        .descendants()
        .filter(|node| is_ofd_element(*node, "PageBlock"))
    {
        let depth = block
            .ancestors()
            .filter(|node| is_ofd_element(*node, "PageBlock"))
            .count();
        if depth == 4 {
            issues.push(violation(
                "PAGEBLOCK_DEPTH",
                ArchiveSeverity::Error,
                "PageBlock nesting exceeds three levels",
                Some(path),
                Some(&depth.to_string()),
                Some("<=3"),
            ));
        }
    }
}

fn direct_archive_layers<'a, 'input>(
    document: &'a XmlDocument<'input>,
) -> impl Iterator<Item = Node<'a, 'input>> {
    document
        .root_element()
        .children()
        .find(|node| is_ofd_element(*node, "Content"))
        .into_iter()
        .flat_map(|content| {
            content
                .children()
                .filter(|node| is_ofd_element(*node, "Layer"))
        })
}

fn archive_layer_name_violations(
    document: &XmlDocument<'_>,
    path: &str,
    issues: &mut Vec<ArchiveViolation>,
) {
    let mut names = HashSet::new();
    for layer in direct_archive_layers(document) {
        if let Some(name) = layer.attribute("Name") {
            if !names.insert(name) {
                issues.push(violation(
                    "LAYER_NAME",
                    ArchiveSeverity::Error,
                    "layer names must be unique within a page",
                    Some(path),
                    Some(name),
                    Some("unique Layer Name"),
                ));
            }
        }
    }
}

fn normalize_archive_layer_names(document: &mut OfdDocument, path: &str) -> Result<(), OfdError> {
    let bytes = document.required_entry(path)?;
    let source = std::str::from_utf8(bytes).map_err(|error| OfdError::Xml {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    let parsed = parse_xml(path, bytes)?;
    let mut names = HashSet::new();
    let mut edits = Vec::new();
    for layer in direct_archive_layers(&parsed) {
        let Some(name) = layer.attribute("Name") else {
            continue;
        };
        if names.insert(name.to_owned()) {
            continue;
        }
        let mut suffix = 2usize;
        let replacement = loop {
            let candidate = format!("{name}_{suffix}");
            if names.insert(candidate.clone()) {
                break candidate;
            }
            suffix = suffix
                .checked_add(1)
                .ok_or_else(|| OfdError::Limit("too many duplicate archival layer names".into()))?;
        };
        let escaped = escape_xml(&replacement)
            .replace('\t', "&#x9;")
            .replace('\n', "&#xA;")
            .replace('\r', "&#xD;");
        if let Some(attribute) = layer.attribute_node("Name") {
            edits.push((attribute.range(), format!("Name=\"{escaped}\"")));
        }
    }
    if !edits.is_empty() {
        document.set_entry(path, apply_archive_edits(source, edits).into_bytes())?;
    }
    Ok(())
}

fn scale_archive_delta_x(encoded: &str, scale: f64) -> Option<String> {
    let mut tokens = encoded.split_whitespace();
    let mut output = Vec::new();
    while let Some(token) = tokens.next() {
        if token == "g" {
            let count = tokens.next()?.parse::<usize>().ok()?;
            let value = tokens.next()?.parse::<f64>().ok()?;
            let scaled = value * scale;
            if !value.is_finite() || !scaled.is_finite() {
                return None;
            }
            output.push(format!("g {count} {scaled}"));
        } else {
            let value = token.parse::<f64>().ok()?;
            let scaled = value * scale;
            if !value.is_finite() || !scaled.is_finite() {
                return None;
            }
            output.push(scaled.to_string());
        }
    }
    Some(output.join(" "))
}

fn normalize_archive_page_content(document: &mut OfdDocument, path: &str) -> Result<(), OfdError> {
    normalize_archive_image_interpolation(document, path)?;
    normalize_archive_redundant_paint(document, path)?;
    let bytes = document.required_entry(path)?;
    let source = std::str::from_utf8(bytes).map_err(|error| OfdError::Xml {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    let xml = parse_xml(path, bytes)?;
    let mut edits = Vec::new();
    let mut empty_clip_ids = HashSet::new();
    for clip in xml
        .descendants()
        .filter(|node| is_ofd_element(*node, "ClipArea"))
    {
        let Some([_, _, width, height]) = clip.attribute("Boundary").and_then(parse_box) else {
            continue;
        };
        if width > 0.0 && height > 0.0 {
            continue;
        }
        if let Some(id) = clip.attribute("ID") {
            empty_clip_ids.insert(id);
        }
        edits.push((clip.range(), String::new()));
    }
    for object in xml.descendants().filter(|node| has_ofd_namespace(*node)) {
        let Some(clip) = object
            .attributes()
            .find(|attr| attr.name() == "Clip" && empty_clip_ids.contains(attr.value()))
        else {
            continue;
        };
        edits.push((clip.range(), String::new()));
        if let Some(visible) = object.attributes().find(|attr| attr.name() == "Visible") {
            edits.push((visible.range(), "Visible=\"false\"".to_owned()));
        } else {
            let tag = qualified_tag(source, object)?;
            let offset = object.range().start + 1 + tag.len();
            edits.push((offset..offset, " Visible=\"false\"".to_owned()));
        }
    }
    for text in xml
        .descendants()
        .filter(|node| is_ofd_element(*node, "TextObject"))
    {
        if text.attribute("HScale").is_some()
            || text.attribute("Clip").is_some()
            || text.attribute("DrawParam").is_some()
            || text
                .attribute("CharDirection")
                .is_some_and(|value| value != "0")
            || text.ancestors().any(|ancestor| {
                is_ofd_element(ancestor, "Layer") && ancestor.attribute("DrawParam").is_some()
            })
            || text.children().any(|child| {
                is_ofd_element(child, "Clips")
                    || (is_ofd_element(child, "FillColor") || is_ofd_element(child, "StrokeColor"))
                        && (child.attribute("Value").is_none()
                            || child.children().any(|node| node.is_element()))
            })
            || text
                .children()
                .any(|child| is_ofd_element(child, "CGTransform"))
                && parse_cg_transforms(text).is_err()
        {
            continue;
        }
        let Some(ctm) = text
            .attributes()
            .find(|attribute| attribute.name() == "CTM")
        else {
            continue;
        };
        let Some([scale, b, c, d, e, f]) = parse_ctm(ctm.value()) else {
            continue;
        };
        if scale <= 0.0
            || b.abs() >= 1e-6
            || c.abs() >= 1e-6
            || (d - 1.0).abs() >= 1e-6
            || (scale - 1.0).abs() <= 1e-6
        {
            continue;
        }
        let code_edits = (|| {
            let mut code_edits = Vec::new();
            let mut has_code = false;
            for code in text
                .children()
                .filter(|child| is_ofd_element(*child, "TextCode"))
            {
                has_code = true;
                if decode_text_code(&element_text(code)).ok()?.is_empty() {
                    return None;
                }
                if let Some(x) = code.attributes().find(|attribute| attribute.name() == "X") {
                    let value = x.value().parse::<f64>().ok()? * scale;
                    if !value.is_finite() {
                        return None;
                    }
                    code_edits.push((x.range(), format!("X=\"{value}\"")));
                }
                if let Some(delta) = code
                    .attributes()
                    .find(|attribute| attribute.name() == "DeltaX")
                {
                    let values = scale_archive_delta_x(delta.value(), scale)?;
                    code_edits.push((delta.range(), format!("DeltaX=\"{values}\"")));
                }
            }
            has_code.then_some(code_edits)
        })();
        let Some(code_edits) = code_edits else {
            continue;
        };
        edits.extend(code_edits);
        edits.push((ctm.range(), format!("CTM=\"1 0 0 1 {e} {f}\"")));
        let tag = qualified_tag(source, text)?;
        let offset = text.range().start + 1 + tag.len();
        edits.push((offset..offset, format!(" HScale=\"{scale}\"")));
    }
    if !edits.is_empty() {
        document.set_entry(path, apply_archive_edits(source, edits).into_bytes())?;
    }
    flatten_archive_page_blocks(document, path, 3, false)?;
    Ok(())
}

fn normalize_archive_image_interpolation(
    document: &mut OfdDocument,
    path: &str,
) -> Result<(), OfdError> {
    let bytes = document.required_entry(path)?;
    let source = std::str::from_utf8(bytes).map_err(|error| OfdError::Xml {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    let xml = parse_xml(path, bytes)?;
    let edits = xml
        .descendants()
        .filter(|node| is_ofd_element(*node, "ImageObject"))
        .filter_map(|node| {
            node.attributes().find(|attribute| {
                attribute.name() == "Interpolate" && attribute.value().eq_ignore_ascii_case("true")
            })
        })
        .map(|attribute| (attribute.range(), "Interpolate=\"false\"".to_owned()))
        .collect::<Vec<_>>();
    if !edits.is_empty() {
        document.set_entry(path, apply_archive_edits(source, edits).into_bytes())?;
    }
    Ok(())
}

fn normalize_archive_redundant_paint(
    document: &mut OfdDocument,
    path: &str,
) -> Result<(), OfdError> {
    let bytes = document.required_entry(path)?;
    let source = std::str::from_utf8(bytes).map_err(|error| OfdError::Xml {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    let xml = parse_xml(path, bytes)?;
    let edits = xml
        .descendants()
        .filter(Node::is_element)
        .flat_map(|node| {
            node.attributes()
                .filter(move |attribute| redundant_archive_paint_attribute(node, attribute.name()))
                .map(|attribute| (attribute.range(), String::new()))
        })
        .collect::<Vec<_>>();
    if !edits.is_empty() {
        document.set_entry(path, apply_archive_edits(source, edits).into_bytes())?;
    }
    Ok(())
}

fn flatten_archive_page_blocks(
    document: &mut OfdDocument,
    path: &str,
    allowed_depth: usize,
    appearance_only: bool,
) -> Result<(), OfdError> {
    for _ in 0..10_000 {
        let source = std::str::from_utf8(document.required_entry(path)?)
            .map_err(|error| OfdError::Xml {
                path: path.to_owned(),
                message: error.to_string(),
            })?
            .to_owned();
        let xml = parse_xml(path, source.as_bytes())?;
        let mut edit = None;
        for block in xml
            .descendants()
            .filter(|node| is_ofd_element(*node, "PageBlock"))
        {
            if block
                .ancestors()
                .filter(|node| is_ofd_element(*node, "PageBlock"))
                .count()
                <= allowed_depth
                || (appearance_only
                    && !block
                        .ancestors()
                        .any(|node| is_ofd_element(node, "Appearance")))
                || block.attributes().next().is_some()
            {
                continue;
            }
            let fragment = &source[block.range()];
            let opening_end = fragment.find('>').ok_or_else(|| {
                OfdError::Invalid(format!("PageBlock in {path} has no opening tag"))
            })?;
            let opening = &fragment[..=opening_end];
            if opening.contains("xmlns") {
                continue;
            }
            let replacement = if opening.ends_with("/>") {
                String::new()
            } else {
                let closing = fragment.rfind("</").ok_or_else(|| {
                    OfdError::Invalid(format!("PageBlock in {path} has no closing tag"))
                })?;
                fragment[opening_end + 1..closing].to_owned()
            };
            edit = Some((block.range(), replacement));
            break;
        }
        let Some((range, replacement)) = edit else {
            return Ok(());
        };
        let mut updated = source;
        updated.replace_range(range, &replacement);
        parse_xml(path, updated.as_bytes())?;
        document.set_entry(path, updated.into_bytes())?;
    }
    Err(OfdError::Limit(
        "archival PageBlock flattening exceeded 10000 wrappers".into(),
    ))
}

fn violation(
    rule: &str,
    severity: ArchiveSeverity,
    message: &str,
    path: Option<&str>,
    actual: Option<&str>,
    expected: Option<&str>,
) -> ArchiveViolation {
    ArchiveViolation {
        rule: rule.to_owned(),
        severity,
        message: message.to_owned(),
        path: path.map(str::to_owned),
        actual: actual.map(str::to_owned),
        expected: expected.map(str::to_owned),
    }
}

fn is_external_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn archive_action_namespace<'a, 'input>(root: Node<'a, 'input>) -> Option<Option<&'a str>> {
    if matches!(root.tag_name().name(), "Document" | "Page" | "Annotations")
        && has_ofd_namespace(root)
    {
        Some(root.tag_name().namespace())
    } else {
        None
    }
}

fn is_archive_action_element(node: Node<'_, '_>, name: &str, namespace: Option<&str>) -> bool {
    node.is_element() && node.tag_name().name() == name && node.tag_name().namespace() == namespace
}

fn archive_goto_action(action: Node<'_, '_>, namespace: Option<&str>) -> bool {
    action
        .children()
        .find(|node| node.is_element() && node.tag_name().namespace() == namespace)
        .is_some_and(|kind| is_archive_action_element(kind, "Goto", namespace))
}

fn prune_archive_obsolete_entries(document: &mut OfdDocument, paths: HashSet<String>) {
    for path in paths {
        // The package entry point is an implicit reference, not an XML link.
        if path.eq_ignore_ascii_case("OFD.xml") {
            continue;
        }
        let basename = path
            .rsplit('/')
            .next()
            .unwrap_or(&path)
            .to_ascii_lowercase();
        let absolute = path.to_ascii_lowercase();
        let referenced = document.entries.iter().any(|(entry_path, bytes)| {
            if !is_xml_package_path(entry_path) {
                return false;
            }
            let bytes = document
                .xml_views
                .get(entry_path)
                .map(Vec::as_slice)
                .unwrap_or(bytes);
            let Ok(xml) = parse_xml(entry_path, bytes) else {
                return true;
            };
            xml.descendants().filter(Node::is_element).any(|node| {
                node.attributes()
                    .any(|attr| archive_path_value_matches(attr.value(), &absolute, &basename))
                    || archive_path_value_matches(&element_text(node), &absolute, &basename)
            })
        });
        if !referenced {
            document.entries.remove(&path);
            document.xml_views.remove(&path);
        }
    }
}

fn archive_path_value_matches(value: &str, absolute: &str, basename: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value == absolute
        || value.strip_prefix('/') == Some(absolute)
        || value == basename
        || value
            .rsplit_once('/')
            .is_some_and(|(_, tail)| tail == basename)
}

fn relocate_public_resources(document: &mut OfdDocument) -> Result<(), OfdError> {
    let root_path = document.doc_roots()?.remove(0);
    let document_bytes = document.required_entry(&root_path)?;
    let document_xml = parse_xml(&root_path, document_bytes)?;
    let common = descendant(document_xml.root_element(), "CommonData")
        .ok_or_else(|| OfdError::Invalid(format!("{root_path} has no CommonData")))?;
    let public_paths = document.document_resource_paths(&root_path, common, "PublicRes")?;
    let mut linked_document_paths =
        document.document_resource_paths(&root_path, common, "DocumentRes")?;

    for public_path in public_paths {
        if document.entry(&public_path)?.is_none() {
            continue;
        }
        let public_bytes = document.required_entry(&public_path)?;
        let public_source = std::str::from_utf8(public_bytes)
            .map_err(|error| OfdError::Xml {
                path: public_path.clone(),
                message: error.to_string(),
            })?
            .to_owned();
        let public_xml = parse_xml(&public_path, public_bytes)?;
        let public_root = public_xml.root_element();
        let containers = public_root
            .children()
            .filter(|node| {
                has_ofd_namespace(*node)
                    && matches!(
                        node.tag_name().name(),
                        "MultiMedias" | "CompositeGraphicUnits" | "DrawParams"
                    )
            })
            .collect::<Vec<_>>();
        if containers.is_empty() {
            continue;
        }
        let public_base = match public_root.attribute("BaseLoc") {
            Some(loc) => resolve_resource_base(&parent_path(&public_path), loc)?,
            None => parent_path(&public_path),
        };

        let mut compatible_destination = None;
        for path in &linked_document_paths {
            if path == &public_path {
                continue;
            }
            if document.entry(path)?.is_none() {
                continue;
            }
            let bytes = document.required_entry(path)?;
            let xml = parse_xml(path, bytes)?;
            let base = match xml.root_element().attribute("BaseLoc") {
                Some(loc) => resolve_resource_base(&parent_path(path), loc)?,
                None => parent_path(path),
            };
            if base != public_base
                || xml.root_element().tag_name().namespace() != public_root.tag_name().namespace()
                || !resource_namespaces_compatible(public_root, xml.root_element())
                || qualified_tag(&public_source, public_root)?
                    != qualified_tag(
                        std::str::from_utf8(bytes).map_err(|error| OfdError::Xml {
                            path: path.clone(),
                            message: error.to_string(),
                        })?,
                        xml.root_element(),
                    )?
            {
                continue;
            }
            compatible_destination = Some((
                path.clone(),
                std::str::from_utf8(bytes)
                    .map_err(|error| OfdError::Xml {
                        path: path.clone(),
                        message: error.to_string(),
                    })?
                    .to_owned(),
            ));
            break;
        }
        let (destination_path, mut destination_source, is_new) =
            if let Some((path, source)) = compatible_destination {
                (path, source, false)
            } else {
                let mut index = 0usize;
                let path = loop {
                    let name = if index == 0 {
                        "DocumentRes_Archive.xml".to_owned()
                    } else {
                        format!("DocumentRes_Archive_{index}.xml")
                    };
                    let path = resolve_path(&parent_path(&public_path), &name)?;
                    if document.matching_entry_path(&path)?.is_none() {
                        break path;
                    }
                    index += 1;
                };
                let markup = &public_source[public_root.range()];
                let Some(open_end) = markup.find('>') else {
                    continue;
                };
                if markup[..open_end].ends_with('/') {
                    continue;
                }
                let tag = qualified_tag(&public_source, public_root)?;
                (path, format!("{}</{tag}>", &markup[..=open_end]), true)
            };

        let mut public_edits = Vec::new();
        for container in containers {
            let name = container.tag_name().name();
            if container
                .children()
                .any(|node| node.is_element() && !has_ofd_namespace(node))
            {
                continue;
            }
            let children = container
                .children()
                .filter(Node::is_element)
                .collect::<Vec<_>>();
            if children.is_empty() {
                public_edits.push((container.range(), String::new()));
                continue;
            }
            let moving_ids = children
                .iter()
                .filter_map(|node| node.attribute("ID"))
                .collect::<HashSet<_>>();
            let mut conflicting_document_id = false;
            for path in &linked_document_paths {
                if path == &destination_path {
                    continue;
                }
                if document.entry(path)?.is_some() {
                    let bytes = document.required_entry(path)?;
                    let xml = parse_xml(path, bytes)?;
                    conflicting_document_id |= xml
                        .root_element()
                        .children()
                        .filter(|node| is_ofd_element(*node, name))
                        .flat_map(|node| node.children().filter(Node::is_element))
                        .filter_map(|node| node.attribute("ID"))
                        .any(|id| moving_ids.contains(id));
                }
            }
            if conflicting_document_id {
                continue;
            }
            let destination_xml = parse_xml(&destination_path, destination_source.as_bytes())?;
            let destination_root = destination_xml.root_element();
            if let Some(existing) = destination_root
                .children()
                .find(|node| is_ofd_element(*node, name))
            {
                let existing_ids = existing
                    .children()
                    .filter(Node::is_element)
                    .filter_map(|node| node.attribute("ID"))
                    .collect::<HashSet<_>>();
                if children.iter().any(|node| {
                    node.attribute("ID")
                        .is_some_and(|id| existing_ids.contains(id))
                }) {
                    continue;
                }
                let contents = children
                    .iter()
                    .map(|node| &public_source[node.range()])
                    .collect::<String>();
                destination_source = insert_xml_child(&destination_source, existing, &contents)?;
            } else {
                destination_source = insert_xml_child(
                    &destination_source,
                    destination_root,
                    &public_source[container.range()],
                )?;
            }
            public_edits.push((container.range(), String::new()));
        }
        if public_edits.is_empty() {
            continue;
        }
        document.set_entry(&destination_path, destination_source.into_bytes())?;
        document.set_entry(
            &public_path,
            apply_archive_edits(&public_source, public_edits).into_bytes(),
        )?;
        if is_new {
            linked_document_paths.push(destination_path.clone());
            let bytes = document.required_entry(&root_path)?;
            let source = std::str::from_utf8(bytes).map_err(|error| OfdError::Xml {
                path: root_path.clone(),
                message: error.to_string(),
            })?;
            let xml = parse_xml(&root_path, bytes)?;
            let common = descendant(xml.root_element(), "CommonData")
                .ok_or_else(|| OfdError::Invalid(format!("{root_path} has no CommonData")))?;
            let max = descendant(common, "MaxUnitID")
                .ok_or_else(|| OfdError::Invalid(format!("{root_path} has no MaxUnitID")))?;
            let tag = qualified_tag(source, common)?;
            let prefix = tag
                .rsplit_once(':')
                .map_or(String::new(), |(prefix, _)| format!("{prefix}:"));
            let relative = snapshot_relative_path(&root_path, &destination_path);
            let mut updated = source.to_owned();
            updated.insert_str(
                max.range().start,
                &format!("<{prefix}DocumentRes>{relative}</{prefix}DocumentRes>"),
            );
            document.set_entry(&root_path, updated.into_bytes())?;
        }
    }
    Ok(())
}

fn relocate_page_resources(document: &mut OfdDocument) -> Result<(), OfdError> {
    let root_path = document.doc_roots()?.remove(0);
    let root = parse_xml(&root_path, document.required_entry(&root_path)?)?;
    let common = descendant(root.root_element(), "CommonData")
        .ok_or_else(|| OfdError::Invalid(format!("{root_path} has no CommonData")))?;
    let mut public_paths = document.document_resource_paths(&root_path, common, "PublicRes")?;
    if public_paths.len() > 1 {
        return Ok(());
    }
    let document_paths = document.document_resource_paths(&root_path, common, "DocumentRes")?;
    let mut page_paths = HashSet::new();
    for (index, _) in document.pages()?.iter().enumerate() {
        for source in document.page_sources(index)? {
            page_paths.extend(document.page_resource_paths(&source.path)?);
        }
    }
    let mut page_paths = page_paths.into_iter().collect::<Vec<_>>();
    page_paths.sort();
    let mut page_id_owners: HashMap<(String, String), HashSet<String>> = HashMap::new();
    for page_path in &page_paths {
        if document.entry(page_path)?.is_none() {
            continue;
        }
        let bytes = document.required_entry(page_path)?;
        let xml = parse_xml(page_path, bytes)?;
        for container in xml.root_element().children().filter(|node| {
            has_ofd_namespace(*node) && matches!(node.tag_name().name(), "Fonts" | "ColorSpaces")
        }) {
            for id in container
                .children()
                .filter(Node::is_element)
                .filter_map(|node| node.attribute("ID"))
            {
                page_id_owners
                    .entry((container.tag_name().name().to_owned(), id.to_owned()))
                    .or_default()
                    .insert(page_path.clone());
            }
        }
    }

    for page_path in page_paths {
        if document.entry(&page_path)?.is_none() {
            continue;
        }
        let page_bytes = document.required_entry(&page_path)?;
        let page_source = std::str::from_utf8(page_bytes)
            .map_err(|error| OfdError::Xml {
                path: page_path.clone(),
                message: error.to_string(),
            })?
            .to_owned();
        let page_xml = parse_xml(&page_path, page_bytes)?;
        let page_root = page_xml.root_element();
        let containers = page_root
            .children()
            .filter(|node| {
                has_ofd_namespace(*node)
                    && matches!(node.tag_name().name(), "Fonts" | "ColorSpaces")
            })
            .collect::<Vec<_>>();
        if containers.is_empty() {
            continue;
        }
        let page_base = match page_root.attribute("BaseLoc") {
            Some(loc) => resolve_resource_base(&parent_path(&page_path), loc)?,
            None => parent_path(&page_path),
        };
        let new_public = if public_paths.is_empty() {
            let mut index = 0usize;
            let path = loop {
                let name = if index == 0 {
                    "PublicRes_Archive.xml".to_owned()
                } else {
                    format!("PublicRes_Archive_{index}.xml")
                };
                let path = resolve_path(&parent_path(&page_path), &name)?;
                if document.matching_entry_path(&path)?.is_none() {
                    break path;
                }
                index += 1;
            };
            let markup = &page_source[page_root.range()];
            let Some(open_end) = markup.find('>') else {
                continue;
            };
            if markup[..open_end].ends_with('/') {
                continue;
            }
            let tag = qualified_tag(&page_source, page_root)?;
            Some((path, format!("{}</{tag}>", &markup[..=open_end])))
        } else {
            None
        };
        let destinations = new_public
            .as_ref()
            .map_or_else(|| public_paths.clone(), |(path, _)| vec![path.clone()]);
        for public_path in destinations {
            if public_path == page_path {
                continue;
            }
            let public_bytes = if let Some((_, source)) = &new_public {
                Some(source.as_bytes())
            } else {
                document
                    .entry(&public_path)?
                    .map(|_| document.required_entry(&public_path))
                    .transpose()?
            };
            let Some(public_bytes) = public_bytes else {
                continue;
            };
            let mut public_source = std::str::from_utf8(public_bytes)
                .map_err(|error| OfdError::Xml {
                    path: public_path.clone(),
                    message: error.to_string(),
                })?
                .to_owned();
            let public_xml = parse_xml(&public_path, public_bytes)?;
            let public_root = public_xml.root_element();
            let public_base = match public_root.attribute("BaseLoc") {
                Some(loc) => resolve_resource_base(&parent_path(&public_path), loc)?,
                None => parent_path(&public_path),
            };
            if page_base != public_base
                || page_root.tag_name().namespace() != public_root.tag_name().namespace()
                || !resource_namespaces_compatible(page_root, public_root)
                || qualified_tag(&page_source, page_root)?
                    != qualified_tag(&public_source, public_root)?
            {
                continue;
            }

            let mut page_edits = Vec::new();
            for container in &containers {
                let name = container.tag_name().name();
                if container
                    .children()
                    .any(|node| node.is_element() && !has_ofd_namespace(node))
                {
                    continue;
                }
                let children = container
                    .children()
                    .filter(Node::is_element)
                    .collect::<Vec<_>>();
                if children.is_empty() {
                    page_edits.push((container.range(), String::new()));
                    continue;
                }
                let moving_ids = children
                    .iter()
                    .filter_map(|node| node.attribute("ID"))
                    .collect::<HashSet<_>>();
                if moving_ids.iter().any(|id| {
                    page_id_owners
                        .get(&(name.to_owned(), (*id).to_owned()))
                        .is_some_and(|owners| owners.len() > 1)
                }) {
                    continue;
                }
                let mut conflicting_document_id = false;
                for document_path in &document_paths {
                    if document.entry(document_path)?.is_some() {
                        let bytes = document.required_entry(document_path)?;
                        let xml = parse_xml(document_path, bytes)?;
                        conflicting_document_id |= xml
                            .root_element()
                            .children()
                            .filter(|node| is_ofd_element(*node, name))
                            .flat_map(|node| node.children().filter(Node::is_element))
                            .filter_map(|node| node.attribute("ID"))
                            .any(|id| moving_ids.contains(id));
                    }
                }
                if conflicting_document_id {
                    continue;
                }
                let public_xml = parse_xml(&public_path, public_source.as_bytes())?;
                let public_root = public_xml.root_element();
                if let Some(existing) = public_root
                    .children()
                    .find(|node| is_ofd_element(*node, name))
                {
                    let existing_ids = existing
                        .children()
                        .filter(Node::is_element)
                        .filter_map(|node| node.attribute("ID"))
                        .collect::<HashSet<_>>();
                    if children.iter().any(|node| {
                        node.attribute("ID")
                            .is_some_and(|id| existing_ids.contains(id))
                    }) {
                        continue;
                    }
                    let contents = children
                        .iter()
                        .map(|node| &page_source[node.range()])
                        .collect::<String>();
                    public_source = insert_xml_child(&public_source, existing, &contents)?;
                } else {
                    public_source = insert_xml_child(
                        &public_source,
                        public_root,
                        &page_source[container.range()],
                    )?;
                }
                page_edits.push((container.range(), String::new()));
            }
            if !page_edits.is_empty() {
                document.set_entry(&public_path, public_source.into_bytes())?;
                document.set_entry(
                    &page_path,
                    apply_archive_edits(&page_source, page_edits).into_bytes(),
                )?;
                if new_public.is_some() {
                    let bytes = document.required_entry(&root_path)?;
                    let source = std::str::from_utf8(bytes).map_err(|error| OfdError::Xml {
                        path: root_path.clone(),
                        message: error.to_string(),
                    })?;
                    let xml = parse_xml(&root_path, bytes)?;
                    let common = descendant(xml.root_element(), "CommonData").ok_or_else(|| {
                        OfdError::Invalid(format!("{root_path} has no CommonData"))
                    })?;
                    let max = descendant(common, "MaxUnitID").ok_or_else(|| {
                        OfdError::Invalid(format!("{root_path} has no MaxUnitID"))
                    })?;
                    let tag = qualified_tag(source, common)?;
                    let prefix = tag
                        .rsplit_once(':')
                        .map_or(String::new(), |(prefix, _)| format!("{prefix}:"));
                    let relative = snapshot_relative_path(&root_path, &public_path);
                    let mut updated = source.to_owned();
                    updated.insert_str(
                        max.range().start,
                        &format!("<{prefix}PublicRes>{relative}</{prefix}PublicRes>"),
                    );
                    document.set_entry(&root_path, updated.into_bytes())?;
                    public_paths.push(public_path);
                }
            }
            break;
        }
    }
    Ok(())
}

fn resource_namespaces_compatible(source: Node<'_, '_>, destination: Node<'_, '_>) -> bool {
    source.namespaces().all(|namespace| {
        destination.lookup_namespace_uri(namespace.name()) == Some(namespace.uri())
    })
}

fn raise_archive_max_unit_id(document: &mut OfdDocument) -> Result<(), OfdError> {
    let root_path = document.doc_roots()?.remove(0);
    let mut highest_id = 0usize;
    for (path, bytes) in &document.entries {
        if !is_xml_package_path(path) {
            continue;
        }
        let Ok(xml) = parse_xml(path, bytes) else {
            continue;
        };
        let root = xml.root_element();
        let namespace = root.tag_name().namespace();
        if !has_ofd_namespace(root)
            || !matches!(
                root.tag_name().name(),
                "OFD"
                    | "Document"
                    | "Page"
                    | "Res"
                    | "Attachments"
                    | "Annotations"
                    | "PageAnnot"
                    | "Signatures"
                    | "Versions"
                    | "CustomTags"
                    | "Extensions"
            )
        {
            continue;
        }
        for node in xml.descendants().filter(Node::is_element) {
            if node.tag_name().namespace() != namespace {
                continue;
            }
            if let Some(id) = node.attribute("ID").and_then(|id| id.parse::<usize>().ok()) {
                highest_id = highest_id.max(id);
            }
        }
    }
    let bytes = document.required_entry(&root_path)?;
    let source = std::str::from_utf8(bytes).map_err(|error| OfdError::Xml {
        path: root_path.clone(),
        message: error.to_string(),
    })?;
    let xml = parse_xml(&root_path, bytes)?;
    let Some(max_node) = descendant(xml.root_element(), "CommonData")
        .and_then(|common| descendant(common, "MaxUnitID"))
    else {
        return Ok(());
    };
    let Ok(current) = element_text(max_node).trim().parse::<usize>() else {
        return Ok(());
    };
    if highest_id > current {
        let mut updated = source.to_owned();
        updated.replace_range(
            element_content_range(max_node, source)?,
            &highest_id.to_string(),
        );
        document.set_entry(&root_path, updated.into_bytes())?;
    }
    Ok(())
}

pub(super) fn apply_archive_edits(
    source: &str,
    mut edits: Vec<(std::ops::Range<usize>, String)>,
) -> String {
    edits.sort_by_key(|edit| edit.0.start);
    let mut disjoint: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    for edit in edits {
        if disjoint
            .last()
            .is_some_and(|previous| edit.0.start < previous.0.end)
        {
            continue;
        }
        disjoint.push(edit);
    }
    let mut updated = source.to_owned();
    for (range, replacement) in disjoint.into_iter().rev() {
        updated.replace_range(range, &replacement);
    }
    updated
}

fn archive_image_format(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("PNG")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("JPEG")
    } else if bytes.starts_with(b"BM") {
        Some("BMP")
    } else if bytes.starts_with(&justbig2::header::MAGIC) {
        Some("JBIG2")
    } else if is_jpeg2000(bytes) {
        Some("JPEG2000")
    } else if bytes.starts_with(b"II\x2a\x00") || bytes.starts_with(b"MM\x00\x2a") {
        Some("TIFF")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("GIF")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redundant_archive_paint_rules_cover_text_path_image_and_composite_objects() {
        let path = "Doc_0/Pages/Page_0/Content.xml";
        let source = br#"<ofd:Page xmlns:ofd="http://www.ofdspec.org/2016"><ofd:TextObject Stroke="false" LineWidth="2" Cap="Round"/><ofd:PathObject Stroke="FALSE" DashPattern="2 1"/><ofd:ImageObject Fill="true" FillColor="0 0 0"/><ofd:CompositeObject Stroke="true" Fill="true" Join="Round"/><ofd:TextObject Stroke="true" LineWidth="2"/></ofd:Page>"#;
        let xml = parse_xml(path, source).unwrap();
        let names = xml
            .descendants()
            .filter(Node::is_element)
            .flat_map(|node| {
                node.attributes()
                    .filter(move |attribute| {
                        redundant_archive_paint_attribute(node, attribute.name())
                    })
                    .map(|attribute| attribute.name())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "LineWidth",
                "Cap",
                "DashPattern",
                "Fill",
                "FillColor",
                "Stroke",
                "Fill",
                "Join"
            ]
        );
        let mut document = OfdDocument::new();
        document.set_entry(path, source.to_vec()).unwrap();
        normalize_archive_redundant_paint(&mut document, path).unwrap();
        let normalized = parse_xml(path, document.entry(path).unwrap().unwrap()).unwrap();
        assert!(normalized
            .descendants()
            .filter(Node::is_element)
            .all(|node| node
                .attributes()
                .all(|attribute| !redundant_archive_paint_attribute(node, attribute.name()))));
        assert_eq!(
            normalized
                .descendants()
                .rfind(|node| is_element(*node, "TextObject"))
                .unwrap()
                .attribute("LineWidth"),
            Some("2")
        );
    }

    #[test]
    fn archival_preparation_removes_redundant_paint_from_a_copy() {
        let mut source = OfdDocument::new();
        source.add_text_page("archive", 40.0, 30.0).unwrap();
        let path = "Doc_0/Pages/Page_0/Content.xml";
        let original = std::str::from_utf8(source.entry(path).unwrap().unwrap()).unwrap();
        let updated = original.replacen(
            "<ofd:TextObject ",
            "<ofd:TextObject Stroke=\"false\" LineWidth=\"2\" Cap=\"Round\" ",
            1,
        );
        assert_ne!(updated, original);
        source.set_entry(path, updated.into_bytes()).unwrap();
        let before = source.to_bytes().unwrap();
        assert_eq!(
            source
                .archive_violations()
                .unwrap()
                .iter()
                .filter(|issue| issue.rule == "REDUNDANT_PAINT")
                .count(),
            2
        );
        let prepared = source.prepare_archive().unwrap();
        assert_eq!(source.to_bytes().unwrap(), before);
        let content = std::str::from_utf8(prepared.entry(path).unwrap().unwrap()).unwrap();
        assert!(content.contains("Stroke=\"false\""));
        assert!(!content.contains("LineWidth=\"2\""));
        assert!(!content.contains("Cap=\"Round\""));
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "REDUNDANT_PAINT"));
        assert_eq!(prepared.page_text(0).unwrap(), source.page_text(0).unwrap());
    }

    #[test]
    fn archival_preparation_removes_view_preferences_from_a_copy() {
        let mut source = OfdDocument::new();
        source.add_blank_page(40.0, 30.0).unwrap();
        let path = "Doc_0/Document.xml";
        let original = std::str::from_utf8(source.entry(path).unwrap().unwrap()).unwrap();
        let updated = original.replace(
            "</ofd:Document>",
            "<ofd:VPreferences><ofd:PageMode>UseOutlines</ofd:PageMode></ofd:VPreferences></ofd:Document>",
        );
        assert_ne!(updated, original);
        source.set_entry(path, updated.into_bytes()).unwrap();
        let before = source.to_bytes().unwrap();
        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "V_PREFERENCES" && issue.path.as_deref() == Some(path)));

        let prepared = source.prepare_archive().unwrap();
        assert_eq!(source.to_bytes().unwrap(), before);
        assert!(!std::str::from_utf8(prepared.entry(path).unwrap().unwrap())
            .unwrap()
            .contains("VPreferences"));
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "V_PREFERENCES"));
        assert_eq!(
            prepared.prepare_archive().unwrap().entry(path).unwrap(),
            prepared.entry(path).unwrap()
        );
    }

    #[test]
    fn duplicate_page_layer_names_are_reported_and_normalized() {
        let mut source = OfdDocument::new();
        source.add_blank_page(40.0, 30.0).unwrap();
        while source.page_layers(0).unwrap().len() < 4 {
            source.add_page_layer(0, "Body", None).unwrap();
        }
        let path = source.pages().unwrap()[0].path.clone();
        let bytes = source.entry(&path).unwrap().unwrap();
        let xml = parse_xml(&path, bytes).unwrap();
        let text = std::str::from_utf8(bytes).unwrap();
        let mut edits = Vec::new();
        for (layer, name) in direct_archive_layers(&xml).zip(["A", "A_2", "A", "A_2"]) {
            let tag = qualified_tag(text, layer).unwrap();
            let offset = layer.range().start + 1 + tag.len();
            edits.push((offset..offset, format!(" Name=\"{name}\"")));
        }
        source
            .set_entry(&path, apply_archive_edits(text, edits).into_bytes())
            .unwrap();
        let before = source.to_bytes().unwrap();
        let issues = source.archive_violations().unwrap();
        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.rule == "LAYER_NAME")
                .count(),
            2
        );
        let prepared = source.prepare_archive().unwrap();
        assert_eq!(source.to_bytes().unwrap(), before);
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "LAYER_NAME"));
        let parsed = parse_xml(&path, prepared.entry(&path).unwrap().unwrap()).unwrap();
        let names = direct_archive_layers(&parsed)
            .filter_map(|layer| layer.attribute("Name"))
            .collect::<Vec<_>>();
        assert_eq!(names, ["A", "A_2", "A_3", "A_2_2"]);
        let twice = prepared.prepare_archive().unwrap();
        assert_eq!(twice.entry(&path).unwrap(), prepared.entry(&path).unwrap());
    }

    fn archival_document() -> OfdDocument {
        let mut document = OfdDocument::new();
        document.add_text_page("archived", 100.0, 100.0).unwrap();
        let resource = "Doc_0/PublicRes.xml";
        let source = std::str::from_utf8(document.entry(resource).unwrap().unwrap()).unwrap();
        let updated = source.replace(
            "FamilyName=\"SimSun\"/>",
            "FamilyName=\"SimSun\"><ofd:FontFile>DejaVuSans.ttf</ofd:FontFile></ofd:Font>",
        );
        document.set_entry(resource, updated.into_bytes()).unwrap();
        document
            .set_entry(
                "Doc_0/Res/DejaVuSans.ttf",
                include_bytes!("../tests/fixtures/dejavu-sans.ttf").to_vec(),
            )
            .unwrap();
        let source = std::str::from_utf8(document.entry("OFD.xml").unwrap().unwrap()).unwrap();
        let updated = source.replace("DocType=\"OFD\"", "DocType=\"OFD-A\"");
        document.set_entry("OFD.xml", updated.into_bytes()).unwrap();
        document
    }

    fn archival_document_with_page_resources() -> OfdDocument {
        let mut document = archival_document();
        let public_path = "Doc_0/PublicRes.xml";
        let public_source = std::str::from_utf8(document.entry(public_path).unwrap().unwrap())
            .unwrap()
            .to_owned();
        let xml = parse_xml(public_path, public_source.as_bytes()).unwrap();
        let fonts = descendant(xml.root_element(), "Fonts").unwrap();
        let fonts_xml = &public_source[fonts.range()];
        document
            .set_entry(
                public_path,
                public_source.replace(fonts_xml, "").into_bytes(),
            )
            .unwrap();
        document
            .set_entry(
                "Doc_0/PageRes.xml",
                format!(
                    "<ofd:Res xmlns:ofd=\"{OFD_NS}\" BaseLoc=\"Res\">{fonts_xml}<ofd:ColorSpaces><ofd:ColorSpace ID=\"42\" Type=\"RGB\"/></ofd:ColorSpaces></ofd:Res>"
                )
                .into_bytes(),
            )
            .unwrap();
        let page_path = "Doc_0/Pages/Page_0/Content.xml";
        let page_source = std::str::from_utf8(document.entry(page_path).unwrap().unwrap()).unwrap();
        document
            .set_entry(
                page_path,
                page_source
                    .replace(
                        "</ofd:Page>",
                        "<ofd:PageRes>../../PageRes.xml</ofd:PageRes></ofd:Page>",
                    )
                    .into_bytes(),
            )
            .unwrap();
        let root_path = "Doc_0/Document.xml";
        let root_source = std::str::from_utf8(document.entry(root_path).unwrap().unwrap())
            .unwrap()
            .to_owned();
        let xml = parse_xml(root_path, root_source.as_bytes()).unwrap();
        let max = descendant(xml.root_element(), "MaxUnitID").unwrap();
        document
            .set_entry(
                root_path,
                root_source
                    .replace(
                        &root_source[max.range()],
                        "<ofd:MaxUnitID>42</ofd:MaxUnitID>",
                    )
                    .into_bytes(),
            )
            .unwrap();
        document
    }

    #[test]
    fn archival_preparation_moves_page_fonts_and_color_spaces() {
        let source = archival_document_with_page_resources();
        let before = source.page_svg(0).unwrap();
        assert_eq!(
            source.page_font_data(0, 0).unwrap(),
            include_bytes!("../tests/fixtures/dejavu-sans.ttf")
        );
        assert_eq!(
            source
                .archive_violations()
                .unwrap()
                .iter()
                .filter(|issue| issue.rule == "RESOURCE_PLACEMENT")
                .count(),
            2
        );

        let prepared = source.prepare_archive().unwrap();
        assert_eq!(prepared.page_svg(0).unwrap(), before);
        assert_eq!(
            prepared.page_font_data(0, 0).unwrap(),
            include_bytes!("../tests/fixtures/dejavu-sans.ttf")
        );
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "RESOURCE_PLACEMENT"));
        let public =
            std::str::from_utf8(prepared.entry("Doc_0/PublicRes.xml").unwrap().unwrap()).unwrap();
        assert!(public.contains("<ofd:Fonts>"));
        assert!(public.contains("<ofd:ColorSpaces>"));
        let page =
            std::str::from_utf8(prepared.entry("Doc_0/PageRes.xml").unwrap().unwrap()).unwrap();
        assert!(!page.contains("<ofd:Fonts>"));
        assert!(!page.contains("<ofd:ColorSpaces>"));
    }

    #[test]
    fn archival_preparation_checks_and_converts_template_only_page_resources() {
        let mut source = archival_document_with_page_resources();
        let mut gif = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut gif, image::ImageFormat::Gif)
            .unwrap();
        source
            .set_entry("Doc_0/Res/template.gif", gif.into_inner())
            .unwrap();
        let resource_path = "Doc_0/PageRes.xml";
        let resource = std::str::from_utf8(source.entry(resource_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                resource_path,
                resource
                    .replace(
                        "</ofd:Res>",
                        "<ofd:MultiMedias><ofd:MultiMedia ID=\"43\" Type=\"Image\" Format=\"GIF\"><ofd:MediaFile>template.gif</ofd:MediaFile></ofd:MultiMedia></ofd:MultiMedias></ofd:Res>",
                    )
                    .into_bytes(),
            )
            .unwrap();
        source
            .promote_page_to_template(0, &[], "Background")
            .unwrap();
        let template_path = source.page_sources(0).unwrap()[0].path.clone();
        let template = std::str::from_utf8(source.entry(&template_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                &template_path,
                template
                    .replace(
                        "</ofd:Layer>",
                        "<ofd:ImageObject ID=\"44\" ResourceID=\"43\" Boundary=\"10 10 10 10\"/></ofd:Layer>",
                    )
                    .into_bytes(),
            )
            .unwrap();
        let page_path = source.pages().unwrap()[0].path.clone();
        let page = std::str::from_utf8(source.entry(&page_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                &page_path,
                page.replace("<ofd:PageRes>../../PageRes.xml</ofd:PageRes>", "")
                    .into_bytes(),
            )
            .unwrap();
        let root_path = "Doc_0/Document.xml";
        let root = std::str::from_utf8(source.entry(root_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                root_path,
                root.replace(
                    "<ofd:MaxUnitID>43</ofd:MaxUnitID>",
                    "<ofd:MaxUnitID>44</ofd:MaxUnitID>",
                )
                .into_bytes(),
            )
            .unwrap();
        let before = source.page_png(0, 25.4).unwrap();
        assert!(
            source.validate_references().unwrap().is_empty(),
            "{:?}",
            source.validate_references().unwrap()
        );
        let issues = source.archive_violations().unwrap();
        assert!(issues.iter().any(|issue| issue.rule == "IMAGE_FORMAT"
            && issue.path.as_deref() == Some("Doc_0/Res/template.gif")));
        assert!(issues.iter().any(|issue| issue.rule == "RESOURCE_PLACEMENT"
            && issue.path.as_deref() == Some(resource_path)));

        let prepared = source.prepare_archive().unwrap();
        assert_eq!(prepared.page_png(0, 25.4).unwrap(), before);
        assert!(prepared.validate_references().unwrap().is_empty());
        assert!(prepared.archive_violations().unwrap().iter().all(|issue| {
            !matches!(issue.rule.as_str(), "IMAGE_FORMAT" | "RESOURCE_PLACEMENT")
        }));
        let resource =
            std::str::from_utf8(prepared.entry(resource_path).unwrap().unwrap()).unwrap();
        assert!(resource.contains("Format=\"PNG\""));
        assert!(!resource.contains("<ofd:Fonts>"));
        assert!(prepared.entry("Doc_0/Res/template.gif").unwrap().is_none());
    }

    #[test]
    fn archival_preparation_creates_missing_public_resource() {
        let mut source = archival_document_with_page_resources();
        let root_path = "Doc_0/Document.xml";
        let root_source = std::str::from_utf8(source.entry(root_path).unwrap().unwrap())
            .unwrap()
            .to_owned();
        let xml = parse_xml(root_path, root_source.as_bytes()).unwrap();
        let public = descendant(xml.root_element(), "PublicRes").unwrap();
        source
            .set_entry(
                root_path,
                root_source
                    .replace(&root_source[public.range()], "")
                    .into_bytes(),
            )
            .unwrap();
        assert!(source.fonts().unwrap().is_empty());
        assert_eq!(
            source.page_fonts(0).unwrap()[0].resource_path,
            "Doc_0/PageRes.xml"
        );
        let before = source.page_svg(0).unwrap();

        let prepared = source.prepare_archive().unwrap();
        let public_path = "Doc_0/PublicRes_Archive.xml";
        let root = std::str::from_utf8(prepared.entry(root_path).unwrap().unwrap()).unwrap();
        assert!(root.contains("<ofd:PublicRes>PublicRes_Archive.xml</ofd:PublicRes>"));
        let public = std::str::from_utf8(prepared.entry(public_path).unwrap().unwrap()).unwrap();
        assert!(public.contains("<ofd:Fonts>"));
        assert!(public.contains("<ofd:ColorSpaces>"));
        assert_eq!(
            prepared.page_fonts(0).unwrap()[0].resource_path,
            public_path
        );
        assert_eq!(prepared.page_svg(0).unwrap(), before);
        assert!(
            prepared.validate_references().unwrap().is_empty(),
            "{:?}",
            prepared.validate_references().unwrap()
        );
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "RESOURCE_PLACEMENT"));
    }

    #[test]
    fn archival_preparation_raises_stale_max_unit_id() {
        let mut source = archival_document_with_page_resources();
        let root_path = "Doc_0/Document.xml";
        let root = std::str::from_utf8(source.entry(root_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                root_path,
                root.replace(
                    "<ofd:MaxUnitID>42</ofd:MaxUnitID>",
                    "<ofd:MaxUnitID>5</ofd:MaxUnitID>",
                )
                .into_bytes(),
            )
            .unwrap();
        source
            .set_entry(
                "Doc_0/Extension.xml",
                b"<Custom xmlns=\"urn:example:custom\" ID=\"1000\"/>".to_vec(),
            )
            .unwrap();
        assert!(source
            .validate_references()
            .unwrap()
            .iter()
            .any(|issue| issue.code == "id_exceeds_max"));

        let prepared = source.prepare_archive().unwrap();
        let root = std::str::from_utf8(prepared.entry(root_path).unwrap().unwrap()).unwrap();
        assert!(root.contains("<ofd:MaxUnitID>42</ofd:MaxUnitID>"));
        assert!(prepared.validate_references().unwrap().is_empty());
        assert!(
            std::str::from_utf8(source.entry(root_path).unwrap().unwrap())
                .unwrap()
                .contains("<ofd:MaxUnitID>5</ofd:MaxUnitID>")
        );
    }

    #[test]
    fn archival_preparation_replaces_a_split_max_unit_id_as_one_value() {
        let mut source = archival_document_with_page_resources();
        let root_path = "Doc_0/Document.xml";
        let root = std::str::from_utf8(source.entry(root_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                root_path,
                root.replace(
                    "<ofd:MaxUnitID>42</ofd:MaxUnitID>",
                    "<ofd:MaxUnitID>0<!-- split -->5</ofd:MaxUnitID>",
                )
                .into_bytes(),
            )
            .unwrap();
        let prepared = source.prepare_archive().unwrap();
        let root = std::str::from_utf8(prepared.entry(root_path).unwrap().unwrap()).unwrap();
        assert!(root.contains("<ofd:MaxUnitID>42</ofd:MaxUnitID>"));
        assert!(!root.contains("split"));
        assert!(prepared.validate_references().unwrap().is_empty());
    }

    #[test]
    fn archival_checks_resolve_split_font_file_path() {
        let mut source = archival_document();
        let resource_path = "Doc_0/PublicRes.xml";
        let resource = std::str::from_utf8(source.entry(resource_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                resource_path,
                resource
                    .replace("DejaVuSans.ttf", "DejaVu<![CDATA[Sans]]>.ttf")
                    .into_bytes(),
            )
            .unwrap();
        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "FONT_SUBSET"));
    }

    #[test]
    fn archival_preparation_counts_unqualified_ofd_resource_ids() {
        let mut source = archival_document_with_page_resources();
        let page_resource = "Doc_0/PageRes.xml";
        let resource = std::str::from_utf8(source.entry(page_resource).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                page_resource,
                resource
                    .replace(&format!(" xmlns:ofd=\"{OFD_NS}\""), "")
                    .replace("ofd:", "")
                    .into_bytes(),
            )
            .unwrap();
        let root_path = "Doc_0/Document.xml";
        let root = std::str::from_utf8(source.entry(root_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                root_path,
                root.replace(
                    "<ofd:MaxUnitID>42</ofd:MaxUnitID>",
                    "<ofd:MaxUnitID>5</ofd:MaxUnitID>",
                )
                .into_bytes(),
            )
            .unwrap();
        assert!(source
            .validate_references()
            .unwrap()
            .iter()
            .any(|issue| issue.code == "id_exceeds_max"));

        let prepared = source.prepare_archive().unwrap();
        assert!(
            std::str::from_utf8(prepared.entry(root_path).unwrap().unwrap())
                .unwrap()
                .contains("<ofd:MaxUnitID>42</ofd:MaxUnitID>")
        );
        assert!(prepared.validate_references().unwrap().is_empty());
    }

    #[test]
    fn archival_preparation_links_nested_public_resource_without_overwriting_entry() {
        let mut source = archival_document_with_page_resources();
        let root_path = "Doc_0/Document.xml";
        let root_source = std::str::from_utf8(source.entry(root_path).unwrap().unwrap())
            .unwrap()
            .to_owned();
        let xml = parse_xml(root_path, root_source.as_bytes()).unwrap();
        let public = descendant(xml.root_element(), "PublicRes").unwrap();
        source
            .set_entry(
                root_path,
                root_source
                    .replace(&root_source[public.range()], "")
                    .into_bytes(),
            )
            .unwrap();

        let nested = "Doc_0/Pages/Page_0/Res.xml";
        let old = std::str::from_utf8(source.entry("Doc_0/PageRes.xml").unwrap().unwrap()).unwrap();
        source
            .set_entry(
                nested,
                old.replace("BaseLoc=\"Res\"", "BaseLoc=\"../../Res\"")
                    .into_bytes(),
            )
            .unwrap();
        let page_path = "Doc_0/Pages/Page_0/Content.xml";
        let page = std::str::from_utf8(source.entry(page_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                page_path,
                page.replace("../../PageRes.xml", "Res.xml").into_bytes(),
            )
            .unwrap();
        let occupied = "Doc_0/Pages/Page_0/PublicRes_Archive.xml";
        source.set_entry(occupied, b"<Unused/>".to_vec()).unwrap();
        let before = source.page_svg(0).unwrap();

        let prepared = source.prepare_archive().unwrap();
        let public_path = "Doc_0/Pages/Page_0/PublicRes_Archive_1.xml";
        assert!(
            std::str::from_utf8(prepared.entry(root_path).unwrap().unwrap())
                .unwrap()
                .contains("<ofd:PublicRes>Pages/Page_0/PublicRes_Archive_1.xml</ofd:PublicRes>")
        );
        assert!(
            std::str::from_utf8(prepared.entry(public_path).unwrap().unwrap())
                .unwrap()
                .contains("<ofd:Fonts>")
        );
        assert_eq!(prepared.entry(occupied).unwrap().unwrap(), b"<Unused/>");
        assert_eq!(prepared.page_svg(0).unwrap(), before);
        assert!(prepared.validate_references().unwrap().is_empty());
    }

    #[test]
    fn archival_preparation_keeps_page_resources_with_incompatible_base_or_id() {
        let mut source = archival_document_with_page_resources();
        let page_path = "Doc_0/PageRes.xml";
        let page = std::str::from_utf8(source.entry(page_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                page_path,
                page.replace("BaseLoc=\"Res\"", "BaseLoc=\"Other\"")
                    .into_bytes(),
            )
            .unwrap();
        source
            .set_entry(
                "Doc_0/Other/DejaVuSans.ttf",
                include_bytes!("../tests/fixtures/dejavu-sans.ttf").to_vec(),
            )
            .unwrap();
        let prepared = source.prepare_archive().unwrap();
        assert_eq!(
            prepared.page_font_data(0, 0).unwrap(),
            include_bytes!("../tests/fixtures/dejavu-sans.ttf")
        );
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "RESOURCE_PLACEMENT"));
        assert!(
            std::str::from_utf8(prepared.entry(page_path).unwrap().unwrap())
                .unwrap()
                .contains("<ofd:Fonts>")
        );

        let mut source = archival_document_with_page_resources();
        let public_path = "Doc_0/PublicRes.xml";
        let public = std::str::from_utf8(source.entry(public_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                public_path,
                public
                    .replace(
                        "</ofd:Res>",
                        "<ofd:Fonts><ofd:Font ID=\"1\" FontName=\"Other\"/></ofd:Fonts></ofd:Res>",
                    )
                    .into_bytes(),
            )
            .unwrap();
        let prepared = source.prepare_archive().unwrap();
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "RESOURCE_PLACEMENT"));
        assert!(
            std::str::from_utf8(prepared.entry(page_path).unwrap().unwrap())
                .unwrap()
                .contains("<ofd:Fonts>")
        );
    }

    #[test]
    fn archival_preparation_keeps_resource_with_unavailable_namespace_binding() {
        let mut source = archival_document_with_page_resources();
        let page_path = "Doc_0/PageRes.xml";
        let page = std::str::from_utf8(source.entry(page_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                page_path,
                page.replace(
                    "BaseLoc=\"Res\"",
                    "BaseLoc=\"Res\" xmlns:vendor=\"urn:example:vendor\"",
                )
                .replace(
                    "Type=\"RGB\"/>",
                    "Type=\"RGB\"><vendor:Hint/></ofd:ColorSpace>",
                )
                .into_bytes(),
            )
            .unwrap();

        let prepared = source.prepare_archive().unwrap();
        let page = std::str::from_utf8(prepared.entry(page_path).unwrap().unwrap()).unwrap();
        let parsed = parse_xml(page_path, page.as_bytes()).unwrap();
        assert!(descendant(parsed.root_element(), "Fonts").is_some());
        assert!(descendant(parsed.root_element(), "ColorSpaces").is_some());
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "RESOURCE_PLACEMENT"));
        assert!(prepared.page_svg(0).is_ok());
    }

    #[test]
    fn archival_preparation_keeps_page_font_priority_when_ids_repeat() {
        let mut source = archival_document_with_page_resources();
        let second_path = "Doc_0/PageRes2.xml";
        source
            .set_entry(
                second_path,
                format!(
                    "<ofd:Res xmlns:ofd=\"{OFD_NS}\" BaseLoc=\"Res\"><ofd:Fonts><ofd:Font ID=\"1\" FontName=\"Other\"><ofd:FontFile>DejaVuSans.ttf</ofd:FontFile></ofd:Font></ofd:Fonts></ofd:Res>"
                )
                .into_bytes(),
            )
            .unwrap();
        let page_path = "Doc_0/Pages/Page_0/Content.xml";
        let page = std::str::from_utf8(source.entry(page_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                page_path,
                page.replace(
                    "</ofd:Page>",
                    "<ofd:PageRes>../../PageRes2.xml</ofd:PageRes></ofd:Page>",
                )
                .into_bytes(),
            )
            .unwrap();
        assert_eq!(
            source.page_fonts(0).unwrap()[0].resource_path,
            "Doc_0/PageRes.xml"
        );
        let before = source.page_svg(0).unwrap();

        let prepared = source.prepare_archive().unwrap();
        assert_eq!(
            prepared.page_fonts(0).unwrap()[0].resource_path,
            "Doc_0/PageRes.xml"
        );
        assert_eq!(prepared.page_svg(0).unwrap(), before);
        let first =
            std::str::from_utf8(prepared.entry("Doc_0/PageRes.xml").unwrap().unwrap()).unwrap();
        assert!(first.contains("<ofd:Fonts>"));
        assert!(!first.contains("<ofd:ColorSpaces>"));
        assert!(
            std::str::from_utf8(prepared.entry(second_path).unwrap().unwrap())
                .unwrap()
                .contains("<ofd:Fonts>")
        );
        assert!(prepared.archive_violations().unwrap().iter().any(|issue| {
            issue.rule == "RESOURCE_PLACEMENT" && issue.actual.as_deref() == Some("Fonts")
        }));
    }

    #[test]
    fn checks_archival_document_type_and_body_count() {
        let mut plain = OfdDocument::new();
        plain.add_text_page("ordinary", 100.0, 100.0).unwrap();
        let issues = plain.archive_violations().unwrap();
        assert_eq!(
            issues
                .iter()
                .map(|issue| issue.rule.as_str())
                .collect::<Vec<_>>(),
            ["DOC_TYPE", "FONT_SUBSET"]
        );

        let mut archival = archival_document();
        assert!(archival.archive_violations().unwrap().is_empty());
        archival.append_document_body().unwrap();
        let issues = archival.archive_violations().unwrap();
        assert_eq!(
            issues
                .iter()
                .map(|issue| issue.rule.as_str())
                .collect::<Vec<_>>(),
            ["FONT_SUBSET", "SINGLE_DOC"]
        );
        assert_eq!(issues[1].actual.as_deref(), Some("2"));
    }

    #[test]
    fn checks_archival_permissions_extensions_media_and_page_blocks() {
        let mut document = archival_document();
        let root = "Doc_0/Document.xml";
        let source = std::str::from_utf8(document.entry(root).unwrap().unwrap()).unwrap();
        let updated = source.replace(
            "</ofd:Document>",
            "<ofd:Permissions/><ofd:Extensions/></ofd:Document>",
        );
        document.set_entry(root, updated.into_bytes()).unwrap();

        let resource = "Doc_0/PublicRes.xml";
        let source = std::str::from_utf8(document.entry(resource).unwrap().unwrap()).unwrap();
        let updated = source.replace("</ofd:Res>", "<ofd:MultiMedias><ofd:MultiMedia ID=\"42\" Type=\"Audio\"><ofd:MediaFile>tone.wav</ofd:MediaFile></ofd:MultiMedia></ofd:MultiMedias></ofd:Res>");
        document.set_entry(resource, updated.into_bytes()).unwrap();

        let page = "Doc_0/Pages/Page_0/Content.xml";
        let source = std::str::from_utf8(document.entry(page).unwrap().unwrap()).unwrap();
        let updated = source.replace("</ofd:Layer>", "<ofd:PageBlock><ofd:PageBlock><ofd:PageBlock><ofd:PageBlock/></ofd:PageBlock></ofd:PageBlock></ofd:PageBlock></ofd:Layer>");
        document.set_entry(page, updated.into_bytes()).unwrap();

        let issues = document.archive_violations().unwrap();
        assert_eq!(
            issues
                .iter()
                .map(|issue| issue.rule.as_str())
                .collect::<Vec<_>>(),
            [
                "AUDIO_VIDEO",
                "EXTENSION",
                "PAGEBLOCK_DEPTH",
                "PERMISSION",
                "RESOURCE_PLACEMENT"
            ]
        );
        assert_eq!(issues[0].severity, ArchiveSeverity::Error);
        assert_eq!(issues[1].severity, ArchiveSeverity::Warn);
        assert_eq!(issues[2].actual.as_deref(), Some("4"));
    }

    #[test]
    fn encrypted_package_reports_archival_violation() {
        let document = OfdDocument::from_bytes(include_bytes!(
            "../tests/fixtures/java-password-encrypted.ofd"
        ))
        .unwrap();
        let issues = document.archive_violations().unwrap();
        assert_eq!(issues[0].rule, "ENCRYPTION");
    }

    #[test]
    fn checks_archival_text_and_image_recommendations() {
        let mut document = archival_document();
        let path = "Doc_0/Pages/Page_0/Content.xml";
        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let updated = source
            .replacen(
                "<ofd:TextObject ",
                "<ofd:TextObject CTM=\"2 0 0 1 0 0\" ",
                1,
            )
            .replacen(" Size=\"5\"", "", 1)
            .replace(
                "</ofd:Layer>",
                "<ofd:ImageObject Interpolate=\"true\"/></ofd:Layer>",
            );
        assert_ne!(updated, source);
        document.set_entry(path, updated.into_bytes()).unwrap();
        let issues = document.archive_violations().unwrap();
        assert_eq!(
            issues
                .iter()
                .map(|issue| issue.rule.as_str())
                .collect::<Vec<_>>(),
            ["IMAGE_INTERPOLATE", "TEXT_HSCALE", "TEXT_SIZE"]
        );
    }

    #[test]
    fn archival_image_interpolation_covers_templates_and_annotations() {
        let mut source = archival_document();
        source
            .promote_page_to_template(0, &[], "Background")
            .unwrap();
        let template_path = source
            .page_sources(0)
            .unwrap()
            .into_iter()
            .find(|item| item.template_id.is_some())
            .unwrap()
            .path;
        let template = std::str::from_utf8(source.entry(&template_path).unwrap().unwrap())
            .unwrap()
            .replace(
                "</ofd:Layer>",
                "<ofd:ImageObject Interpolate=\"true\"/></ofd:Layer>",
            );
        source
            .set_entry(&template_path, template.into_bytes())
            .unwrap();

        source
            .add_highlight_annotation(0, [1.0, 2.0, 30.0, 4.0], [255, 255, 0], 128)
            .unwrap();
        let annotation_path = source.annotations().unwrap()[0].source_path.clone();
        let annotation = std::str::from_utf8(source.entry(&annotation_path).unwrap().unwrap())
            .unwrap()
            .replace(
                "</ofd:Appearance>",
                "<ofd:ImageObject Interpolate=\"true\"/></ofd:Appearance>",
            );
        source
            .set_entry(&annotation_path, annotation.into_bytes())
            .unwrap();

        let violations = source.archive_violations().unwrap();
        let interpolation_paths = violations
            .iter()
            .filter(|issue| issue.rule == "IMAGE_INTERPOLATE")
            .map(|issue| issue.path.as_deref().unwrap())
            .collect::<HashSet<_>>();
        assert_eq!(
            interpolation_paths,
            HashSet::from([template_path.as_str(), annotation_path.as_str()])
        );

        let prepared = source.prepare_archive().unwrap();
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "IMAGE_INTERPOLATE"));
        for path in [&template_path, &annotation_path] {
            let xml = std::str::from_utf8(prepared.entry(path).unwrap().unwrap()).unwrap();
            assert!(xml.contains("Interpolate=\"false\""));
        }
        assert_eq!(
            prepared.prepare_archive().unwrap().to_bytes().unwrap(),
            prepared.to_bytes().unwrap()
        );
    }

    #[test]
    fn archival_template_content_checks_and_normalizes_deep_blocks_and_empty_clips() {
        let mut source = archival_document();
        source
            .promote_page_to_template(0, &[], "Background")
            .unwrap();
        let path = source
            .page_sources(0)
            .unwrap()
            .into_iter()
            .find(|item| item.template_id.is_some())
            .unwrap()
            .path;
        let mut template = std::str::from_utf8(source.entry(&path).unwrap().unwrap())
            .unwrap()
            .to_owned();
        template = template.replacen("<ofd:TextObject ", "<ofd:TextObject Clip=\"777\" ", 1);
        let layer = template.find("<ofd:Layer").unwrap();
        let opening_end = layer + template[layer..].find('>').unwrap() + 1;
        template.insert_str(opening_end, &"<ofd:PageBlock>".repeat(4));
        template = template.replace(
            "</ofd:Layer>",
            &format!(
                "<ofd:ClipArea ID=\"777\" Boundary=\"0 0 0 1\"/>{}</ofd:Layer>",
                "</ofd:PageBlock>".repeat(4)
            ),
        );
        source.set_entry(&path, template.into_bytes()).unwrap();

        let violations = source.archive_violations().unwrap();
        assert!(violations
            .iter()
            .any(|issue| issue.rule == "PAGEBLOCK_DEPTH"
                && issue.path.as_deref() == Some(path.as_str())));
        assert!(
            violations
                .iter()
                .any(|issue| issue.rule == "CLIP_AREA"
                    && issue.path.as_deref() == Some(path.as_str()))
        );

        let prepared = source.prepare_archive().unwrap();
        let xml = std::str::from_utf8(prepared.entry(&path).unwrap().unwrap()).unwrap();
        assert!(!xml.contains("<ofd:ClipArea"));
        assert!(xml.contains("Visible=\"false\""));
        assert!(!xml.contains("<ofd:PageBlock><ofd:PageBlock><ofd:PageBlock><ofd:PageBlock>"));
        assert!(prepared.archive_violations().unwrap().iter().all(|issue| {
            issue.path.as_deref() != Some(path.as_str())
                || !matches!(issue.rule.as_str(), "PAGEBLOCK_DEPTH" | "CLIP_AREA")
        }));
    }

    #[test]
    fn checks_external_resources_and_non_goto_actions() {
        let mut document = archival_document();
        let resource = "Doc_0/PublicRes.xml";
        let source = std::str::from_utf8(document.entry(resource).unwrap().unwrap()).unwrap();
        let updated = source.replace("</ofd:Res>", "<ofd:MultiMedias><ofd:MultiMedia ID=\"42\" Type=\"Image\"><ofd:MediaFile>https://example.invalid/image.png</ofd:MediaFile></ofd:MultiMedia></ofd:MultiMedias></ofd:Res>");
        document.set_entry(resource, updated.into_bytes()).unwrap();
        let page = "Doc_0/Pages/Page_0/Content.xml";
        let source = std::str::from_utf8(document.entry(page).unwrap().unwrap()).unwrap();
        let updated = source.replace("</ofd:Page>", "<ofd:Actions><ofd:Action Event=\"PO\"><ofd:URI URI=\"relative\"/></ofd:Action></ofd:Actions></ofd:Page>");
        document.set_entry(page, updated.into_bytes()).unwrap();
        let issues = document.archive_violations().unwrap();
        assert_eq!(
            issues
                .iter()
                .map(|issue| issue.rule.as_str())
                .collect::<Vec<_>>(),
            ["EXTERNAL_RESOURCE", "NON_GOTO_ACTION", "RESOURCE_PLACEMENT"]
        );
        assert_eq!(
            issues[0].actual.as_deref(),
            Some("https://example.invalid/image.png")
        );
        assert_eq!(issues[1].actual.as_deref(), Some("URI"));
    }

    #[test]
    fn archival_check_detects_external_resource_split_by_comment() {
        let mut document = archival_document();
        let resource_path = "Doc_0/PublicRes.xml";
        let resource =
            std::str::from_utf8(document.entry(resource_path).unwrap().unwrap()).unwrap();
        document.set_entry(resource_path, resource.replace(
            "</ofd:Res>",
            "<ofd:MultiMedias><ofd:MultiMedia ID=\"42\" Type=\"Image\"><ofd:MediaFile>https://exa<!-- split -->mple.invalid/image.png</ofd:MediaFile></ofd:MultiMedia></ofd:MultiMedias></ofd:Res>",
        ).into_bytes()).unwrap();
        assert!(document.archive_violations().unwrap().iter().any(|issue| {
            issue.rule == "EXTERNAL_RESOURCE"
                && issue.actual.as_deref() == Some("https://example.invalid/image.png")
        }));
    }

    #[test]
    fn checks_attachment_formats() {
        let mut document = archival_document();
        document.add_attachment("report.pdf", b"PDF").unwrap();
        let issues = document.archive_violations().unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "ATTACHMENT");
        assert_eq!(issues[0].expected.as_deref(), Some("TXT/XML"));

        document
            .set_attachment_metadata(
                0,
                AttachmentMetadata {
                    format: Some("xml".into()),
                    ..AttachmentMetadata::default()
                },
            )
            .unwrap();
        assert!(document.archive_violations().unwrap().is_empty());
    }

    #[test]
    fn recommends_icc_profile_for_color_spaces() {
        let mut document = archival_document();
        let path = "Doc_0/PublicRes.xml";
        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let updated = source.replace(
            "</ofd:Res>",
            "<ofd:ColorSpaces><ofd:ColorSpace ID=\"42\" Type=\"RGB\"/></ofd:ColorSpaces></ofd:Res>",
        );
        document.set_entry(path, updated.into_bytes()).unwrap();
        let issues = document.archive_violations().unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "COLOR_PROFILE");
        assert_eq!(issues[0].severity, ArchiveSeverity::Info);

        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let updated = source.replace(
            "Type=\"RGB\"/>",
            "Type=\"RGB\"><ofd:Profile>color.icc</ofd:Profile></ofd:ColorSpace>",
        );
        document.set_entry(path, updated.into_bytes()).unwrap();
        assert!(document.archive_violations().unwrap().is_empty());
    }

    #[test]
    fn checks_color_space_type() {
        let mut document = archival_document();
        let path = "Doc_0/PublicRes.xml";
        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let updated = source.replace(
            "</ofd:Res>",
            "<ofd:ColorSpaces><ofd:ColorSpace ID=\"42\" Type=\"LAB\"><ofd:Profile>color.icc</ofd:Profile></ofd:ColorSpace></ofd:ColorSpaces></ofd:Res>",
        );
        document.set_entry(path, updated.into_bytes()).unwrap();
        let issues = document.archive_violations().unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, "COLOR_SPACE");
        assert_eq!(issues[0].actual.as_deref(), Some("LAB"));
    }

    #[test]
    fn archival_resource_checks_ignore_extension_namespace_lookalikes() {
        let mut document = archival_document();
        let path = "Doc_0/PublicRes.xml";
        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let extension = "<x:Fonts xmlns:x=\"urn:example:extension\"><x:Font ID=\"90\"/></x:Fonts><x:ColorSpaces xmlns:x=\"urn:example:extension\"><x:ColorSpace ID=\"91\" Type=\"LAB\"/></x:ColorSpaces><x:MultiMedias xmlns:x=\"urn:example:extension\"><x:MultiMedia ID=\"92\" Type=\"Image\"><x:Blob>keep</x:Blob></x:MultiMedia><x:MultiMedia ID=\"93\" Type=\"Audio\"/></x:MultiMedias>";
        document
            .set_entry(
                path,
                source
                    .replace("</ofd:Res>", &format!("{extension}</ofd:Res>"))
                    .into_bytes(),
            )
            .unwrap();
        let issues = document.archive_violations().unwrap();
        for rule in [
            "RESOURCE_PLACEMENT",
            "COLOR_SPACE",
            "COLOR_PROFILE",
            "FONT_SUBSET",
            "IMAGE_EXTENSION",
            "AUDIO_VIDEO",
        ] {
            assert!(issues.iter().all(|issue| issue.rule != rule), "{rule}");
        }
        let prepared = document.prepare_archive().unwrap();
        let result = std::str::from_utf8(prepared.entry(path).unwrap().unwrap()).unwrap();
        assert!(result.contains(extension));
        let reopened = OfdDocument::from_bytes(&prepared.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.entry(path).unwrap(), prepared.entry(path).unwrap());
    }

    #[test]
    fn archival_resource_checks_ignore_ofd_nodes_inside_extension_wrappers() {
        let mut document = archival_document();
        let path = "Doc_0/PublicRes.xml";
        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let extension = "<x:Metadata xmlns:x=\"urn:example:extension\"><ofd:ColorSpace ID=\"91\" Type=\"LAB\"/><ofd:Font ID=\"92\"/><ofd:MultiMedia ID=\"93\" Type=\"Audio\"/></x:Metadata>";
        document
            .set_entry(
                path,
                source
                    .replace("</ofd:Res>", &format!("{extension}</ofd:Res>"))
                    .into_bytes(),
            )
            .unwrap();
        let issues = document.archive_violations().unwrap();
        for rule in ["COLOR_SPACE", "COLOR_PROFILE", "FONT_SUBSET", "AUDIO_VIDEO"] {
            assert!(issues.iter().all(|issue| issue.rule != rule), "{rule}");
        }
        let prepared = document.prepare_archive().unwrap();
        let result = std::str::from_utf8(prepared.entry(path).unwrap().unwrap()).unwrap();
        assert!(result.contains(extension));
    }

    #[test]
    fn archival_page_resource_relocation_preserves_extension_containers() {
        let mut document = archival_document_with_page_resources();
        let path = "Doc_0/PageRes.xml";
        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let extension = "<x:Fonts xmlns:x=\"urn:example:extension\"><x:Font ID=\"90\"/></x:Fonts><x:ColorSpaces xmlns:x=\"urn:example:extension\"><x:ColorSpace ID=\"91\" Type=\"LAB\"/></x:ColorSpaces>";
        document
            .set_entry(
                path,
                source
                    .replace("</ofd:Res>", &format!("{extension}</ofd:Res>"))
                    .into_bytes(),
            )
            .unwrap();
        let prepared = document.prepare_archive().unwrap();
        let result = std::str::from_utf8(prepared.entry(path).unwrap().unwrap()).unwrap();
        assert!(result.contains(extension));
        let issues = prepared.archive_violations().unwrap();
        for rule in ["RESOURCE_PLACEMENT", "COLOR_SPACE", "FONT_SUBSET"] {
            assert!(issues.iter().all(|issue| issue.rule != rule), "{rule}");
        }
    }

    #[test]
    fn archival_resource_relocation_keeps_mixed_and_same_name_extension_containers() {
        let mut document = archival_document_with_page_resources();
        let page_path = "Doc_0/PageRes.xml";
        let page = std::str::from_utf8(document.entry(page_path).unwrap().unwrap()).unwrap();
        let marker = "<x:Metadata xmlns:x=\"urn:example:extension\">keep</x:Metadata>";
        document
            .set_entry(
                page_path,
                page.replace("</ofd:Fonts>", &format!("{marker}</ofd:Fonts>"))
                    .into_bytes(),
            )
            .unwrap();
        let public_path = "Doc_0/PublicRes.xml";
        let public = std::str::from_utf8(document.entry(public_path).unwrap().unwrap()).unwrap();
        let lookalike = "<x:ColorSpaces xmlns:x=\"urn:example:extension\"><x:ColorSpace ID=\"42\"/></x:ColorSpaces>";
        document
            .set_entry(
                public_path,
                public
                    .replace("</ofd:Res>", &format!("{lookalike}</ofd:Res>"))
                    .into_bytes(),
            )
            .unwrap();

        let before = document.page_svg(0).unwrap();
        let prepared = document.prepare_archive().unwrap();
        let page = std::str::from_utf8(prepared.entry(page_path).unwrap().unwrap()).unwrap();
        assert!(page.contains(marker));
        assert!(page.contains("<ofd:Fonts>"));
        let public = std::str::from_utf8(prepared.entry(public_path).unwrap().unwrap()).unwrap();
        assert!(public.contains(lookalike));
        assert!(public.contains("<ofd:ColorSpaces>"));
        assert_eq!(prepared.page_svg(0).unwrap(), before);
    }

    #[test]
    fn archival_page_content_ignores_extension_graphic_lookalikes() {
        let mut document = archival_document();
        let path = "Doc_0/Pages/Page_0/Content.xml";
        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let extension = "<x:PageBlock xmlns:x=\"urn:example:extension\"><x:PageBlock><x:PageBlock><x:PageBlock><x:ClipArea ID=\"99\" Boundary=\"0 0 0 0\"/><x:TextObject CTM=\"2 0 0 1 3 4\" Clip=\"99\"><x:TextCode>X</x:TextCode></x:TextObject><x:PathObject Stroke=\"false\" LineWidth=\"2\"/><x:ImageObject Interpolate=\"true\"/></x:PageBlock></x:PageBlock></x:PageBlock></x:PageBlock>";
        assert!(source.contains("</ofd:Layer>"));
        document
            .set_entry(
                path,
                source
                    .replace("</ofd:Layer>", &format!("{extension}</ofd:Layer>"))
                    .into_bytes(),
            )
            .unwrap();
        let issues = document.archive_violations().unwrap();
        for rule in [
            "CLIP_AREA",
            "TEXT_SIZE",
            "TEXT_HSCALE",
            "IMAGE_INTERPOLATE",
            "PAGEBLOCK_DEPTH",
            "REDUNDANT_PAINT",
        ] {
            assert!(issues.iter().all(|issue| issue.rule != rule), "{rule}");
        }
        let prepared = document.prepare_archive().unwrap();
        let result = std::str::from_utf8(prepared.entry(path).unwrap().unwrap()).unwrap();
        assert!(result.contains(extension));
        assert_eq!(
            prepared.page_text(0).unwrap(),
            document.page_text(0).unwrap()
        );
    }

    #[test]
    fn checks_image_format_from_bytes_and_missing_file() {
        let mut document = archival_document();
        let path = "Doc_0/PublicRes.xml";
        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let updated = source.replace(
            "</ofd:Res>",
            "<ofd:MultiMedias><ofd:MultiMedia ID=\"42\" Type=\"Image\" Format=\"PNG\"><ofd:MediaFile>image.bin</ofd:MediaFile></ofd:MultiMedia></ofd:MultiMedias></ofd:Res>",
        );
        document.set_entry(path, updated.into_bytes()).unwrap();
        let issues = document.archive_violations().unwrap();
        let image_issue = issues
            .iter()
            .find(|issue| issue.rule == "IMAGE_FORMAT")
            .unwrap();
        assert_eq!(image_issue.severity, ArchiveSeverity::Error);

        document
            .set_entry("Doc_0/Res/image.bin", b"GIF89a\0\0".to_vec())
            .unwrap();
        let issues = document.archive_violations().unwrap();
        let image_issue = issues
            .iter()
            .find(|issue| issue.rule == "IMAGE_FORMAT")
            .unwrap();
        assert_eq!(image_issue.actual.as_deref(), Some("GIF"));

        document
            .set_entry("Doc_0/Res/image.bin", b"\x89PNG\r\n\x1a\n".to_vec())
            .unwrap();
        assert!(document
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "IMAGE_FORMAT"));
    }

    #[test]
    fn reports_existing_signatures() {
        let document = OfdDocument::from_bytes(include_bytes!(
            "../../../../ofdrw-sign/src/test/resources/signedFile.ofd"
        ))
        .unwrap();
        let issues = document.archive_violations().unwrap();
        assert!(issues.iter().any(|issue| issue.rule == "SIGNATURE"));
    }

    #[test]
    fn reports_missing_signatures_index_without_aborting() {
        let document = OfdDocument::from_bytes(include_bytes!(
            "../../../../ofdrw-layout/src/test/resources/fptpl.ofd"
        ))
        .unwrap();
        let issues = document.archive_violations().unwrap();
        assert!(issues
            .iter()
            .any(|issue| issue.rule == "SIGNATURE" && issue.actual.as_deref() == Some("missing")));
    }

    #[test]
    fn checks_font_embedding_and_missing_file() {
        let mut document = archival_document();
        let path = "Doc_0/PublicRes.xml";
        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let updated = source.replace("DejaVuSans.ttf", "missing.ttf");
        document.set_entry(path, updated.into_bytes()).unwrap();
        let issues = document.archive_violations().unwrap();
        assert!(issues.iter().any(|issue| {
            issue.rule == "FONT_SUBSET" && issue.path.as_deref() == Some("Doc_0/Res/missing.ttf")
        }));

        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let updated = source.replace("<ofd:FontFile>missing.ttf</ofd:FontFile>", "");
        document.set_entry(path, updated.into_bytes()).unwrap();
        let issues = document.archive_violations().unwrap();
        assert!(issues.iter().any(|issue| {
            issue.rule == "FONT_SUBSET" && issue.actual.as_deref() == Some("missing")
        }));
    }

    #[test]
    fn checks_annotation_flags_and_appearance_blocks() {
        let mut document = archival_document();
        let id = document
            .add_highlight_annotation(0, [1.0, 2.0, 30.0, 4.0], [255, 255, 0], 128)
            .unwrap();
        let issues = document.archive_violations().unwrap();
        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.rule == "ANNOTATION")
                .count(),
            3
        );
        document.set_annotation_read_only(0, &id, true).unwrap();
        document.set_annotation_no_zoom(0, &id, true).unwrap();
        document.set_annotation_no_rotate(0, &id, true).unwrap();
        assert!(document
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "ANNOTATION"));

        let path = document.annotations().unwrap()[0].source_path.clone();
        let source = std::str::from_utf8(document.entry(&path).unwrap().unwrap()).unwrap();
        let updated = source.replace("</ofd:Appearance>", "<ofd:PageBlock/></ofd:Appearance>");
        document.set_entry(&path, updated.into_bytes()).unwrap();
        let issues = document.archive_violations().unwrap();
        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.rule == "ANNOTATION")
                .count(),
            1
        );
    }

    #[test]
    fn archival_annotation_checks_ignore_extension_lookalikes() {
        let mut document = archival_document();
        let id = document
            .add_highlight_annotation(0, [1.0, 2.0, 30.0, 4.0], [255, 255, 0], 128)
            .unwrap();
        document.set_annotation_read_only(0, &id, true).unwrap();
        document.set_annotation_no_zoom(0, &id, true).unwrap();
        document.set_annotation_no_rotate(0, &id, true).unwrap();
        let path = document.annotations().unwrap()[0].source_path.clone();
        let source = std::str::from_utf8(document.entry(&path).unwrap().unwrap()).unwrap();
        let nested =
            "<x:Appearance xmlns:x=\"urn:example:extension\"><x:PageBlock/></x:Appearance>";
        let outer = "<x:Annot xmlns:x=\"urn:example:extension\"><x:Appearance><x:PageBlock/></x:Appearance></x:Annot>";
        assert!(source.contains("</ofd:Annot>"));
        assert!(source.contains("</ofd:PageAnnot>"));
        let updated = source
            .replace("</ofd:Annot>", &format!("{nested}</ofd:Annot>"))
            .replace("</ofd:PageAnnot>", &format!("{outer}</ofd:PageAnnot>"));
        document.set_entry(&path, updated.into_bytes()).unwrap();
        assert!(document
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "ANNOTATION"));

        let prepared = document.prepare_archive().unwrap();
        let result = std::str::from_utf8(prepared.entry(&path).unwrap().unwrap()).unwrap();
        assert!(result.contains(nested));
        assert!(result.contains(outer));
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "ANNOTATION"));
    }

    #[test]
    fn checks_clip_area_and_outline_actions() {
        let mut document = archival_document();
        let page = "Doc_0/Pages/Page_0/Content.xml";
        let source = std::str::from_utf8(document.entry(page).unwrap().unwrap()).unwrap();
        let updated = source.replace(
            "</ofd:Page>",
            "<ofd:ClipArea Boundary=\"0 0 0 2\"/></ofd:Page>",
        );
        document.set_entry(page, updated.into_bytes()).unwrap();
        let root = "Doc_0/Document.xml";
        let source = std::str::from_utf8(document.entry(root).unwrap().unwrap()).unwrap();
        let updated = source.replace(
            "</ofd:Document>",
            "<ofd:Outlines><ofd:OutlineElem Title=\"x\"><ofd:Actions><ofd:Action><ofd:URI URI=\"relative\"/></ofd:Action></ofd:Actions></ofd:OutlineElem></ofd:Outlines></ofd:Document>",
        );
        document.set_entry(root, updated.into_bytes()).unwrap();
        let issues = document.archive_violations().unwrap();
        for rule in ["CLIP_AREA", "OUTLINE_ACTION", "NON_GOTO_ACTION"] {
            assert!(issues.iter().any(|issue| issue.rule == rule), "{rule}");
        }
    }

    #[test]
    fn checks_image_extensions_and_resource_placement() {
        let mut document = archival_document();
        let resource = "Doc_0/PublicRes.xml";
        let source = std::str::from_utf8(document.entry(resource).unwrap().unwrap()).unwrap();
        let updated = source.replace(
            "</ofd:Res>",
            "<ofd:MultiMedias><ofd:MultiMedia ID=\"42\" Type=\"Image\"><ofd:MediaFile>image.png</ofd:MediaFile><x:Data xmlns:x=\"urn:custom\"/></ofd:MultiMedia></ofd:MultiMedias></ofd:Res>",
        );
        document.set_entry(resource, updated.into_bytes()).unwrap();
        document
            .set_entry("Doc_0/Res/image.png", b"\x89PNG\r\n\x1a\n".to_vec())
            .unwrap();
        let issues = document.archive_violations().unwrap();
        for rule in ["IMAGE_EXTENSION", "RESOURCE_PLACEMENT"] {
            assert!(issues.iter().any(|issue| issue.rule == rule), "{rule}");
        }
        assert!(issues.iter().all(|issue| issue.rule != "IMAGE_FORMAT"));
        let prepared = document.prepare_archive().unwrap();
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "IMAGE_EXTENSION" && issue.rule != "RESOURCE_PLACEMENT"));
        let public = std::str::from_utf8(prepared.entry(resource).unwrap().unwrap()).unwrap();
        assert!(!public.contains("x:Data"));
        assert!(!public.contains("<ofd:MultiMedias>"));
        let document_resource = std::str::from_utf8(
            prepared
                .entry("Doc_0/DocumentRes_Archive.xml")
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert!(document_resource.contains("<ofd:MultiMedias>"));
        assert!(prepared.page_svg(0).is_ok());
    }

    #[test]
    fn detects_shared_image_registered_in_page_resources() {
        let mut document = archival_document();
        document.add_text_page("second", 100.0, 100.0).unwrap();
        document
            .set_entry(
                "Doc_0/PageRes.xml",
                format!("<?xml version=\"1.0\"?><ofd:Res xmlns:ofd=\"{OFD_NS}\" BaseLoc=\"Res\"><ofd:MultiMedias><ofd:MultiMedia ID=\"42\" Type=\"Image\"><ofd:MediaFile>image.png</ofd:MediaFile></ofd:MultiMedia></ofd:MultiMedias></ofd:Res>").into_bytes(),
            )
            .unwrap();
        document
            .set_entry("Doc_0/Res/image.png", b"\x89PNG\r\n\x1a\n".to_vec())
            .unwrap();
        for index in 0..2 {
            let path = format!("Doc_0/Pages/Page_{index}/Content.xml");
            let source = std::str::from_utf8(document.entry(&path).unwrap().unwrap()).unwrap();
            let updated = source
                .replace(
                    "</ofd:Layer>",
                    "<ofd:ImageObject ID=\"90\" ResourceID=\"42\" Boundary=\"0 0 1 1\"/></ofd:Layer>",
                )
                .replace(
                    "</ofd:Page>",
                    "<ofd:PageRes>../../PageRes.xml</ofd:PageRes></ofd:Page>",
                );
            document.set_entry(&path, updated.into_bytes()).unwrap();
        }
        let issues = document.archive_violations().unwrap();
        assert!(issues.iter().any(|issue| {
            issue.rule == "IMAGE_RESOURCE_REG" && issue.path.as_deref() == Some("Doc_0/PageRes.xml")
        }));

        let path = "Doc_0/Pages/Page_1/Content.xml";
        let source = std::str::from_utf8(document.entry(path).unwrap().unwrap()).unwrap();
        let updated = source.replace(
            "<ofd:ImageObject ID=\"90\" ResourceID=\"42\" Boundary=\"0 0 1 1\"/>",
            "",
        );
        document.set_entry(path, updated.into_bytes()).unwrap();
        assert!(document
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "IMAGE_RESOURCE_REG"));

        document
            .promote_page_to_template(0, &[1], "Background")
            .unwrap();
        assert!(document.archive_violations().unwrap().iter().any(|issue| {
            issue.rule == "IMAGE_RESOURCE_REG" && issue.path.as_deref() == Some("Doc_0/PageRes.xml")
        }));
    }

    #[test]
    fn archival_preparation_normalizes_supported_rules_without_mutating_source() {
        let mut source = archival_document();
        let root = "OFD.xml";
        let xml = std::str::from_utf8(source.entry(root).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                root,
                xml.replace("DocType=\"OFD-A\"", "DocType=\"OFD\"")
                    .into_bytes(),
            )
            .unwrap();
        let document = "Doc_0/Document.xml";
        let xml = std::str::from_utf8(source.entry(document).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                document,
                xml.replace(
                    "</ofd:Document>",
                    "<ofd:Permissions/><ofd:Extensions/></ofd:Document>",
                )
                .into_bytes(),
            )
            .unwrap();
        let page = "Doc_0/Pages/Page_0/Content.xml";
        let xml = std::str::from_utf8(source.entry(page).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                page,
                xml.replace(
                    "</ofd:Layer>",
                    "<ofd:ImageObject Interpolate=\"true\"/></ofd:Layer>",
                )
                .into_bytes(),
            )
            .unwrap();
        source
            .add_highlight_annotation(0, [1.0, 2.0, 30.0, 4.0], [255, 255, 0], 128)
            .unwrap();

        let before = source.to_bytes().unwrap();
        let prepared = source.prepare_archive().unwrap();
        assert_eq!(source.to_bytes().unwrap(), before);
        assert!(prepared.archive_violations().unwrap().is_empty());
        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "DOC_TYPE"));
        let repeated = prepared.prepare_archive().unwrap();
        assert_eq!(repeated.to_bytes().unwrap(), prepared.to_bytes().unwrap());
    }

    #[test]
    fn archival_preparation_moves_safe_single_glyph_horizontal_scale_to_hscale() {
        let mut source = OfdDocument::new();
        source.add_text_page("A", 100.0, 100.0).unwrap();
        let page = "Doc_0/Pages/Page_0/Content.xml";
        let xml = std::str::from_utf8(source.entry(page).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                page,
                xml.replace(" Size=\"5\"", " Size=\"5\" CTM=\"2 0 0 1 3 4\"")
                    .into_bytes(),
            )
            .unwrap();
        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "TEXT_HSCALE"));
        let before = source.page_png(0, 72.0).unwrap();
        let prepared = source.prepare_archive().unwrap();
        let after = std::str::from_utf8(prepared.entry(page).unwrap().unwrap()).unwrap();
        assert!(after.contains("HScale=\"2\""));
        assert!(after.contains("CTM=\"1 0 0 1 3 4\""));
        assert!(!prepared
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "TEXT_HSCALE"));
        assert_eq!(source.page_text(0).unwrap(), prepared.page_text(0).unwrap());
        assert_eq!(before, prepared.page_png(0, 72.0).unwrap());
    }

    #[test]
    fn archival_preparation_scales_multi_glyph_positions_with_hscale() {
        let mut source = OfdDocument::new();
        source.add_text_page("AB", 100.0, 100.0).unwrap();
        let page = "Doc_0/Pages/Page_0/Content.xml";
        let xml = std::str::from_utf8(source.entry(page).unwrap().unwrap()).unwrap();
        assert!(xml.contains("DeltaX="));
        source
            .set_entry(
                page,
                xml.replace(" Size=\"5\"", " Size=\"5\" CTM=\"2 0 0 1 3 4\"")
                    .replace("<ofd:TextCode X=\"0\"", "<ofd:TextCode X=\"3\"")
                    .into_bytes(),
            )
            .unwrap();
        let before = source.page_png(0, 72.0).unwrap();
        let prepared = source.prepare_archive().unwrap();
        let after = std::str::from_utf8(prepared.entry(page).unwrap().unwrap()).unwrap();
        assert!(after.contains("CTM=\"1 0 0 1 3 4\""));
        assert!(after.contains("HScale=\"2\""));
        assert!(after.contains("<ofd:TextCode X=\"6\""));
        assert!(!prepared
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "TEXT_HSCALE"));
        assert_eq!(before, prepared.page_png(0, 72.0).unwrap());
    }

    #[test]
    fn archival_preparation_preserves_repeated_delta_x_while_scaling() {
        let mut source = OfdDocument::new();
        source.add_text_page("ABCD", 100.0, 100.0).unwrap();
        let page = "Doc_0/Pages/Page_0/Content.xml";
        let xml = std::str::from_utf8(source.entry(page).unwrap().unwrap()).unwrap();
        let start = xml.find("DeltaX=\"").unwrap();
        let end = start + 8 + xml[start + 8..].find('"').unwrap() + 1;
        let mut updated = xml.to_owned();
        updated.replace_range(start..end, "DeltaX=\"g 3 3\"");
        updated = updated.replace(" Size=\"5\"", " Size=\"5\" CTM=\"2 0 0 1 3 4\"");
        source.set_entry(page, updated.into_bytes()).unwrap();
        let before = source.page_png(0, 72.0).unwrap();
        let prepared = source.prepare_archive().unwrap();
        let after = std::str::from_utf8(prepared.entry(page).unwrap().unwrap()).unwrap();
        assert!(after.contains("DeltaX=\"g 3 6\""));
        assert_eq!(before, prepared.page_png(0, 72.0).unwrap());
    }

    #[test]
    fn archival_preparation_preserves_shaped_ligature_outlines() {
        let mut source = OfdDocument::new();
        let font_id = source
            .add_embedded_font(
                "DejaVu Serif",
                include_bytes!("../tests/fixtures/dejavu-serif.ttf"),
            )
            .unwrap();
        source.add_blank_page(50.0, 30.0).unwrap();
        source
            .add_shaped_text_object(0, "AfiB", [5.0, 5.0, 30.0, 10.0], 6.0, &font_id.to_string())
            .unwrap();
        let page = "Doc_0/Pages/Page_0/Content.xml";
        let xml = std::str::from_utf8(source.entry(page).unwrap().unwrap()).unwrap();
        assert!(xml.contains("GlyphCount=\"1\""));
        source
            .set_entry(
                page,
                xml.replace(" Size=\"6\"", " Size=\"6\" CTM=\"2 0 0 1 3 4\"")
                    .into_bytes(),
            )
            .unwrap();
        let before = source.page_png(0, 144.0).unwrap();
        let prepared = source.prepare_archive().unwrap();
        let after = std::str::from_utf8(prepared.entry(page).unwrap().unwrap()).unwrap();
        assert!(after.contains("HScale=\"2\""));
        assert!(after.contains("CTM=\"1 0 0 1 3 4\""));
        assert_eq!(source.page_text(0).unwrap(), prepared.page_text(0).unwrap());
        assert_eq!(before, prepared.page_png(0, 144.0).unwrap());
    }

    #[test]
    fn archival_preparation_preserves_rotated_text_ctm() {
        let mut source = OfdDocument::new();
        source.add_text_page("AB", 100.0, 100.0).unwrap();
        let page = "Doc_0/Pages/Page_0/Content.xml";
        let xml = std::str::from_utf8(source.entry(page).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                page,
                xml.replace(
                    " Size=\"5\"",
                    " Size=\"5\" CharDirection=\"90\" CTM=\"2 0 0 1 3 4\"",
                )
                .into_bytes(),
            )
            .unwrap();
        let prepared = source.prepare_archive().unwrap();
        let after = std::str::from_utf8(prepared.entry(page).unwrap().unwrap()).unwrap();
        assert!(after.contains("CTM=\"2 0 0 1 3 4\""));
        assert!(!after.contains("HScale="));
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "TEXT_HSCALE"));
    }

    #[test]
    fn archival_preparation_rejects_signed_and_multibody_packages() {
        let signed = OfdDocument::from_bytes(include_bytes!(
            "../../../../ofdrw-sign/src/test/resources/signedFile.ofd"
        ))
        .unwrap();
        assert!(signed.prepare_archive().is_err());

        let mut multiple = archival_document();
        multiple.append_document_body().unwrap();
        assert!(multiple.prepare_archive().is_err());
    }

    #[test]
    fn archival_preparation_preserves_extension_namespace_lookalikes() {
        let mut source = archival_document();
        let root = "OFD.xml";
        let xml = std::str::from_utf8(source.entry(root).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                root,
                xml.replace(
                    "</ofd:DocBody>",
                    "<x:Signatures xmlns:x=\"urn:example:extension\"/></ofd:DocBody>",
                )
                .into_bytes(),
            )
            .unwrap();
        let path = "Doc_0/Document.xml";
        let xml = std::str::from_utf8(source.entry(path).unwrap().unwrap()).unwrap();
        let lookalikes = "<x:Permissions xmlns:x=\"urn:example:extension\"/><x:VPreferences xmlns:x=\"urn:example:extension\"/><x:Extensions xmlns:x=\"urn:example:extension\"/>";
        source
            .set_entry(
                path,
                xml.replace(
                    "</ofd:Document>",
                    &format!(
                        "{lookalikes}<ofd:Permissions/><ofd:VPreferences/><ofd:Extensions/></ofd:Document>"
                    ),
                )
                .into_bytes(),
            )
            .unwrap();
        let issues = source.archive_violations().unwrap();
        for rule in ["PERMISSION", "V_PREFERENCES", "EXTENSION"] {
            assert!(issues.iter().any(|issue| issue.rule == rule));
        }

        let prepared = source.prepare_archive().unwrap();
        let xml = std::str::from_utf8(prepared.entry(path).unwrap().unwrap()).unwrap();
        assert!(xml.contains(lookalikes));
        assert!(!xml.contains("<ofd:Permissions/>"));
        assert!(!xml.contains("<ofd:VPreferences/>"));
        assert!(!xml.contains("<ofd:Extensions/>"));
        let issues = prepared.archive_violations().unwrap();
        for rule in ["PERMISSION", "V_PREFERENCES", "EXTENSION"] {
            assert!(issues.iter().all(|issue| issue.rule != rule));
        }
        assert_eq!(prepared.page_text(0).unwrap(), source.page_text(0).unwrap());
        let reopened = OfdDocument::from_bytes(&prepared.to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.entry(path).unwrap(), prepared.entry(path).unwrap());
    }

    #[test]
    fn archival_preparation_converts_gif_resource_to_png() {
        let mut gif = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut gif, image::ImageFormat::Gif)
            .unwrap();
        let mut source = OfdDocument::new();
        source
            .add_image_page(&gif.into_inner(), "GIF", 10.0, 10.0)
            .unwrap();
        let resource_path = "Doc_0/DocumentRes.xml";
        let xml = std::str::from_utf8(source.entry(resource_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                resource_path,
                xml.replace("Image_0.gif", "image_0.GIF").into_bytes(),
            )
            .unwrap();
        let mut prepared = source.prepare_archive().unwrap();
        let resource =
            std::str::from_utf8(prepared.entry("Doc_0/DocumentRes.xml").unwrap().unwrap()).unwrap();
        assert!(resource.contains("Format=\"PNG\""));
        assert!(resource.contains("image_0.GIF.archival.png"));
        assert!(prepared
            .entry("Doc_0/Res/image_0.GIF.archival.png")
            .unwrap()
            .unwrap()
            .starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(source
            .entry("Doc_0/Res/image_0.GIF.archival.png")
            .unwrap()
            .is_none());
        assert!(prepared.entry("Doc_0/Res/Image_0.gif").unwrap().is_none());
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "IMAGE_FORMAT"));
        assert!(prepared.page_svg(0).is_ok());
        let png = prepared
            .entry("Doc_0/Res/image_0.GIF.archival.png")
            .unwrap()
            .unwrap()
            .to_vec();
        prepared
            .append_image_page(0, &png, "PNG", [10.0, 10.0])
            .unwrap();
        assert!(prepared.page_svg(0).is_ok());
        assert!(prepared.page_svg(1).is_ok());
    }

    #[test]
    fn archival_preparation_converts_gif_with_split_media_file_path() {
        let mut gif = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut gif, image::ImageFormat::Gif)
            .unwrap();
        let mut source = OfdDocument::new();
        source
            .add_image_page(&gif.into_inner(), "GIF", 10.0, 10.0)
            .unwrap();
        let resource_path = "Doc_0/DocumentRes.xml";
        let resource = std::str::from_utf8(source.entry(resource_path).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                resource_path,
                resource
                    .replace("Image_0.gif", "Image_0<!-- split -->.gif")
                    .into_bytes(),
            )
            .unwrap();
        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "IMAGE_FORMAT"));
        let prepared = source.prepare_archive().unwrap();
        let resource =
            std::str::from_utf8(prepared.entry(resource_path).unwrap().unwrap()).unwrap();
        assert!(resource.contains("Image_0.gif.archival.png"));
        assert!(!resource.contains("split"));
        assert!(prepared.entry("Doc_0/Res/Image_0.gif").unwrap().is_none());
        assert!(prepared.page_svg(0).is_ok());
        assert!(prepared.validate_references().unwrap().is_empty());
    }

    #[test]
    fn archival_preparation_removes_non_goto_actions_and_keeps_goto() {
        let mut source = archival_document();
        let page = "Doc_0/Pages/Page_0/Content.xml";
        let xml = std::str::from_utf8(source.entry(page).unwrap().unwrap()).unwrap();
        let updated = xml.replace(
            "</ofd:Page>",
            "<ofd:Actions><ofd:Action Event=\"CLICK\"><ofd:Goto/></ofd:Action><ofd:Action Event=\"CLICK\"><ofd:URI URI=\"relative\"/></ofd:Action></ofd:Actions></ofd:Page>",
        );
        source.set_entry(page, updated.into_bytes()).unwrap();
        let root = "Doc_0/Document.xml";
        let xml = std::str::from_utf8(source.entry(root).unwrap().unwrap()).unwrap();
        let updated = xml.replace(
            "</ofd:Document>",
            "<ofd:Outlines><ofd:OutlineElem Title=\"x\"><ofd:Actions><ofd:Action><ofd:URI URI=\"relative\"/></ofd:Action></ofd:Actions></ofd:OutlineElem></ofd:Outlines></ofd:Document>",
        );
        source.set_entry(root, updated.into_bytes()).unwrap();

        let prepared = source.prepare_archive().unwrap();
        let page_xml = std::str::from_utf8(prepared.entry(page).unwrap().unwrap()).unwrap();
        assert!(page_xml.contains("<ofd:Goto/>"));
        assert!(!page_xml.contains("<ofd:URI"));
        let document_xml = std::str::from_utf8(prepared.entry(root).unwrap().unwrap()).unwrap();
        assert!(!document_xml.contains("<ofd:URI"));
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "NON_GOTO_ACTION" && issue.rule != "OUTLINE_ACTION"));
        assert!(std::str::from_utf8(source.entry(page).unwrap().unwrap())
            .unwrap()
            .contains("<ofd:URI"));
    }

    #[test]
    fn archival_preparation_preserves_custom_actions() {
        let mut source = archival_document();
        let custom_path = "Doc_0/Custom/business.xml";
        let custom =
            br#"<Business><Actions><Action><URI URI="local"/></Action></Actions></Business>"#;
        source.set_entry(custom_path, custom.to_vec()).unwrap();
        let page_path = "Doc_0/Pages/Page_0/Content.xml";
        let page = std::str::from_utf8(source.entry(page_path).unwrap().unwrap()).unwrap();
        let page = page.replace(
            "</ofd:Page>",
            "<ext:Actions xmlns:ext=\"urn:example:business\"><ext:Action><ext:URI URI=\"local\"/></ext:Action></ext:Actions></ofd:Page>",
        );
        source.set_entry(page_path, page.into_bytes()).unwrap();
        let document_path = "Doc_0/Document.xml";
        let document = std::str::from_utf8(source.entry(document_path).unwrap().unwrap()).unwrap();
        let document = document.replace(
            "</ofd:Document>",
            "<ofd:Outlines><ofd:OutlineElem Title=\"business\"><ext:Actions xmlns:ext=\"urn:example:business\"><ext:Action><ext:URI URI=\"local\"/></ext:Action></ext:Actions></ofd:OutlineElem></ofd:Outlines></ofd:Document>",
        );
        source
            .set_entry(document_path, document.into_bytes())
            .unwrap();

        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "NON_GOTO_ACTION" && issue.rule != "OUTLINE_ACTION"));
        let prepared = source.prepare_archive().unwrap();
        assert_eq!(
            prepared.entry(custom_path).unwrap(),
            Some(custom.as_slice())
        );
        let prepared_page =
            std::str::from_utf8(prepared.entry(page_path).unwrap().unwrap()).unwrap();
        assert!(prepared_page.contains("<ext:Action><ext:URI URI=\"local\"/></ext:Action>"));
        let prepared_document =
            std::str::from_utf8(prepared.entry(document_path).unwrap().unwrap()).unwrap();
        assert!(prepared_document.contains("<ext:Action><ext:URI URI=\"local\"/></ext:Action>"));
    }

    #[test]
    fn archival_preparation_handles_legacy_ofd_namespace_actions() {
        let mut source = archival_document();
        let page_path = "Doc_0/Pages/Page_0/Content.xml";
        let page = std::str::from_utf8(source.entry(page_path).unwrap().unwrap()).unwrap();
        let page = page.replace(OFD_NS, OFD_NS_LEGACY).replace(
            "</ofd:Page>",
            "<ofd:Actions><ofd:Action Event=\"PO\"><ofd:URI URI=\"relative\"/></ofd:Action></ofd:Actions></ofd:Page>",
        );
        source.set_entry(page_path, page.into_bytes()).unwrap();

        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "NON_GOTO_ACTION"));
        let prepared = source.prepare_archive().unwrap();
        let page = std::str::from_utf8(prepared.entry(page_path).unwrap().unwrap()).unwrap();
        assert!(!page.contains("<ofd:URI"));
        assert!(!prepared
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "NON_GOTO_ACTION"));
    }

    #[test]
    fn archival_preparation_removes_non_goto_action_from_uppercase_xml_page() {
        let mut source = archival_document();
        let old_path = "Doc_0/Pages/Page_0/Content.xml";
        let new_path = "Doc_0/Pages/Page_0/CONTENT.XML";
        let old_page = std::str::from_utf8(source.entry(old_path).unwrap().unwrap()).unwrap();
        let page = old_page.replace(
            "</ofd:Page>",
            "<ofd:Actions><ofd:Action Event=\"PO\"><ofd:URI URI=\"relative\"/></ofd:Action></ofd:Actions></ofd:Page>",
        );
        let doc_path = "Doc_0/Document.xml";
        let document = std::str::from_utf8(source.entry(doc_path).unwrap().unwrap()).unwrap();
        let document = document.replace("Pages/Page_0/Content.xml", "Pages/Page_0/CONTENT.XML");
        source.set_entry(doc_path, document.into_bytes()).unwrap();
        source.set_entry(new_path, page.into_bytes()).unwrap();
        source.remove_entry(old_path).unwrap();

        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .any(
                |issue| issue.rule == "NON_GOTO_ACTION" && issue.path.as_deref() == Some(new_path)
            ));
        let prepared = source.prepare_archive().unwrap();
        let page = std::str::from_utf8(prepared.entry(new_path).unwrap().unwrap()).unwrap();
        assert!(!page.contains("<ofd:URI"));
        assert!(!prepared
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "NON_GOTO_ACTION"));
        assert_eq!(prepared.page_text(0).unwrap(), vec!["archived"]);
    }

    #[test]
    fn archival_preparation_flattens_plain_deep_page_blocks() {
        let mut source = archival_document();
        let path = "Doc_0/Pages/Page_0/Content.xml";
        let mut page = std::str::from_utf8(source.entry(path).unwrap().unwrap())
            .unwrap()
            .to_owned();
        let layer = page.find("<ofd:Layer").unwrap();
        let opening_end = layer + page[layer..].find('>').unwrap() + 1;
        page.insert_str(opening_end, &"<ofd:PageBlock>".repeat(4));
        page = page.replace(
            "</ofd:Layer>",
            &format!("{}</ofd:Layer>", "</ofd:PageBlock>".repeat(4)),
        );
        source.set_entry(path, page.as_bytes().to_vec()).unwrap();
        let before = source.page_svg(0).unwrap();
        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "PAGEBLOCK_DEPTH"));
        let prepared = source.prepare_archive().unwrap();
        assert_eq!(prepared.page_text(0).unwrap(), source.page_text(0).unwrap());
        assert_eq!(prepared.page_svg(0).unwrap(), before);
        assert_eq!(
            std::str::from_utf8(prepared.entry(path).unwrap().unwrap())
                .unwrap()
                .matches("<ofd:PageBlock>")
                .count(),
            3
        );
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "PAGEBLOCK_DEPTH"));
        assert!(prepared.validate_references().unwrap().is_empty());
        assert_eq!(source.entry(path).unwrap().unwrap(), page.as_bytes());
    }

    #[test]
    fn archival_preparation_preserves_deep_page_blocks_with_identity_or_namespace() {
        for fourth in [
            "<ofd:PageBlock ID=\"77\">",
            "<ofd:PageBlock xmlns:ext=\"urn:example:extension\">",
        ] {
            let mut source = archival_document();
            let path = "Doc_0/Pages/Page_0/Content.xml";
            let mut page = std::str::from_utf8(source.entry(path).unwrap().unwrap())
                .unwrap()
                .to_owned();
            let layer = page.find("<ofd:Layer").unwrap();
            let opening_end = layer + page[layer..].find('>').unwrap() + 1;
            page.insert_str(
                opening_end,
                &format!("{}{}", "<ofd:PageBlock>".repeat(3), fourth),
            );
            page = page.replace(
                "</ofd:Layer>",
                &format!("{}</ofd:Layer>", "</ofd:PageBlock>".repeat(4)),
            );
            source.set_entry(path, page.as_bytes().to_vec()).unwrap();
            let prepared = source.prepare_archive().unwrap();
            let prepared_page =
                std::str::from_utf8(prepared.entry(path).unwrap().unwrap()).unwrap();
            assert!(prepared_page.contains(fourth));
            assert!(prepared
                .archive_violations()
                .unwrap()
                .iter()
                .any(|issue| issue.rule == "PAGEBLOCK_DEPTH"));
        }
    }

    #[test]
    fn archival_preparation_flattens_plain_annotation_appearance_blocks() {
        let mut source = archival_document();
        source
            .add_highlight_annotation(0, [1.0, 2.0, 30.0, 4.0], [255, 255, 0], 128)
            .unwrap();
        let path = source.annotations().unwrap()[0].source_path.clone();
        let mut annotation = std::str::from_utf8(source.entry(&path).unwrap().unwrap())
            .unwrap()
            .to_owned();
        let appearance = annotation.find("<ofd:Appearance").unwrap();
        let opening_end = appearance + annotation[appearance..].find('>').unwrap() + 1;
        annotation.insert_str(opening_end, "<ofd:PageBlock><ofd:PageBlock>");
        annotation = annotation.replace(
            "</ofd:Appearance>",
            "</ofd:PageBlock></ofd:PageBlock></ofd:Appearance>",
        );
        source
            .set_entry(&path, annotation.as_bytes().to_vec())
            .unwrap();
        let before = source.page_svg(0).unwrap();
        assert!(source.archive_violations().unwrap().iter().any(|issue| {
            issue.rule == "ANNOTATION" && issue.actual.as_deref() == Some("PageBlock")
        }));
        let prepared = source.prepare_archive().unwrap();
        let prepared_annotation =
            std::str::from_utf8(prepared.entry(&path).unwrap().unwrap()).unwrap();
        assert!(!prepared_annotation.contains("PageBlock"));
        assert_eq!(prepared.page_svg(0).unwrap(), before);
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "ANNOTATION"));
        assert!(prepared.validate_references().unwrap().is_empty());
        assert_eq!(source.entry(&path).unwrap().unwrap(), annotation.as_bytes());

        let decorated = annotation.replacen("<ofd:PageBlock>", "<ofd:PageBlock ID=\"77\">", 1);
        source.set_entry(&path, decorated.into_bytes()).unwrap();
        let prepared = source.prepare_archive().unwrap();
        let prepared_annotation =
            std::str::from_utf8(prepared.entry(&path).unwrap().unwrap()).unwrap();
        assert!(prepared_annotation.contains("<ofd:PageBlock ID=\"77\">"));
        assert!(prepared.archive_violations().unwrap().iter().any(|issue| {
            issue.rule == "ANNOTATION" && issue.actual.as_deref() == Some("PageBlock")
        }));
    }

    #[test]
    fn archival_preparation_hides_objects_with_empty_clips() {
        let mut source = archival_document();
        let page = "Doc_0/Pages/Page_0/Content.xml";
        let xml = std::str::from_utf8(source.entry(page).unwrap().unwrap()).unwrap();
        let updated = xml
            .replacen("<ofd:TextObject ", "<ofd:TextObject Clip=\"71\" ", 1)
            .replace(
                "</ofd:Page>",
                "<ofd:ClipArea ID=\"71\" Boundary=\"0 0 0 4\"/></ofd:Page>",
            );
        source.set_entry(page, updated.into_bytes()).unwrap();
        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "CLIP_AREA"));
        let prepared = source.prepare_archive().unwrap();
        let xml = std::str::from_utf8(prepared.entry(page).unwrap().unwrap()).unwrap();
        assert!(!xml.contains("ClipArea"));
        assert!(!xml.contains("Clip=\"71\""));
        assert!(xml.contains("Visible=\"false\""));
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "CLIP_AREA"));
    }

    #[test]
    fn archival_preparation_moves_public_image_resource_without_changing_page() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let mut source = OfdDocument::new();
        source
            .add_image_page(&png.into_inner(), "PNG", 10.0, 10.0)
            .unwrap();
        let document_resource = "Doc_0/DocumentRes.xml";
        let document_xml = std::str::from_utf8(source.entry(document_resource).unwrap().unwrap())
            .unwrap()
            .to_owned();
        let parsed = parse_xml(document_resource, document_xml.as_bytes()).unwrap();
        let container = descendant(parsed.root_element(), "MultiMedias").unwrap();
        let media_xml = document_xml[container.range()].to_owned();
        source
            .set_entry(
                document_resource,
                document_xml.replace(&media_xml, "").into_bytes(),
            )
            .unwrap();
        let public_resource = "Doc_0/PublicRes.xml";
        let public_xml =
            std::str::from_utf8(source.entry(public_resource).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                public_resource,
                public_xml
                    .replace("</ofd:Res>", &format!("{media_xml}</ofd:Res>"))
                    .into_bytes(),
            )
            .unwrap();
        let before = source.page_svg(0).unwrap();
        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "RESOURCE_PLACEMENT"));

        let prepared = source.prepare_archive().unwrap();
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "RESOURCE_PLACEMENT"));
        assert_eq!(prepared.page_svg(0).unwrap(), before);
        assert!(
            !std::str::from_utf8(prepared.entry(public_resource).unwrap().unwrap())
                .unwrap()
                .contains("<ofd:MultiMedias>")
        );
        assert!(
            std::str::from_utf8(prepared.entry(document_resource).unwrap().unwrap())
                .unwrap()
                .contains("<ofd:MultiMedias>")
        );
    }

    #[test]
    fn archival_preparation_creates_document_resource_with_matching_base() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let mut source = OfdDocument::new();
        source
            .add_image_page(&png.into_inner(), "PNG", 10.0, 10.0)
            .unwrap();
        let document_resource = "Doc_0/DocumentRes.xml";
        let document_xml = std::str::from_utf8(source.entry(document_resource).unwrap().unwrap())
            .unwrap()
            .to_owned();
        let parsed = parse_xml(document_resource, document_xml.as_bytes()).unwrap();
        let container = descendant(parsed.root_element(), "MultiMedias").unwrap();
        let media_xml = document_xml[container.range()].to_owned();
        source
            .set_entry(
                document_resource,
                document_xml.replace(&media_xml, "").into_bytes(),
            )
            .unwrap();
        let public_resource = "Doc_0/PublicRes.xml";
        let public_xml =
            std::str::from_utf8(source.entry(public_resource).unwrap().unwrap()).unwrap();
        source
            .set_entry(
                public_resource,
                public_xml
                    .replace("BaseLoc=\"Res\"", "BaseLoc=\"Other\"")
                    .replace("</ofd:Res>", &format!("{media_xml}</ofd:Res>"))
                    .into_bytes(),
            )
            .unwrap();
        let image = source
            .entry("Doc_0/Res/Image_0.png")
            .unwrap()
            .unwrap()
            .to_vec();
        source.set_entry("Doc_0/Other/Image_0.png", image).unwrap();
        let before = source.page_svg(0).unwrap();

        let prepared = source.prepare_archive().unwrap();
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "RESOURCE_PLACEMENT"));
        assert!(
            std::str::from_utf8(prepared.entry(public_resource).unwrap().unwrap())
                .unwrap()
                .find("<ofd:MultiMedias>")
                .is_none()
        );
        let new_path = "Doc_0/DocumentRes_Archive.xml";
        assert!(
            std::str::from_utf8(prepared.entry(new_path).unwrap().unwrap())
                .unwrap()
                .contains("<ofd:MultiMedias>")
        );
        assert!(
            std::str::from_utf8(prepared.entry("Doc_0/Document.xml").unwrap().unwrap())
                .unwrap()
                .contains("<ofd:DocumentRes>DocumentRes_Archive.xml</ofd:DocumentRes>")
        );
        assert_eq!(prepared.page_svg(0).unwrap(), before);
        assert!(prepared.validate_references().unwrap().is_empty());
    }

    #[test]
    fn archival_preparation_preserves_nested_public_resource_base() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let mut source = OfdDocument::new();
        source
            .add_image_page(&png.into_inner(), "PNG", 10.0, 10.0)
            .unwrap();
        let document_path = "Doc_0/DocumentRes.xml";
        let document_xml = std::str::from_utf8(source.entry(document_path).unwrap().unwrap())
            .unwrap()
            .to_owned();
        let parsed = parse_xml(document_path, document_xml.as_bytes()).unwrap();
        let media = descendant(parsed.root_element(), "MultiMedias").unwrap();
        let media_xml = &document_xml[media.range()];
        source
            .set_entry(
                document_path,
                document_xml.replace(media_xml, "").into_bytes(),
            )
            .unwrap();
        let public_path = "Doc_0/Resources/PublicRes.xml";
        source
            .set_entry(
                public_path,
                format!(
                    "<ofd:Res xmlns:ofd=\"{OFD_NS}\" BaseLoc=\"../Other\">{media_xml}</ofd:Res>"
                )
                .into_bytes(),
            )
            .unwrap();
        let image = source
            .entry("Doc_0/Res/Image_0.png")
            .unwrap()
            .unwrap()
            .to_vec();
        source.set_entry("Doc_0/Other/Image_0.png", image).unwrap();
        let root_path = "Doc_0/Document.xml";
        let root_source = std::str::from_utf8(source.entry(root_path).unwrap().unwrap())
            .unwrap()
            .to_owned();
        let root_xml = parse_xml(root_path, root_source.as_bytes()).unwrap();
        let public_link = descendant(root_xml.root_element(), "PublicRes").unwrap();
        source
            .set_entry(
                root_path,
                root_source
                    .replace(
                        &root_source[public_link.range()],
                        "<ofd:PublicRes>Resources/PublicRes.xml</ofd:PublicRes>",
                    )
                    .into_bytes(),
            )
            .unwrap();
        let before = source.page_svg(0).unwrap();

        let prepared = source.prepare_archive().unwrap();
        let new_path = "Doc_0/Resources/DocumentRes_Archive.xml";
        assert!(
            std::str::from_utf8(prepared.entry(new_path).unwrap().unwrap())
                .unwrap()
                .contains("BaseLoc=\"../Other\"")
        );
        assert!(
            std::str::from_utf8(prepared.entry(root_path).unwrap().unwrap())
                .unwrap()
                .contains("<ofd:DocumentRes>Resources/DocumentRes_Archive.xml</ofd:DocumentRes>")
        );
        assert_eq!(prepared.page_svg(0).unwrap(), before);
        assert!(prepared.validate_references().unwrap().is_empty());
    }

    #[test]
    fn archival_preparation_replaces_binary_attachment_with_xml_summary() {
        let mut source = archival_document();
        source
            .add_attachment("report & more.pdf", b"%PDF-example")
            .unwrap();
        assert!(source
            .archive_violations()
            .unwrap()
            .iter()
            .any(|issue| issue.rule == "ATTACHMENT"));

        let prepared = source.prepare_archive().unwrap();
        let attachment = &prepared.attachments().unwrap()[0];
        assert!(prepared
            .entry(&source.attachments().unwrap()[0].path)
            .unwrap()
            .is_none());
        assert_eq!(attachment.name, "report & more.pdf.summary.xml");
        assert_eq!(attachment.format.as_deref(), Some("XML"));
        let summary = std::str::from_utf8(prepared.attachment_data(0).unwrap()).unwrap();
        assert!(summary.contains("<OriginalName>report &amp; more.pdf</OriginalName>"));
        parse_xml(&attachment.path, summary.as_bytes()).unwrap();
        assert!(summary.contains("<OriginalFormat>unknown</OriginalFormat>"));
        assert!(summary.contains("<OriginalSize>12</OriginalSize>"));
        let digest = Sha256::digest(b"%PDF-example")
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(summary.contains(&format!("<SHA256>{digest}</SHA256>")));
        assert!(prepared
            .archive_violations()
            .unwrap()
            .iter()
            .all(|issue| issue.rule != "ATTACHMENT"));
        assert_eq!(source.attachment_data(0).unwrap(), b"%PDF-example");
    }

    #[test]
    fn archival_preparation_keeps_obsolete_payload_when_another_xml_references_it() {
        let mut source = archival_document();
        source
            .add_attachment("report.pdf", b"%PDF-example")
            .unwrap();
        let old_path = source.attachments().unwrap()[0].path.clone();
        source
            .set_entry(
                "Doc_0/Other.xml",
                format!(
                    "<Other><Path>../Res/{}</Path></Other>",
                    old_path.rsplit('/').next().unwrap()
                )
                .into_bytes(),
            )
            .unwrap();
        let prepared = source.prepare_archive().unwrap();
        assert!(prepared.entry(&old_path).unwrap().is_some());
        assert_eq!(
            prepared.attachments().unwrap()[0].format.as_deref(),
            Some("XML")
        );
    }

    #[test]
    fn archival_preparation_keeps_payload_referenced_across_text_nodes() {
        let mut source = archival_document();
        source
            .add_attachment("report.pdf", b"%PDF-example")
            .unwrap();
        let old_path = source.attachments().unwrap()[0].path.clone();
        let name = old_path.rsplit('/').next().unwrap();
        let split = name.len() / 2;
        source
            .set_entry(
                "Doc_0/Other.xml",
                format!(
                    "<Other><Path>../Res/{}<!-- split -->{}</Path></Other>",
                    &name[..split],
                    &name[split..]
                )
                .into_bytes(),
            )
            .unwrap();
        let prepared = source.prepare_archive().unwrap();
        assert!(prepared.entry(&old_path).unwrap().is_some());
        assert_eq!(
            prepared.attachments().unwrap()[0].format.as_deref(),
            Some("XML")
        );
    }

    fn audiovisual_archive_source(kind: &str, loc: &str) -> OfdDocument {
        let mut doc = archival_document();
        let path = "Doc_0/PublicRes.xml";
        let xml = std::str::from_utf8(doc.required_entry(path).unwrap()).unwrap();
        let xml = xml.replace("</ofd:Res>", &format!("<ofd:MultiMedias><ofd:MultiMedia ID=\"42\" Type=\"{kind}\"><ofd:MediaFile>{loc}</ofd:MediaFile></ofd:MultiMedia></ofd:MultiMedias></ofd:Res>"));
        doc.set_entry(path, xml.into_bytes()).unwrap();
        doc
    }

    #[test]
    fn archival_preparation_removes_audio_video_and_preserves_the_source() {
        for kind in ["Audio", "Video", "aUdIo", "vIdEo"] {
            let mut source = audiovisual_archive_source(kind, "tone.bin");
            source
                .set_entry("Doc_0/Res/tone.bin", b"original media".to_vec())
                .unwrap();
            let before = source.to_bytes().unwrap();
            let prepared = source.prepare_archive().unwrap();
            assert!(prepared
                .archive_violations()
                .unwrap()
                .iter()
                .all(|v| v.rule != "AUDIO_VIDEO"));
            assert!(prepared.entry("Doc_0/Res/tone.bin").unwrap().is_none());
            assert!(prepared
                .document_multimedia_resources(0)
                .unwrap()
                .is_empty());
            assert_eq!(source.to_bytes().unwrap(), before);
            assert_eq!(prepared.page_text(0).unwrap(), source.page_text(0).unwrap());
            assert!(prepared.validate_references().unwrap().is_empty());
        }
    }

    #[test]
    fn archival_audio_paths_join_comments_cdata_and_unique_case() {
        let mut source =
            audiovisual_archive_source("Audio", "Nested/to<!--split-->ne<![CDATA[.MP3]]>");
        source
            .set_entry("Doc_0/Res/nested/TONE.mp3", b"audio".to_vec())
            .unwrap();
        let prepared = source.prepare_archive().unwrap();
        assert!(prepared
            .entry("Doc_0/Res/nested/TONE.mp3")
            .unwrap()
            .is_none());
        assert!(prepared
            .document_multimedia_resources(0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn archival_audio_missing_external_and_absent_payloads_need_no_fetch() {
        for loc in ["missing.wav", "https://example.invalid/video.mp4", ""] {
            let source = audiovisual_archive_source("Audio", loc);
            let prepared = source.prepare_archive().unwrap();
            assert!(prepared
                .archive_violations()
                .unwrap()
                .iter()
                .all(|v| v.rule != "AUDIO_VIDEO"));
        }
    }

    #[test]
    fn archival_audio_keeps_payloads_referenced_by_other_xml() {
        let mut source = audiovisual_archive_source("Audio", "tone.mp3");
        source
            .set_entry("Doc_0/Res/tone.mp3", b"shared audio".to_vec())
            .unwrap();
        source
            .set_entry(
                "Custom.XML",
                b"<keep><file>Doc_0/Res/to<!--split-->ne<![CDATA[.mp3]]></file></keep>".to_vec(),
            )
            .unwrap();
        let prepared = source.prepare_archive().unwrap();
        assert_eq!(
            prepared.entry("Doc_0/Res/tone.mp3").unwrap(),
            Some(b"shared audio".as_slice())
        );
        assert!(prepared
            .document_multimedia_resources(0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn archival_audio_rejects_ambiguous_case_and_traversal_without_mutation() {
        let mut ambiguous = audiovisual_archive_source("Video", "tone.mp4");
        ambiguous
            .set_entry("Doc_0/Res/tone.mp4", b"first".to_vec())
            .unwrap();
        ambiguous
            .set_entry("Doc_0/Res/TONE.mp4", b"second".to_vec())
            .unwrap();
        let traversal = audiovisual_archive_source("Video", "../../../escape.mp4");
        for source in [ambiguous, traversal] {
            let before = source.to_bytes().unwrap();
            assert!(source.prepare_archive().is_err());
            assert_eq!(source.to_bytes().unwrap(), before);
        }
    }

    #[test]
    fn archival_media_preserves_foreign_namespace_and_wrapped_lookalikes() {
        let mut source = audiovisual_archive_source("Audio", "tone.mp3");
        source
            .set_entry("Doc_0/Res/tone.mp3", b"shared".to_vec())
            .unwrap();
        let path = "Doc_0/PublicRes.xml";
        let xml = std::str::from_utf8(source.required_entry(path).unwrap()).unwrap();
        let foreign = "<x:MultiMedia xmlns:x=\"urn:custom\" ID=\"43\" Type=\"Audio\"><x:MediaFile>tone.mp3</x:MediaFile></x:MultiMedia><x:Wrap xmlns:x=\"urn:custom\"><ofd:MultiMedia ID=\"44\" Type=\"Video\"><ofd:MediaFile>tone.mp3</ofd:MediaFile></ofd:MultiMedia></x:Wrap>";
        let foreign_image = "<x:Wrap xmlns:x=\"urn:custom\"><ofd:MultiMedia ID=\"45\" Type=\"Image\" Format=\"GIF\"><ofd:MediaFile>opaque.gif</ofd:MediaFile></ofd:MultiMedia></x:Wrap>";
        let xml = xml.replace("</ofd:Res>", &format!("{foreign}{foreign_image}</ofd:Res>"));
        source.set_entry(path, xml.into_bytes()).unwrap();
        source
            .set_entry(
                "Doc_0/Res/opaque.gif",
                b"GIF89a opaque extension data".to_vec(),
            )
            .unwrap();
        let prepared = source.prepare_archive().unwrap();
        let xml = std::str::from_utf8(prepared.required_entry(path).unwrap()).unwrap();
        assert!(xml.contains(foreign));
        assert!(xml.contains(foreign_image));
        assert_eq!(
            prepared.entry("Doc_0/Res/opaque.gif").unwrap(),
            Some(b"GIF89a opaque extension data".as_slice())
        );
        assert!(!xml.contains("ID=\"42\""));
        assert_eq!(
            prepared.entry("Doc_0/Res/tone.mp3").unwrap(),
            Some(b"shared".as_slice())
        );
    }

    #[test]
    fn archival_audio_pruning_keeps_payloads_when_other_xml_cannot_be_read() {
        let mut source = archival_document();
        let path = "Doc_0/Res/tone.mp3";
        source.set_entry(path, b"shared".to_vec()).unwrap();
        source
            .entries
            .insert("Unknown.xml".into(), b"<unknown".to_vec());
        prune_archive_obsolete_entries(&mut source, HashSet::from([path.to_owned()]));
        assert!(source.entry(path).unwrap().is_some());
    }

    #[test]
    fn archival_media_legacy_namespaces_remove_only_the_real_resource() {
        let mut source = audiovisual_archive_source("Video", "tone.mp4");
        source
            .set_entry("Doc_0/Res/tone.mp4", b"video".to_vec())
            .unwrap();
        let path = "Doc_0/PublicRes.xml";
        let xml = std::str::from_utf8(source.required_entry(path).unwrap())
            .unwrap()
            .replace(OFD_NS, OFD_NS_LEGACY);
        source.set_entry(path, xml.into_bytes()).unwrap();
        let prepared = source.prepare_archive().unwrap();
        assert!(prepared.entry("Doc_0/Res/tone.mp4").unwrap().is_none());
        assert!(prepared
            .document_multimedia_resources(0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn archival_audio_keeps_the_implicit_package_entry_point() {
        let source = audiovisual_archive_source("Audio", "/OFD.xml");
        let prepared = source.prepare_archive().unwrap();
        assert!(prepared.entry("OFD.xml").unwrap().is_some());
        assert_eq!(prepared.doc_roots().unwrap().len(), 1);
        assert!(prepared
            .document_multimedia_resources(0)
            .unwrap()
            .is_empty());
        assert!(prepared.validate_references().unwrap().is_empty());
    }
}
