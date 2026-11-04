use super::*;
use serde_json::{json, Value};

const SOURCE: &[u8] = include_bytes!("../../ofdrw/tests/fixtures/canvas-images-java/source.png");

fn output(call: impl FnOnce(*mut OfdBuffer) -> i32) -> Value {
    let mut buffer = OfdBuffer {
        data: ptr::null_mut(),
        len: 0,
    };
    assert_eq!(call(&mut buffer), 0);
    let value =
        serde_json::from_slice(unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) })
            .unwrap();
    unsafe { ofd_buffer_free(buffer) };
    value
}

#[test]
fn pattern_abi_matches_all_java_writer_cases_and_image_matrices() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ofdrw/tests/fixtures/canvas-pattern-java");
    let reference: Value = serde_json::from_str(include_str!(
        "../../ofdrw/tests/fixtures/canvas-pattern-java/reference.json"
    ))
    .unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let java = OfdDocument::open(root.join(format!("{}.ofd", case["name"].as_str().unwrap())))
            .unwrap();
        let mut doc = OfdDocument::new();
        doc.add_blank_page(40., 30.).unwrap();
        let page = java.pages().unwrap().remove(0);
        doc.set_page_area(
            0,
            ofdrw::PageArea {
                physical_box: page.physical_box.unwrap(),
                application_box: page.application_box,
                content_box: page.content_box,
                bleed_box: page.bleed_box,
            },
        )
        .unwrap();
        let handle = register_document(doc).unwrap();
        let options = CString::new(
            json!({"repetition":case["repetition"],"pixels_per_mm":case["ppm"],
            "image_size":case["size"],"ctm":case["transform"]})
            .to_string(),
        )
        .unwrap();
        let format = CString::new("png").unwrap();
        let pattern = output(|out| unsafe {
            ofd_create_canvas_pattern_json(
                handle,
                0,
                SOURCE.as_ptr(),
                SOURCE.len(),
                format.as_ptr(),
                options.as_ptr(),
                out,
            )
        });
        assert_eq!(pattern["cells"][0]["ctm"], case["expected"]["image_ctm"]);
        assert_eq!(
            pattern["cells"][0]["boundary"],
            case["expected"]["image_boundary"]
        );
        assert_eq!(pattern["width"], case["expected"]["width"]);
        assert_eq!(pattern["height"], case["expected"]["height"]);
        assert!(unsafe { document_ref(handle).unwrap() }
            .page_paths(0)
            .unwrap()
            .is_empty());
        let stroke = case["stroke"].as_bool().unwrap_or(false);
        let style = if stroke {
            json!({"stroke_pattern":pattern,"line_width":1.2})
        } else {
            json!({"fill_pattern":pattern})
        };
        let mut commands = vec![json!({"type":"set_draw_param","style":style})];
        if !case["canvas_ctm"].is_null() {
            commands.push(json!({"type":"set_transform","matrix":case["canvas_ctm"]}));
        }
        if !case["alpha"].is_null() {
            commands.push(json!({"type":"set_global_alpha","alpha":case["alpha"]}));
        }
        if stroke {
            commands.extend([
                json!({"type":"rect","rect":[2,2,18,12]}),
                json!({"type":"stroke"}),
            ]);
        } else {
            commands.push(json!({"type":"fill_rect","rect":[2,2,18,12]}));
        }
        let request =
            CString::new(json!({"boundary":[4,3,24,20],"commands":commands}).to_string()).unwrap();
        output(|out| unsafe { ofd_draw_canvas_json(handle, 0, request.as_ptr(), out) });
        let doc = unsafe { document_ref(handle).unwrap() };
        assert!(doc.validate_references().unwrap().is_empty());
        let reopened = OfdDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        for dpi in [25.4, 101.6] {
            let png = doc.page_png(0, dpi).unwrap();
            assert!(png == java.page_png(0, dpi).unwrap(), "{case}");
            assert!(png == reopened.page_png(0, dpi).unwrap());
        }
        unsafe { ofd_close(handle) };
    }
}

#[test]
fn pattern_abi_rejects_bad_options_and_pointers_without_resource_edits() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let before = doc.to_bytes().unwrap();
    let handle = register_document(doc).unwrap();
    let format = CString::new("png").unwrap();
    for value in [
        json!({"image_size":[0,4]}),
        json!({"image_size":[4]}),
        json!({"ctm":[1,2]}),
        json!({"repetition":"repeat-x"}),
        json!({"pixels_per_mm":0}),
        json!({"extra":true}),
    ] {
        let options = CString::new(value.to_string()).unwrap();
        let mut out = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe {
                ofd_create_canvas_pattern_json(
                    handle,
                    0,
                    SOURCE.as_ptr(),
                    SOURCE.len(),
                    format.as_ptr(),
                    options.as_ptr(),
                    &mut out,
                )
            },
            0
        );
        assert!(out.data.is_null());
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
    }
    let options = CString::new("{}").unwrap();
    assert_ne!(
        unsafe {
            ofd_create_canvas_pattern_json(
                handle,
                0,
                SOURCE.as_ptr(),
                SOURCE.len(),
                format.as_ptr(),
                options.as_ptr(),
                ptr::null_mut(),
            )
        },
        0
    );
    let mut out = OfdBuffer {
        data: ptr::null_mut(),
        len: 0,
    };
    assert_ne!(
        unsafe {
            ofd_create_canvas_pattern_json(
                handle,
                0,
                ptr::null(),
                SOURCE.len(),
                format.as_ptr(),
                options.as_ptr(),
                &mut out,
            )
        },
        0
    );
    assert_ne!(
        unsafe {
            ofd_create_canvas_pattern_json(
                handle,
                0,
                SOURCE.as_ptr(),
                SOURCE.len(),
                ptr::null(),
                options.as_ptr(),
                &mut out,
            )
        },
        0
    );
    assert_ne!(
        unsafe {
            ofd_create_canvas_pattern_json(
                handle,
                0,
                SOURCE.as_ptr(),
                SOURCE.len(),
                format.as_ptr(),
                ptr::null(),
                &mut out,
            )
        },
        0
    );
    assert_eq!(
        unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
        before
    );
    unsafe { ofd_close(handle) };
    assert_ne!(
        unsafe {
            ofd_create_canvas_pattern_json(
                handle,
                0,
                SOURCE.as_ptr(),
                SOURCE.len(),
                format.as_ptr(),
                options.as_ptr(),
                &mut out,
            )
        },
        0
    );
}

