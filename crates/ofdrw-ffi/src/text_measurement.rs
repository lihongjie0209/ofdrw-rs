use ofdrw::{FlowLineMetrics, FlowTextMetrics};
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct LineResponse<'a> {
    text: &'a str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    baseline: f64,
    paragraph_start: bool,
}

impl<'a> From<&'a FlowLineMetrics> for LineResponse<'a> {
    fn from(line: &'a FlowLineMetrics) -> Self {
        Self {
            text: &line.text,
            x: line.x,
            y: line.y,
            width: line.width,
            height: line.height,
            baseline: line.baseline,
            paragraph_start: line.paragraph_start,
        }
    }
}

#[derive(Serialize)]
pub(super) struct Response<'a> {
    width: f64,
    height: f64,
    lines: Vec<LineResponse<'a>>,
}

impl<'a> From<&'a FlowTextMetrics> for Response<'a> {
    fn from(metrics: &'a FlowTextMetrics) -> Self {
        Self {
            width: metrics.width,
            height: metrics.height,
            lines: metrics.lines.iter().map(LineResponse::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn ffi_preserves_natural_zero_advance_metrics_and_writes_the_mark() {
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
        let request = serde_json::json!({"content_width":20,"font_id":font.to_string(),
            "font_size":2,"line_height":3,"spans":[{"text":"\u{301}"}]});
        let before = unsafe { document_ref(handle).unwrap().to_bytes().unwrap() };
        let input = CString::new(request.to_string()).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_measure_flowed_spans_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        let measured: serde_json::Value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        assert_eq!(measured["width"], 0.);
        assert_eq!(measured["height"], 3.);
        assert_eq!(measured["lines"][0]["text"], "\u{301}");
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            unsafe { document_ref(handle).unwrap().to_bytes().unwrap() },
            before
        );
        let mut invalid = request.clone();
        invalid["spans"][0]["letter_spacing"] = serde_json::json!(-0.1);
        let input = CString::new(invalid.to_string()).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_measure_flowed_spans_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        assert!(output.data.is_null());
        assert_eq!(
            unsafe { document_ref(handle).unwrap().to_bytes().unwrap() },
            before
        );
        let mut draw = request;
        draw.as_object_mut().unwrap().remove("content_width");
        let input = CString::new(draw.to_string()).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            unsafe { ofd_add_flowed_spans_json(handle, 0, input.as_ptr(), &mut output) },
            0
        );
        unsafe { ofd_buffer_free(output) };
        assert_eq!(
            unsafe { document_ref(handle).unwrap().page_text(0).unwrap() },
            ["\u{301}"]
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_measurement_matches_font_table_reference_and_does_not_change_the_document() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../ofdrw/tests/fixtures/text-measurement.json"
        ))
        .unwrap();
        for shape in [false, true] {
            let mut handle = ptr::null_mut();
            assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
            let font = unsafe {
                document_mut(handle)
                    .unwrap()
                    .add_embedded_font(
                        "DejaVu Serif",
                        include_bytes!("../../ofdrw/tests/fixtures/dejavu-serif.ttf"),
                    )
                    .unwrap()
            };
            let before = unsafe { document_ref(handle).unwrap().to_bytes().unwrap() };
            let mut request = fixture["request"].clone();
            request["font_id"] = serde_json::json!(font.to_string());
            request["shape"] = serde_json::json!(shape);
            let input = CString::new(request.to_string()).unwrap();
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_eq!(
                unsafe { ofd_measure_flowed_spans_json(handle, 0, input.as_ptr(), &mut output) },
                0
            );
            let actual: serde_json::Value = serde_json::from_slice(unsafe {
                std::slice::from_raw_parts(output.data, output.len)
            })
            .unwrap();
            assert_eq!(actual, fixture["expected"]);
            unsafe { ofd_buffer_free(output) };
            assert_eq!(
                unsafe { document_ref(handle).unwrap().to_bytes().unwrap() },
                before
            );
            assert!(unsafe { document_ref(handle).unwrap().pages().unwrap() }.is_empty());
            assert_ne!(
                unsafe {
                    ofd_measure_flowed_spans_json(handle, 0, input.as_ptr(), ptr::null_mut())
                },
                0
            );
            for bad in [
                serde_json::json!({}),
                serde_json::json!({"content_width":"20","spans":[{"text":"A"}]}),
                serde_json::json!({"content_width":0,"spans":[{"text":"A"}]}),
                serde_json::json!({"content_width":20,"spans":[]}),
                serde_json::json!({"content_width":20,"spans":[{"text":"A","font_size":-1}]}),
            ] {
                let input = CString::new(bad.to_string()).unwrap();
                let mut output = OfdBuffer {
                    data: ptr::null_mut(),
                    len: 0,
                };
                assert_ne!(
                    unsafe {
                        ofd_measure_flowed_spans_json(handle, 0, input.as_ptr(), &mut output)
                    },
                    0
                );
                assert!(output.data.is_null());
                assert_eq!(
                    unsafe { document_ref(handle).unwrap().to_bytes().unwrap() },
                    before
                );
            }
            unsafe { ofd_close(handle) };
            let mut output = OfdBuffer {
                data: ptr::null_mut(),
                len: 0,
            };
            assert_ne!(
                unsafe { ofd_measure_flowed_spans_json(handle, 0, input.as_ptr(), &mut output) },
                0
            );
            assert_ne!(
                unsafe {
                    ofd_measure_flowed_spans_json(ptr::null_mut(), 0, input.as_ptr(), &mut output)
                },
                0
            );
        }
    }
}
