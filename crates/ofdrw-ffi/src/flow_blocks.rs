use super::*;
use ofdrw::{FlowBlock, FlowParagraphStyle};

fn paragraph_style(value: &serde_json::Value) -> Result<FlowParagraphStyle, String> {
    let object = value
        .as_object()
        .ok_or("paragraph style must be an object")?;
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "font_id"
                | "font_size"
                | "line_height"
                | "align"
                | "paragraph_gap"
                | "keep_paragraphs_together"
                | "first_line_indent"
                | "rest_line_indent"
                | "first_line_indent_chars"
                | "indent_each_paragraph"
                | "shape"
                | "language"
        ) {
            return Err(format!("unsupported paragraph style field: {key}"));
        }
    }
    let parsed = parse_flow_style(value)?;
    macro_rules! optional {
        ($field:ident) => {
            object
                .contains_key(stringify!($field))
                .then_some(parsed.$field)
        };
    }
    Ok(FlowParagraphStyle {
        font_id: parsed.font_id,
        font_size: optional!(font_size),
        line_height: optional!(line_height),
        align: optional!(align),
        paragraph_gap: optional!(paragraph_gap),
        keep_paragraphs_together: optional!(keep_paragraphs_together),
        first_line_indent: optional!(first_line_indent),
        rest_line_indent: optional!(rest_line_indent),
        first_line_indent_chars: optional!(first_line_indent_chars),
        indent_each_paragraph: optional!(indent_each_paragraph),
        shape: optional!(shape),
        language: parsed.language,
    })
}

