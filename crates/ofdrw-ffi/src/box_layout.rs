use super::*;
use ofdrw::{BoxClear, BoxFloat, BoxLayoutItem, BoxLayoutResult, BoxPlacement, LayoutBox};
use serde::Serialize;

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FloatRequest {
    #[default]
    Left,
    Right,
    Center,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ClearRequest {
    None,
    Left,
    Right,
    #[default]
    Both,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoxRequest {
    size: [f64; 2],
    #[serde(default)]
    margin: [f64; 4],
    #[serde(default)]
    padding: [f64; 4],
    #[serde(default)]
    border: [f64; 4],
    background_color: Option<[u8; 3]>,
    #[serde(default)]
    border_color: [u8; 3],
    border_dash: Option<Vec<f64>>,
    alpha: Option<u8>,
    #[serde(default, rename = "float")]
    float_direction: FloatRequest,
    #[serde(default)]
    clear: ClearRequest,
    #[serde(default)]
    integrity: bool,
    #[serde(default)]
    offset: [f64; 2],
    #[serde(default)]
    placeholder: bool,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ItemRequest {
    Box(BoxRequest),
    LineBreak {},
    PageBreak {},
}

pub(super) fn parse_item(value: &serde_json::Value) -> Result<BoxLayoutItem, String> {
    let request: ItemRequest = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    Ok(match request {
        ItemRequest::LineBreak {} => BoxLayoutItem::LineBreak,
        ItemRequest::PageBreak {} => BoxLayoutItem::PageBreak,
        ItemRequest::Box(b) => BoxLayoutItem::Box(LayoutBox {
            size: b.size,
            margin: b.margin,
            padding: b.padding,
            border: b.border,
            background_color: b.background_color,
            border_color: b.border_color,
            border_dash: b.border_dash,
            alpha: b.alpha,
            float: match b.float_direction {
                FloatRequest::Left => BoxFloat::Left,
                FloatRequest::Right => BoxFloat::Right,
                FloatRequest::Center => BoxFloat::Center,
            },
            clear: match b.clear {
                ClearRequest::None => BoxClear::None,
                ClearRequest::Left => BoxClear::Left,
                ClearRequest::Right => BoxClear::Right,
                ClearRequest::Both => BoxClear::Both,
            },
            integrity: b.integrity,
            offset: b.offset,
            placeholder: b.placeholder,
        }),
    })
}

pub(super) fn parse_layout(value: &serde_json::Value) -> Result<LayoutBox, String> {
    let mut layout = value.as_object().ok_or("layout must be an object")?.clone();
    if layout.contains_key("type") {
        return Err("layout has no type field".into());
    }
    layout.insert("type".into(), serde_json::json!("box"));
    let BoxLayoutItem::Box(layout) = parse_item(&serde_json::Value::Object(layout))? else {
        return Err("layout must describe a box".into());
    };
    Ok(layout)
}

#[derive(Serialize)]
pub(super) struct StartResponse {
    page_index: usize,
    y: f64,
}

#[derive(Serialize)]
pub(super) struct PlacementResponse<'a> {
    source_index: usize,
    page_index: usize,
    allocation: &'a [f64; 4],
    border_box: &'a [f64; 4],
    padding_box: &'a [f64; 4],
    content_box: &'a [f64; 4],
    borders: &'a [f64; 4],
    source_offset: f64,
    first: bool,
    last: bool,
}

impl<'a> From<&'a BoxPlacement> for PlacementResponse<'a> {
    fn from(p: &'a BoxPlacement) -> Self {
        Self {
            source_index: p.source_index,
            page_index: p.page_index,
            allocation: &p.allocation,
            border_box: &p.border_box,
            padding_box: &p.padding_box,
            content_box: &p.content_box,
            borders: &p.borders,
            source_offset: p.source_offset,
            first: p.first,
            last: p.last,
        }
    }
}

#[derive(Serialize)]
pub(super) struct Response<'a> {
    pages: &'a [usize],
    placements: Vec<PlacementResponse<'a>>,
    next: Option<StartResponse>,
}