#[test]
fn image_cell_json_ctm_is_applied_and_malformed_ctm_is_rejected() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    let handle = register_document(doc).unwrap();
    let options = CString::new(r#"{"image_size":[4,3]}"#).unwrap();
    let format = CString::new("png").unwrap();
    let mut pattern = output(|out| unsafe {
        ofd_create_canvas_pattern_json(
            handle,
            0,
            SOURCE.as_ptr(),
            SOURCE.len(),
            format.as_ptr(),
            options.as_ptr(),
            out,
        )
    });
    pattern["cells"][0]["ctm"] = json!([3, 1, 0.5, 2, 0, 0]);
    let draw = |pattern: &Value| {
        CString::new(
            json!({"boundary":[0,0,40,30],"commands":[
                {"type":"set_draw_param","style":{"fill_pattern":pattern}},
                {"type":"fill_rect","rect":[2,2,18,12]}
            ]})
            .to_string(),
        )
        .unwrap()
    };
    let request = draw(&pattern);
    output(|out| unsafe { ofd_draw_canvas_json(handle, 0, request.as_ptr(), out) });
    let doc = unsafe { document_ref(handle).unwrap() };
    assert!(doc
        .entry_names()
        .filter(|name| name.ends_with(".xml"))
        .any(|name| {
            std::str::from_utf8(doc.entry(name).unwrap().unwrap())
                .unwrap()
                .contains("CTM=\"3 1 0.5 2 0 0\"")
        }));
    let before = doc.to_bytes().unwrap();
    pattern["cells"][0]["ctm"] = json!([1, 2]);
    let request = draw(&pattern);
    let mut out = OfdBuffer {
        data: ptr::null_mut(),
        len: 0,
    };
    assert_ne!(
        unsafe { ofd_draw_canvas_json(handle, 0, request.as_ptr(), &mut out) },
        0
    );
    assert_eq!(
        unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
        before
    );
    unsafe { ofd_close(handle) };
}

#[test]
fn pattern_abi_respects_document_ownership_and_signed_document_guards() {
    let mut doc = OfdDocument::new();
    doc.add_blank_page(40., 30.).unwrap();
    doc.append_document_body().unwrap();
    doc.append_blank_page(1, 40., 30.).unwrap();
    let handle = register_document(doc).unwrap();
    let options = CString::new("{}").unwrap();
    let format = CString::new("png").unwrap();
    output(|out| unsafe {
        ofd_create_canvas_pattern_json(
            handle,
            1,
            SOURCE.as_ptr(),
            SOURCE.len(),
            format.as_ptr(),
            options.as_ptr(),
            out,
        )
    });
    let doc = unsafe { document_ref(handle).unwrap() };
    assert!(doc.document_multimedia_resources(0).unwrap().is_empty());
    assert_eq!(doc.document_multimedia_resources(1).unwrap().len(), 1);
    assert!(doc.page_paths(1).unwrap().is_empty());
    assert!(doc.validate_references().unwrap().is_empty());
    unsafe { ofd_close(handle) };
    let signed = OfdDocument::open(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ofdrw-sign/src/test/resources/signedFile.ofd"),
    )
    .unwrap();
    let before = signed.to_bytes().unwrap();
    let handle = register_document(signed).unwrap();
    let mut out = OfdBuffer {
        data: ptr::null_mut(),
        len: 0,
    };
    assert_ne!(
        unsafe {
            ofd_create_canvas_pattern_json(
                handle,
                0,
                SOURCE.as_ptr(),
                SOURCE.len(),
                format.as_ptr(),
                options.as_ptr(),
                &mut out,
            )
        },
        0
    );
    let bad_utf8 = [255_u8, 0];
    assert_ne!(
        unsafe {
            ofd_create_canvas_pattern_json(
                handle,
                0,
                SOURCE.as_ptr(),
                SOURCE.len(),
                bad_utf8.as_ptr().cast(),
                options.as_ptr(),
                &mut out,
            )
        },
        0
    );
    assert_ne!(
        unsafe {
            ofd_create_canvas_pattern_json(
                handle,
                0,
                SOURCE.as_ptr(),
                SOURCE.len(),
                format.as_ptr(),
                bad_utf8.as_ptr().cast(),
                &mut out,
            )
        },
        0
    );
    assert_eq!(
        unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
        before
    );
    unsafe { ofd_close(handle) };
}
