use ofdrw::{OfdDocument, PathStyle, SoundActionOptions};

fn source() -> OfdDocument {
    let mut doc = OfdDocument::new();
    doc.add_text_page("av", 20., 20.).unwrap();
    doc.add_path_object(0, "M 1 1 L 10 10", [0., 0., 20., 20.], PathStyle::default())
        .unwrap();
    let resource = "Doc_0/PublicRes.xml";
    let xml = std::str::from_utf8(doc.entry(resource).unwrap().unwrap()).unwrap();
    let xml = xml.replace(
        "FamilyName=\"SimSun\"/>",
        "FamilyName=\"SimSun\"><ofd:FontFile>DejaVuSans.ttf</ofd:FontFile></ofd:Font>",
    );
    doc.set_entry(resource, xml.into_bytes()).unwrap();
    doc.set_entry(
        "Doc_0/Res/DejaVuSans.ttf",
        include_bytes!("fixtures/dejavu-sans.ttf").to_vec(),
    )
    .unwrap();
    OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap()
}
fn assert_clean(doc: &OfdDocument) {
    assert!(
        doc.archive_violations().unwrap().is_empty(),
        "{:?}",
        doc.archive_violations().unwrap()
    );
    assert!(
        doc.validate_references().unwrap().is_empty(),
        "{:?}",
        doc.validate_references().unwrap()
    );
}

#[test]
fn removes_document_and_page_av_resources_with_playback_actions() {
    let mut doc = source();
    let object = doc.page_objects(0).unwrap()[0].id.clone().unwrap();
    let audio = doc
        .add_multimedia_resource(0, "Audio", "MP3", b"audio data")
        .unwrap();
    let video = doc
        .add_page_multimedia_resource(0, "Video", "MP4", b"video data")
        .unwrap();
    doc.add_page_object_sound_action(0, &object, &audio, SoundActionOptions::default())
        .unwrap();
    doc.add_page_object_movie_action(0, &object, &video, None)
        .unwrap();
    let resources = doc.page_multimedia_resources(0).unwrap();
    assert_eq!(resources.len(), 2);
    let before = doc.to_bytes().unwrap();
    let png = doc.page_png(0, 72.).unwrap();
    let prepared = doc.prepare_archive().unwrap();
    assert_clean(&prepared);
    assert!(prepared.page_multimedia_resources(0).unwrap().is_empty());
    assert!(prepared.page_object_actions(0, &object).unwrap().is_empty());
    for resource in resources {
        assert!(prepared.entry(&resource.path).unwrap().is_none());
    }
    assert_eq!(prepared.page_png(0, 72.).unwrap(), png);
    assert_eq!(doc.to_bytes().unwrap(), before);
    let reopened = OfdDocument::from_bytes(&prepared.to_bytes().unwrap()).unwrap();
    assert_clean(&reopened);
    assert_eq!(reopened.page_png(0, 72.).unwrap(), png);
}

#[test]
fn removes_template_only_av_resources_and_actions() {
    let mut doc = source();
    let object = doc.page_objects(0).unwrap()[0].id.clone().unwrap();
    let audio = doc
        .add_page_multimedia_resource(0, "Audio", "MP3", b"template audio")
        .unwrap();
    let payload = doc.page_multimedia_resources(0).unwrap()[0].path.clone();
    doc.add_page_object_sound_action(0, &object, &audio, SoundActionOptions::default())
        .unwrap();
    doc.promote_page_to_template(0, &[], "Background").unwrap();
    let page = doc.pages().unwrap()[0].path.clone();
    let original = std::str::from_utf8(doc.entry(&page).unwrap().unwrap()).unwrap();
    let parsed = roxmltree::Document::parse(original).unwrap();
    let mut xml = original.to_owned();
    let refs = parsed
        .root_element()
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "PageRes")
        .map(|n| n.range())
        .collect::<Vec<_>>();
    for range in refs.into_iter().rev() {
        xml.replace_range(range, "");
    }
    doc.set_entry(&page, xml.into_bytes()).unwrap();
    assert!(doc.page_multimedia_resources(0).unwrap().is_empty());
    assert!(doc
        .archive_violations()
        .unwrap()
        .iter()
        .any(|v| v.rule == "AUDIO_VIDEO"));
    let png = doc.page_png(0, 72.).unwrap();
    let prepared = doc.prepare_archive().unwrap();
    assert_clean(&prepared);
    assert!(prepared.entry(&payload).unwrap().is_none());
    assert_eq!(prepared.page_png(0, 72.).unwrap(), png);
    let reopened = OfdDocument::from_bytes(&prepared.to_bytes().unwrap()).unwrap();
    assert_clean(&reopened);
}

#[test]
fn removing_audio_keeps_shared_image_payload_and_appearance() {
    let mut doc = source();
    let png = doc.page_png(0, 72.).unwrap();
    let audio = doc
        .add_multimedia_resource(0, "Audio", "PNG", &png)
        .unwrap();
    let audio_resource = doc.document_multimedia_resources(0).unwrap()[0].clone();
    doc.add_image_object(0, &png, "PNG", [0., 0., 10., 10.])
        .unwrap();
    let path = doc
        .entry_names()
        .find(|path| {
            path.ends_with(".xml") && {
                let xml = std::str::from_utf8(doc.entry(path).unwrap().unwrap()).unwrap();
                roxmltree::Document::parse(xml)
                    .unwrap()
                    .descendants()
                    .any(|n| {
                        n.tag_name().name() == "MultiMedia" && n.attribute("Type") == Some("Image")
                    })
            }
        })
        .unwrap()
        .to_owned();
    let xml = std::str::from_utf8(doc.entry(&path).unwrap().unwrap()).unwrap();
    let parsed = roxmltree::Document::parse(xml).unwrap();
    let image = parsed
        .descendants()
        .find(|n| n.attribute("Type") == Some("Image"))
        .unwrap();
    let file = image
        .children()
        .find(|n| n.has_tag_name(("http://www.ofdspec.org/2016", "MediaFile")))
        .unwrap();
    let mut updated = xml.to_owned();
    let old = file.range();
    let name = audio_resource.path.rsplit('/').next().unwrap();
    updated.replace_range(old, &format!("<ofd:MediaFile>{name}</ofd:MediaFile>"));
    doc.set_entry(&path, updated.into_bytes()).unwrap();
    let before = doc.page_png(0, 72.).unwrap();
    let prepared = doc.prepare_archive().unwrap();
    assert_clean(&prepared);
    assert_eq!(
        prepared.entry(&audio_resource.path).unwrap(),
        Some(png.as_slice())
    );
    let resources = prepared.document_multimedia_resources(0).unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].media_type.as_deref(), Some("Image"));
    assert_ne!(resources[0].id, audio);
    assert_eq!(prepared.page_png(0, 72.).unwrap(), before);
}