impl<'a> From<&'a BoxLayoutResult> for Response<'a> {
    fn from(result: &'a BoxLayoutResult) -> Self {
        Self {
            pages: &result.pages,
            placements: result
                .placements
                .iter()
                .map(PlacementResponse::from)
                .collect(),
            next: result.next.map(|p| StartResponse {
                page_index: p.page_index,
                y: p.y,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_request_preserves_defaults_and_all_drawing_and_row_options() {
        let BoxLayoutItem::Box(b) =
            parse_item(&serde_json::json!({"type":"box","size":[4,3]})).unwrap()
        else {
            panic!("expected box")
        };
        assert_eq!(
            b,
            LayoutBox {
                size: [4., 3.],
                ..Default::default()
            }
        );
        for (float, direction) in [
            ("left", BoxFloat::Left),
            ("right", BoxFloat::Right),
            ("center", BoxFloat::Center),
        ] {
            for (clear, rule) in [
                ("none", BoxClear::None),
                ("left", BoxClear::Left),
                ("right", BoxClear::Right),
                ("both", BoxClear::Both),
            ] {
                let value = serde_json::json!({"type":"box","size":[4,3],"margin":[1,2,3,4],"padding":[0.1,0.2,0.3,0.4],
                    "border":[0.5,0.6,0.7,0.8],"background_color":[1,2,3],"border_color":[4,5,6],"alpha":128,
                    "float":float,"clear":clear,"integrity":true,"offset":[2,-1],"placeholder":true});
                let BoxLayoutItem::Box(b) = parse_item(&value).unwrap() else {
                    panic!("expected box")
                };
                assert_eq!(b.float, direction);
                assert_eq!(b.clear, rule);
                assert_eq!(b.margin, [1., 2., 3., 4.]);
                assert_eq!(b.padding, [0.1, 0.2, 0.3, 0.4]);
                assert_eq!(b.border, [0.5, 0.6, 0.7, 0.8]);
                assert_eq!(b.offset, [2., -1.]);
                assert_eq!(b.background_color, Some([1, 2, 3]));
                assert_eq!(b.border_color, [4, 5, 6]);
                assert_eq!(b.alpha, Some(128));
                assert!(b.integrity && b.placeholder);
            }
        }
        assert!(matches!(
            parse_item(&serde_json::json!({"type":"line_break"})).unwrap(),
            BoxLayoutItem::LineBreak
        ));
        assert!(matches!(
            parse_item(&serde_json::json!({"type":"page_break"})).unwrap(),
            BoxLayoutItem::PageBreak
        ));
    }

    #[test]
    fn ffi_dashed_boxes_match_java_boundaries_and_reject_invalid_patterns_atomically() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../ofdrw/tests/fixtures/box-border-dashes-java.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let mut handle = ptr::null_mut();
            assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
            let mut request = serde_json::json!({"page_size":[30,20], "margins":[2,2,2,2],
                "items":[{"type":"box", "size":[10,5], "border":case["border"], "border_dash":case["dash"]}]});
            let input = CString::new(request.to_string()).unwrap();
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(
                unsafe { ofd_add_box_layout_json(handle, 0, input.as_ptr(), &mut output) },
                0
            );
            unsafe { ofd_buffer_free(output) };
            let paths = unsafe { document_ref(handle).unwrap().page_paths(0).unwrap() };
            let expected = case["paths"].as_array().unwrap();
            assert_eq!(paths.len(), expected.len());
            for (actual, expected) in paths.iter().zip(expected) {
                assert_eq!(serde_json::json!(actual.boundary), expected["boundary"]);
            }
            let before = unsafe { document_ref(handle).unwrap().to_bytes().unwrap() };
            for invalid in [
                serde_json::json!([0]),
                serde_json::json!([-1]),
                serde_json::json!([1, 0]),
                serde_json::json!(["1"]),
                serde_json::json!({}),
                serde_json::json!(vec![1; 258]),
            ] {
                request["items"][0]["border_dash"] = invalid;
                let input = CString::new(request.to_string()).unwrap();
                let mut output = OfdBuffer {
                    data: ptr::null_mut(),
                    len: 0,
                };
                assert_ne!(
                    unsafe { ofd_add_box_layout_json(handle, 0, input.as_ptr(), &mut output) },
                    0
                );
                assert!(output.data.is_null());
                assert_eq!(
                    unsafe { document_ref(handle).unwrap().to_bytes().unwrap() },
                    before
                );
            }
            unsafe { ofd_close(handle) };
        }
    }

    #[test]
    fn box_request_rejects_ambiguous_unknown_and_malformed_fields() {
        for invalid in [
            serde_json::json!(null),
            serde_json::json!({"type":"unknown"}),
            serde_json::json!({"type":"box"}),
            serde_json::json!({"type":"box","size":[1]}),
            serde_json::json!({"type":"line_break","size":[1,1]}),
            serde_json::json!({"type":"page_break","placeholder":true}),
        ] {
            assert!(parse_item(&invalid).is_err(), "{invalid}");
        }
        for (key, value) in [
            ("margin", serde_json::json!([1, 2])),
            ("padding", serde_json::json!(null)),
            ("border", serde_json::json!([1, "2", 3, 4])),
            ("background_color", serde_json::json!([1, 2, 256])),
            ("border_color", serde_json::json!([1, 2])),
            ("alpha", serde_json::json!(256)),
            ("float", serde_json::json!("justify")),
            ("clear", serde_json::json!(false)),
            ("integrity", serde_json::json!("true")),
            ("offset", serde_json::json!([1])),
            ("placeholder", serde_json::json!(0)),
            ("typo", serde_json::json!(1)),
        ] {
            let mut invalid = serde_json::json!({"type":"box","size":[4,3]});
            invalid[key] = value;
            assert!(parse_item(&invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn ffi_box_layout_returns_all_fragment_fields_and_rejects_invalid_calls_atomically() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../ofdrw/tests/fixtures/box-layout.json"))
                .unwrap();
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        let input = CString::new(fixture["request"].to_string()).unwrap();
        let before = unsafe { document_mut(handle).unwrap().to_bytes().unwrap() };
        assert_ne!(
            unsafe { ofd_add_box_layout_json(handle, 0, input.as_ptr(), ptr::null_mut()) },
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
            unsafe { ofd_add_box_layout_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        let result: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(result, fixture["expected"]);
        unsafe { ofd_buffer_free(output) };
        let before = unsafe { document_mut(handle).unwrap().to_bytes().unwrap() };
        for bad in [
            serde_json::json!(null),
            serde_json::json!({"items":false}),
            serde_json::json!({"items":[],"font_size":2}),
            serde_json::json!({"items":[],"margins":[1,2]}),
            serde_json::json!({"items":[],"page_size":[0,0]}),
            serde_json::json!({"items":[],"start":{"page_index":999,"y":2}}),
            serde_json::json!({"items":[{"type":"box","size":[4,4],"background_color":[255,0,0]},{"type":"box","size":[-1,3]}]}),
        ] {
            let input = CString::new(bad.to_string()).unwrap();
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_ne!(
                unsafe { ofd_add_box_layout_json(handle, 0, input.as_ptr(), &mut output) },
                0
            );
            assert_eq!(
                unsafe { document_mut(handle).unwrap().to_bytes().unwrap() },
                before
            );
            assert!(output.data.is_null());
        }
        let empty = CString::new(r#"{"items":[]}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_box_layout_json(handle, 0, empty.as_ptr(), &mut output) },
            0
        );
        let result: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(
            result,
            serde_json::json!({"pages":[],"placements":[],"next":null})
        );
        unsafe {
            ofd_buffer_free(output);
            ofd_close(handle)
        };
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_add_box_layout_json(handle, 0, empty.as_ptr(), &mut output) },
            0
        );
        assert_ne!(
            unsafe { ofd_add_box_layout_json(ptr::null_mut(), 0, empty.as_ptr(), &mut output) },
            0
        );
    }
}
