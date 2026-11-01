use super::*;
use ofdrw::{FlowContainer, FlowContainerChild};

fn fields(value: &serde_json::Value, allowed: &[&str]) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("container or child must be an object")?;
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("unsupported container field: {key}"));
        }
    }
    Ok(())
}

pub(super) fn parse_container(
    value: &serde_json::Value,
    depth: usize,
    count: &mut usize,
) -> Result<FlowContainer, String> {
    *count += 1;
    if depth >= 8 || *count > 10_000 {
        return Err("flow container depth or node limit exceeded".into());
    }
    fields(value, &["layout", "gap", "children"])?;
    let layout =
        box_layout::parse_layout(value.get("layout").ok_or("container layout is required")?)?;
    let gap = value
        .get("gap")
        .map(|v| v.as_f64().ok_or("container gap must be a number"))
        .transpose()?
        .unwrap_or(0.);
    let mut children = Vec::new();
    if let Some(values) = value.get("children") {
        let values = values
            .as_array()
            .ok_or("container children must be an array")?;
        if values.len() > 10_000 {
            return Err("too many container children".into());
        }
        for child in values {
            *count += 1;
            if *count > 10_000 {
                return Err("too many container children".into());
            }
            let kind = child
                .get("type")
                .and_then(serde_json::Value::as_str)
                .ok_or("container child type is required")?;
            children.push(match kind {
                "paragraph" => {
                    fields(child, &["type", "spans", "style"])?;
                    let spans = parse_flow_spans(child)?;
                    let style = match child.get("style") {
                        Some(style) if style.is_object() => parse_flow_style(style)?,
                        Some(_) => return Err("paragraph style must be an object".into()),
                        None => ofdrw::FlowTextStyle::default(),
                    };
                    FlowContainerChild::Paragraph {
                        spans,
                        style: Box::new(style),
                    }
                }
                "image" => {
                    fields(
                        child,
                        &[
                            "type",
                            "data_base64",
                            "format",
                            "width",
                            "height",
                            "intrinsic_size",
                            "fit_within",
                            "align",
                        ],
                    )?;
                    FlowContainerChild::Image {
                        image: parse_table_cell_image(child)?,
                        align: parse_table_cell_align(child.get("align"))?
                            .unwrap_or(FlowTextAlign::Left),
                    }
                }
                "table" => {
                    fields(child, &["type", "table", "table_style", "align"])?;
                    let table =
                        parse_nested_table(child.get("table").ok_or("table grid is required")?, 0)?;
                    let mut style = TableStyle::default();
                    if let Some(options) = child.get("table_style") {
                        fields(
                            options,
                            &["font_id", "font_size", "line_height", "repeat_header"],
                        )?;
                        let parsed = parse_flow_style(options)?;
                        style.font_id = parsed.font_id;
                        style.font_size = parsed.font_size;
                        style.line_height = parsed.line_height;
                        if let Some(value) = options.get("repeat_header") {
                            style.repeat_header =
                                value.as_bool().ok_or("repeat_header must be a boolean")?;
                        }
                    }
                    FlowContainerChild::Table {
                        table: Box::new(table),
                        style: Box::new(style),
                        align: parse_table_cell_align(child.get("align"))?
                            .unwrap_or(FlowTextAlign::Left),
                    }
                }
                "container" => {
                    fields(child, &["type", "container"])?;
                    FlowContainerChild::Container(Box::new(parse_container(
                        child
                            .get("container")
                            .ok_or("nested container is required")?,
                        depth + 1,
                        count,
                    )?))
                }
                "line_break" => { fields(child, &["type"])?; FlowContainerChild::LineBreak }
                "page_area_fill" => { fields(child, &["type"])?; FlowContainerChild::PageAreaFill }
                "spacer" => {
                    fields(child, &["type", "height"])?;
                    FlowContainerChild::Spacer {
                        height: child
                            .get("height")
                            .map(|v| v.as_f64().ok_or("spacer height must be a number"))
                            .transpose()?
                            .unwrap_or(0.),
                    }
                }
                _ => {
                    return Err(
                        "container child type must be paragraph, image, table, container, spacer, line_break or page_area_fill"
                            .into(),
                    )
                }
            });
        }
    }
    Ok(FlowContainer {
        layout,
        children,
        gap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typography(value: &mut serde_json::Value, font: &str, shape: bool) {
        if let Some(object) = value.as_object_mut() {
            let kind = object
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned();
            if let Some(style) = object
                .get_mut(if kind == "table" {
                    "table_style"
                } else {
                    "style"
                })
                .and_then(serde_json::Value::as_object_mut)
            {
                style.insert("font_id".into(), serde_json::json!(font));
                if kind == "paragraph" {
                    style.insert("shape".into(), serde_json::json!(shape));
                }
            }
            for value in object.values_mut() {
                typography(value, font, shape);
            }
        } else if let Some(array) = value.as_array_mut() {
            for value in array {
                typography(value, font, shape);
            }
        }
    }

    #[test]
    fn ffi_mixed_containers_match_reference_and_reject_invalid_children_atomically() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../ofdrw/tests/fixtures/flow-containers.json"
        ))
        .unwrap();
        for (shape, dynamic, repeat) in [
            (false, false, false),
            (true, false, false),
            (false, true, false),
            (true, true, false),
            (false, false, true),
        ] {
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
            typography(&mut request, &font.to_string(), shape);
            if dynamic {
                request["containers"][0]["children"][1]["container"]["layout"]["clear"] =
                    serde_json::json!("none");
            }
            if repeat {
                request["containers"][0]["children"][3]["table_style"]["repeat_header"] =
                    serde_json::json!(true);
            }
            let expected = &fixture[if repeat {
                "header_expected"
            } else {
                "expected"
            }];
            let text = &fixture[if repeat { "header_text" } else { "text" }];
            let input = CString::new(request.to_string()).unwrap();
            let before = unsafe { document_ref(handle).unwrap().to_bytes().unwrap() };
            assert_ne!(
                unsafe { ofd_add_flow_containers_json(handle, 0, input.as_ptr(), ptr::null_mut()) },
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
                unsafe { ofd_add_flow_containers_json(handle, 0, input.as_ptr(), &mut output) },
                0
            );
            let result: serde_json::Value = serde_json::from_slice(unsafe {
                std::slice::from_raw_parts(output.data, output.len)
            })
            .unwrap();
            assert_eq!(&result, expected);
            unsafe { ofd_buffer_free(output) };
            for page in 0..expected["pages"].as_array().unwrap().len() {
                let actual = unsafe { document_ref(handle).unwrap().page_text(page).unwrap() };
                assert_eq!(serde_json::json!(actual), text[page]);
            }
            assert_eq!(
                unsafe { document_ref(handle).unwrap().page_images(1).unwrap()[0].boundary },
                Some([10.75, 2., 4., 4.])
            );
            let before = unsafe { document_ref(handle).unwrap().to_bytes().unwrap() };
            for child in [
                serde_json::json!({}),
                serde_json::json!({"type":"unknown"}),
                serde_json::json!({"type":"paragraph","spans":[]}),
                serde_json::json!({"type":"container","container":{"layout":{"size":[4,0]},"children":false}}),
                serde_json::json!({"type":"image","data_base64":"bad","format":"PNG","width":4,"height":4}),
                serde_json::json!({"type":"spacer","height":-1}),
                serde_json::json!({"type":"spacer","height":1,"extra":true}),
                serde_json::json!({"type":"table","table":{"row_count":1}}),
            ] {
                let mut bad = request.clone();
                bad["containers"][0]["children"]
                    .as_array_mut()
                    .unwrap()
                    .push(child);
                let input = CString::new(bad.to_string()).unwrap();
                let mut output = OfdBuffer {
                    data: ptr::null_mut(),
                    len: 0,
                };
                assert_ne!(
                    unsafe { ofd_add_flow_containers_json(handle, 0, input.as_ptr(), &mut output) },
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
                unsafe { ofd_add_flow_containers_json(handle, 0, input.as_ptr(), &mut output) },
                0
            );
            assert_ne!(
                unsafe {
                    ofd_add_flow_containers_json(ptr::null_mut(), 0, input.as_ptr(), &mut output)
                },
                0
            );
        }
    }

    #[test]
    fn ffi_controls_preserve_page_fragments_and_empty_pages() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../ofdrw/tests/fixtures/container-controls.json"
        ))
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
            typography(&mut request, &font.to_string(), shape);
            let input = CString::new(request.to_string()).unwrap();
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(
                unsafe { ofd_add_flow_containers_json(handle, 0, input.as_ptr(), &mut output) },
                0
            );
            let result: serde_json::Value = serde_json::from_slice(unsafe {
                std::slice::from_raw_parts(output.data, output.len)
            })
            .unwrap();
            assert_eq!(result, fixture["expected"]);
            unsafe { ofd_buffer_free(output) };
            let bytes = unsafe { document_ref(handle).unwrap().to_bytes().unwrap() };
            for (page, expected) in fixture["text"].as_array().unwrap().iter().enumerate() {
                let actual = unsafe { document_ref(handle).unwrap().page_text(page).unwrap() };
                assert_eq!(serde_json::json!(actual), *expected);
            }
            for kind in ["line_break", "page_area_fill"] {
                let mut invalid = request.clone();
                invalid["containers"][0]["children"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"type":kind,"height":1}));
                let input = CString::new(invalid.to_string()).unwrap();
                let mut output = OfdBuffer {
                    data: ptr::null_mut(),
                    len: 0,
                };
                assert_ne!(
                    unsafe { ofd_add_flow_containers_json(handle, 0, input.as_ptr(), &mut output) },
                    0
                );
                assert!(output.data.is_null());
                assert_eq!(
                    unsafe { document_ref(handle).unwrap().to_bytes().unwrap() },
                    bytes
                );
            }
            unsafe { ofd_close(handle) };
        }
    }

    #[test]
    fn ffi_layered_containers_route_nested_content_and_restore_default_writes() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../ofdrw/tests/fixtures/flow-containers.json"
        ))
        .unwrap();
        for (shape, dynamic) in [(false, false), (true, false), (false, true), (true, true)] {
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
            typography(&mut request, &font.to_string(), shape);
            request["containers"][0]["layout"]["layer"] = serde_json::json!("foreground");
            let nested = &mut request["containers"][0]["children"][1]["container"]["layout"];
            nested["layer"] = serde_json::json!("background");
            if dynamic {
                nested["clear"] = serde_json::json!("none");
            }
            let input = CString::new(request.to_string()).unwrap();
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(
                unsafe { ofd_add_flow_containers_json(handle, 0, input.as_ptr(), &mut output) },
                0
            );
            let result: serde_json::Value = serde_json::from_slice(unsafe {
                std::slice::from_raw_parts(output.data, output.len)
            })
            .unwrap();
            assert_eq!(result, fixture["expected"]);
            unsafe { ofd_buffer_free(output) };
            let document = unsafe { document_ref(handle).unwrap() };
            for page in 0..3 {
                let objects = document.page_objects(page).unwrap();
                assert!(objects.iter().all(|object| object.layer_type != "Body"));
                for text in document.page_text_objects(page).unwrap() {
                    let value = text
                        .codes
                        .iter()
                        .map(|code| code.text.as_str())
                        .collect::<String>();
                    let object = objects.iter().find(|object| object.id == text.id).unwrap();
                    assert_eq!(
                        object.layer_type,
                        if value == "B" || value == "C" {
                            "Background"
                        } else {
                            "Foreground"
                        }
                    );
                }
            }
            let before = document.to_bytes().unwrap();
            for layer in [
                serde_json::json!("overlay"),
                serde_json::json!(false),
                serde_json::json!(1),
                serde_json::json!({}),
            ] {
                let mut invalid = request.clone();
                invalid["containers"][0]["layout"]["layer"] = layer;
                let input = CString::new(invalid.to_string()).unwrap();
                let mut output = OfdBuffer {
                    data: ptr::null_mut(),
                    len: 0,
                };
                assert_ne!(
                    unsafe { ofd_add_flow_containers_json(handle, 0, input.as_ptr(), &mut output) },
                    0
                );
                assert!(output.data.is_null());
                assert_eq!(
                    unsafe { document_ref(handle).unwrap().to_bytes().unwrap() },
                    before
                );
            }
            let document = unsafe { document_mut(handle).unwrap() };
            document
                .add_text_object(0, "Body", [2., 2., 6., 3.], 2., None)
                .unwrap();
            assert!(document
                .page_objects(0)
                .unwrap()
                .iter()
                .any(|object| object.layer_type == "Body"));
            unsafe { ofd_close(handle) };
        }
    }

    #[test]
    fn container_parser_bounds_depth_and_fields_before_document_mutation() {
        for bad in [
            serde_json::json!({}),
            serde_json::json!({"layout":{"size":[4,0]},"children":null}),
            serde_json::json!({"layout":{"size":[4,0]},"gap":"1"}),
            serde_json::json!({"layout":{"size":[4,0],"extra":true}}),
            serde_json::json!({"layout":{"size":[4,0]},"extra":true}),
        ] {
            assert!(parse_container(&bad, 0, &mut 0).is_err());
        }
        let zero = serde_json::json!({"layout":{"size":[4,0]},"children":[{"type":"spacer"}]});
        assert!(parse_container(&zero, 0, &mut 0).is_ok());
        assert!(parse_container(&zero, 8, &mut 0).is_err());
        assert!(parse_container(&zero, 0, &mut 10_000).is_err());
    }
}
