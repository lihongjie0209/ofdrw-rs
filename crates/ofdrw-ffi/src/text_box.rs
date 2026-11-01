use super::*;

pub(super) fn parse_box(value: &serde_json::Value) -> Result<ofdrw::TextBox, String> {
    let object = value.as_object().ok_or("text box must be an object")?;
    for key in object.keys() {
        if !matches!(key.as_str(), "layout" | "spans" | "style") {
            return Err(format!("unsupported text box field: {key}"));
        }
    }
    let layout =
        box_layout::parse_layout(value.get("layout").ok_or("text box layout is required")?)?;
    let spans = parse_flow_spans(value)?;
    let style = match value.get("style") {
        Some(style) if style.is_object() => parse_flow_style(style)?,
        Some(_) => return Err("text box style must be an object".into()),
        None => ofdrw::FlowTextStyle::default(),
    };
    Ok(ofdrw::TextBox {
        layout,
        spans,
        style,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_text_boxes_match_reference_and_reject_bad_requests_atomically() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../ofdrw/tests/fixtures/text-boxes.json"))
                .unwrap();
        for shape in [false, true] {
            let mut handle = ptr::null_mut();
            assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
            let font = unsafe {
                document_mut(handle)
                    .unwrap()
                    .add_embedded_font(
                        "DejaVu Sans",
                        include_bytes!("../../ofdrw/tests/fixtures/dejavu-sans.ttf"),
                    )
                    .unwrap()
            };
            let mut request = fixture["request"].clone();
            request["boxes"][0]["style"]["font_id"] = serde_json::json!(font.to_string());
            request["boxes"][0]["style"]["shape"] = serde_json::json!(shape);
            let input = CString::new(request.to_string()).unwrap();
            let before = unsafe { document_ref(handle).unwrap().to_bytes().unwrap() };
            assert_ne!(
                unsafe { ofd_add_text_boxes_json(handle, 0, input.as_ptr(), ptr::null_mut()) },
                0
            );
            assert_eq!(
                unsafe { document_ref(handle).unwrap().to_bytes().unwrap() },
                before
            );
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(
                unsafe { ofd_add_text_boxes_json(handle, 0, input.as_ptr(), &mut output) },
                0
            );
            let actual: serde_json::Value = serde_json::from_slice(unsafe {
                std::slice::from_raw_parts(output.data, output.len)
            })
            .unwrap();
            assert_eq!(actual, fixture["expected"]);
            unsafe { ofd_buffer_free(output) };
            assert_eq!(
                unsafe { document_ref(handle).unwrap().page_text(1).unwrap() },
                ["D"]
            );
            let before = unsafe { document_ref(handle).unwrap().to_bytes().unwrap() };
            for bad in [
                serde_json::json!({}),
                serde_json::json!({"layout":{"size":[8,0]},"spans":[]}),
                serde_json::json!({"layout":{"size":[8,0],"type":"box"},"spans":[{"text":"A"}]}),
                serde_json::json!({"layout":{"size":[8,0]},"spans":[{"text":"A"}],"style":false}),
                serde_json::json!({"layout":{"size":[8,0]},"spans":[{"text":"A"}],"unknown":true}),
                serde_json::json!({"layout":{"size":[8,0]},"spans":[{"text":"A","font_id":"missing"}]}),
            ] {
                let mut bad_request = request.clone();
                bad_request["boxes"].as_array_mut().unwrap().push(bad);
                let input = CString::new(bad_request.to_string()).unwrap();
                let mut output = OfdBuffer {
                    data: ptr::null_mut(),
                    len: 0,
                };
                assert_ne!(
                    unsafe { ofd_add_text_boxes_json(handle, 0, input.as_ptr(), &mut output) },
                    0
                );
                assert!(output.data.is_null());
                assert_eq!(
                    unsafe { document_ref(handle).unwrap().to_bytes().unwrap() },
                    before
                );
            }
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            unsafe { ofd_close(handle) };
            assert_ne!(
                unsafe { ofd_add_text_boxes_json(handle, 0, input.as_ptr(), &mut output) },
                0
            );
            assert_ne!(
                unsafe { ofd_add_text_boxes_json(ptr::null_mut(), 0, input.as_ptr(), &mut output) },
                0
            );
        }
    }
}