pub(super) fn parse_block(value: &serde_json::Value) -> Result<FlowBlock, String> {
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("paragraph") => {
            let spans = match (value.get("text"), value.get("spans")) {
                (Some(text), None) => vec![FlowSpan::new(
                    text.as_str().ok_or("paragraph text must be a string")?,
                )],
                (None, Some(_)) => parse_flow_spans(value)?,
                _ => return Err("paragraph requires exactly one of text or spans".into()),
            };
            let style = value
                .get("style")
                .filter(|v| !v.is_null())
                .map(paragraph_style)
                .transpose()?
                .map(Box::new);
            Ok(FlowBlock::Paragraph { spans, style })
        }
        Some("image") => Ok(FlowBlock::Image {
            image: parse_table_cell_image(value)?,
            align: parse_table_cell_align(value.get("align"))?.unwrap_or(FlowTextAlign::Left),
        }),
        Some("table") => {
            let table =
                parse_nested_table(value.get("table").ok_or("flow table grid is required")?, 0)?;
            let repeat_header = value
                .get("repeat_header")
                .map(|v| v.as_bool().ok_or("repeat_header must be a boolean"))
                .transpose()?
                .unwrap_or(false);
            Ok(FlowBlock::Table {
                table,
                repeat_header,
            })
        }
        Some("spacer") => Ok(FlowBlock::Spacer {
            height: value
                .get("height")
                .and_then(serde_json::Value::as_f64)
                .ok_or("flow spacer height must be a number")?,
        }),
        Some("page_break") => Ok(FlowBlock::PageBreak),
        _ => Err("flow block type must be paragraph, image, table, spacer, or page_break".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intrinsic_image_sizing_uses_the_shared_parser_and_rejects_ambiguous_requests() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../ofdrw/tests/fixtures/image-layout.json"))
                .unwrap();
        let block = &fixture["request"]["blocks"][0];
        let FlowBlock::Image { image, .. } = parse_block(block).unwrap() else {
            panic!("image expected")
        };
        assert_eq!([image.width, image.height], [4., 2.]);
        let mut plain = block.clone();
        plain.as_object_mut().unwrap().remove("fit_within");
        assert_eq!(parse_table_cell_image(&plain).unwrap().width, 10.);
        plain["width"] = serde_json::json!(0);
        plain["height"] = serde_json::json!(0);
        assert_eq!(parse_table_cell_image(&plain).unwrap().height, 5.);
        plain["intrinsic_size"] = serde_json::json!(false);
        plain["width"] = serde_json::json!(10);
        plain["height"] = serde_json::json!(5);
        plain["fit_within"] = serde_json::json!([4, 4]);
        assert_eq!(parse_table_cell_image(&plain).unwrap().width, 4.);
        for (key, value) in [
            ("intrinsic_size", serde_json::json!("true")),
            ("width", serde_json::json!(2)),
            ("height", serde_json::json!(null)),
            ("fit_within", serde_json::json!([0, 4])),
            ("fit_within", serde_json::json!([4])),
            ("fit_within", serde_json::json!([4, 4, 4])),
            ("fit_within", serde_json::json!([4, null])),
            ("data_base64", serde_json::json!("invalid")),
            ("format", serde_json::json!("JPEG")),
        ] {
            let mut bad = block.clone();
            bad[key] = value;
            assert!(parse_block(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ffi_mixed_flow_returns_cursor_and_preserves_the_source_on_invalid_output_or_late_error() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../ofdrw/tests/fixtures/flow-blocks.json"))
                .unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let request = CString::new(fixture["request"].to_string()).unwrap();
        let before = unsafe { document_mut(handle).unwrap().to_bytes().unwrap() };
        assert_ne!(
            unsafe { ofd_add_flow_blocks_json(handle, 0, request.as_ptr(), ptr::null_mut()) },
            0
        );
        assert_eq!(
            unsafe { document_mut(handle).unwrap().to_bytes().unwrap() },
            before
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_flow_blocks_json(handle, 0, request.as_ptr(), &mut output) },
            0
        );
        let result: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(result["pages"], fixture["expected"]["pages"]);
        assert_eq!(
            result["next"]["page_index"],
            fixture["expected"]["next"]["page_index"]
        );
        assert_eq!(
            result["next"]["y"].as_f64(),
            fixture["expected"]["next"]["y"].as_f64()
        );
        unsafe { ofd_buffer_free(output) };
        let before = unsafe { document_mut(handle).unwrap().to_bytes().unwrap() };
        for invalid in [
            serde_json::json!({"type":"unknown"}),
            serde_json::json!({"type":"image","data_base64":"","format":"PNG","width":2,"height":2}),
            serde_json::json!({"type":"spacer","height":-1}),
            serde_json::json!({"type":"paragraph","text":"x","style":{"font_size":0}}),
        ] {
            let mut request = fixture["request"].clone();
            request["blocks"].as_array_mut().unwrap().push(invalid);
            let request = CString::new(request.to_string()).unwrap();
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_ne!(
                unsafe { ofd_add_flow_blocks_json(handle, 0, request.as_ptr(), &mut output) },
                0
            );
            assert_eq!(
                unsafe { document_mut(handle).unwrap().to_bytes().unwrap() },
                before
            );
            assert!(output.data.is_null());
        }
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn paragraph_overrides_preserve_presence_and_reject_geometry_and_wrong_types() {
        let value = serde_json::json!({"font_id":"3","font_size":4,"line_height":5,"align":"center",
            "paragraph_gap":0,"keep_paragraphs_together":false,"first_line_indent":0,
            "rest_line_indent":1,"first_line_indent_chars":2,"indent_each_paragraph":true,
            "shape":false,"language":"en"});
        let parsed = paragraph_style(&value).unwrap();
        assert_eq!(parsed.shape, Some(false));
        assert_eq!(parsed.keep_paragraphs_together, Some(false));
        assert_eq!(parsed.paragraph_gap, Some(0.));
        assert_eq!(parsed.first_line_indent_chars, Some(2));
        assert_eq!(parsed.font_id.as_deref(), Some("3"));
        assert_eq!(parsed.language.as_deref(), Some("en"));
        assert_eq!(
            paragraph_style(&serde_json::json!({})).unwrap(),
            FlowParagraphStyle::default()
        );
        for invalid in [
            serde_json::json!(null),
            serde_json::json!({"page_size":[20,20]}),
            serde_json::json!({"start":{"page_index":0,"y":2}}),
            serde_json::json!({"shape":0}),
            serde_json::json!({"font_size":"4"}),
            serde_json::json!({"align":"justify"}),
        ] {
            assert!(paragraph_style(&invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn flow_blocks_reject_ambiguous_paragraphs_and_malformed_content() {
        for invalid in [
            serde_json::json!({"type":"paragraph"}),
            serde_json::json!({"type":"paragraph","text":"x","spans":[]}),
            serde_json::json!({"type":"paragraph","text":2}),
            serde_json::json!({"type":"paragraph","spans":false}),
            serde_json::json!({"type":"paragraph","spans":[{"text":"x","font_size":"2"}]}),
            serde_json::json!({"type":"image","align":"justify"}),
            serde_json::json!({"type":"table"}),
            serde_json::json!({"type":"spacer","height":"2"}),
            serde_json::json!({"type":"rule","thickness":1}),
            serde_json::json!(null),
        ] {
            assert!(parse_block(&invalid).is_err(), "{invalid}");
        }
        assert!(matches!(
            parse_block(&serde_json::json!({"type":"page_break"})).unwrap(),
            FlowBlock::PageBreak
        ));
        assert!(matches!(
            parse_block(&serde_json::json!({"type":"spacer","height":3})).unwrap(),
            FlowBlock::Spacer { height: 3. }
        ));
        let request: serde_json::Value =
            serde_json::from_str(include_str!("../../ofdrw/tests/fixtures/flow-blocks.json"))
                .unwrap();
        let mut table = request["request"]["blocks"][2].clone();
        table["repeat_header"] = serde_json::json!("yes");
        assert!(parse_block(&table).is_err());
    }

    #[test]
    fn top_level_flow_tables_preserve_the_existing_eight_level_cell_nesting_budget() {
        let grid = || {
            serde_json::json!({"row_count":1,"column_widths":[16],
            "cells":[{"row":0,"column":0,"text":"A"}]})
        };
        let mut nested = grid();
        for _ in 0..8 {
            let mut parent = grid();
            parent["cells"][0].as_object_mut().unwrap().remove("text");
            parent["cells"][0]["blocks"] = serde_json::json!([{"type":"table","table":nested}]);
            nested = parent;
        }
        assert!(parse_block(&serde_json::json!({"type":"table","table":nested})).is_ok());
        let mut parent = grid();
        parent["cells"][0].as_object_mut().unwrap().remove("text");
        parent["cells"][0]["blocks"] = serde_json::json!([{"type":"table","table":nested}]);
        assert!(parse_block(&serde_json::json!({"type":"table","table":parent})).is_err());
    }
}
