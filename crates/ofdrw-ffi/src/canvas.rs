use super::*;
use ofdrw::{CanvasCommand, CanvasFont, CanvasTextAlign};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    boundary: [f64; 4],
    commands: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FontInput {
    font_id: Option<String>,
    size: f64,
    letter_spacing: f64,
    weight: u16,
    italic: bool,
    read_direction: u16,
    char_direction: u16,
    align: String,
}

impl Default for FontInput {
    fn default() -> Self {
        Self {
            font_id: None,
            size: 1.,
            letter_spacing: 0.,
            weight: 400,
            italic: false,
            read_direction: 0,
            char_direction: 0,
            align: "start".into(),
        }
    }
}

impl FontInput {
    fn into_core(self) -> Result<CanvasFont, String> {
        Ok(CanvasFont {
            font_id: self.font_id,
            size: self.size,
            letter_spacing: self.letter_spacing,
            weight: self.weight,
            italic: self.italic,
            read_direction: self.read_direction,
            char_direction: self.char_direction,
            align: match self.align.as_str() {
                "start" => CanvasTextAlign::Start,
                "end" => CanvasTextAlign::End,
                "left" => CanvasTextAlign::Left,
                "right" => CanvasTextAlign::Right,
                "center" => CanvasTextAlign::Center,
                _ => return Err("unknown canvas text alignment".into()),
            },
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasureRequest {
    text: String,
    #[serde(default)]
    font: FontInput,
}

pub(super) fn parse_measure(source: &str) -> Result<(String, CanvasFont), String> {
    if source.len() > 16 * 1024 * 1024 {
        return Err("canvas JSON exceeds 16 MiB".into());
    }
    let request: MeasureRequest = serde_json::from_str(source).map_err(|e| e.to_string())?;
    Ok((request.text, request.font.into_core()?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FontStyleRequest {
    style: String,
    #[serde(default)]
    font: FontInput,
    #[serde(default = "default_pixels_per_mm")]
    pixels_per_mm: f64,
}

fn default_pixels_per_mm() -> f64 {
    3.78
}

pub(super) fn parse_font_style(source: &str) -> Result<(String, CanvasFont, f64), String> {
    if source.len() > 16 * 1024 * 1024 {
        return Err("canvas JSON exceeds 16 MiB".into());
    }
    let request: FontStyleRequest = serde_json::from_str(source).map_err(|e| e.to_string())?;
    Ok((
        request.style,
        request.font.into_core()?,
        request.pixels_per_mm,
    ))
}

pub(super) fn font_json(font: &CanvasFont) -> serde_json::Value {
    let align = match font.align {
        CanvasTextAlign::Start => "start",
        CanvasTextAlign::End => "end",
        CanvasTextAlign::Left => "left",
        CanvasTextAlign::Right => "right",
        CanvasTextAlign::Center => "center",
    };
    serde_json::json!({
        "font_id": font.font_id, "size": font.size,
        "letter_spacing": font.letter_spacing, "weight": font.weight,
        "italic": font.italic, "read_direction": font.read_direction,
        "char_direction": font.char_direction, "align": align,
    })
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PatternRequest {
    repetition: String,
    pixels_per_mm: f64,
    image_size: Option<[f64; 2]>,
    ctm: Option<[f64; 6]>,
}

impl Default for PatternRequest {
    fn default() -> Self {
        Self {
            repetition: "repeat".into(),
            pixels_per_mm: 3.78,
            image_size: None,
            ctm: None,
        }
    }
}

pub(super) fn create_pattern(
    document: &mut OfdDocument,
    page: usize,
    data: &[u8],
    format: &str,
    source: &str,
) -> Result<Vec<u8>, String> {
    if source.len() > 16 * 1024 * 1024 {
        return Err("canvas JSON exceeds 16 MiB".into());
    }
    let request: PatternRequest = serde_json::from_str(source).map_err(|e| e.to_string())?;
    let mut staged = document.clone();
    let mut pattern = staged
        .create_canvas_pattern(
            page,
            data,
            format,
            &request.repetition,
            request.pixels_per_mm,
        )
        .map_err(|e| e.to_string())?;
    if let Some([width, height]) = request.image_size {
        pattern
            .set_image_size(width, height)
            .map_err(|e| e.to_string())?;
    }
    if let Some(ctm) = request.ctm {
        pattern.set_transform(ctm).map_err(|e| e.to_string())?;
    }
    let style = pattern.as_pattern_style();
    let cells = style
        .cells
        .iter()
        .map(|cell| match cell {
            ofdrw::PatternCell::TransformedImage {
                resource_id,
                boundary,
                ctm,
            } => Ok(serde_json::json!({
                "kind":"image", "resource_id":resource_id, "boundary":boundary, "ctm":ctm,
            })),
            _ => Err("unexpected Canvas pattern cell".to_owned()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let reflect = match style.reflect {
        ofdrw::PatternReflectMethod::Normal => "normal",
        ofdrw::PatternReflectMethod::Row => "row",
        ofdrw::PatternReflectMethod::Column => "column",
        ofdrw::PatternReflectMethod::RowAndColumn => "row_and_column",
    };
    let mut value = serde_json::json!({"width":style.width,"height":style.height,
        "reflect":reflect,"relative_to":"object","cells":cells});
    if let Some(ctm) = style.ctm {
        value["ctm"] = serde_json::json!(ctm);
    }
    let output = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    *document = staged;
    Ok(output)
}

// Empty struct variants reject stray fields on commands without arguments.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    SetFont {
        font: FontInput,
    },
    FillText {
        text: String,
        point: [f64; 2],
    },
    DrawImage {
        data_base64: String,
        format: String,
        rect: [f64; 4],
    },
    DrawImageAt {
        data_base64: String,
        format: String,
        point: [f64; 2],
        pixels_per_mm: Option<f64>,
    },
    DrawImageRegion {
        data_base64: String,
        format: String,
        source: [f64; 4],
        rect: [f64; 4],
        pixels_per_mm: Option<f64>,
    },
    BeginPath {},
    ClosePath {},
    MoveTo {
        point: [f64; 2],
    },
    LineTo {
        point: [f64; 2],
    },
    QuadraticCurveTo {
        control: [f64; 2],
        end: [f64; 2],
    },
    BezierCurveTo {
        control1: [f64; 2],
        control2: [f64; 2],
        end: [f64; 2],
    },
    ArcTo {
        radii: [f64; 2],
        angle: f64,
        #[serde(default)]
        large: bool,
        #[serde(default)]
        sweep: bool,
        end: [f64; 2],
    },
    CircleArc {
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        #[serde(default)]
        sweep: bool,
    },
    Rect {
        rect: [f64; 4],
    },
    Stroke {},
    Fill {
        #[serde(default)]
        even_odd: bool,
    },
    FillRect {
        rect: [f64; 4],
    },
    StrokeRect {
        rect: [f64; 4],
    },
    Clip {
        #[serde(default)]
        even_odd: bool,
    },
    Save {},
    Restore {},
    Translate {
        offset: [f64; 2],
    },
    Scale {
        factors: [f64; 2],
    },
    Rotate {
        angle: f64,
    },
    Transform {
        matrix: [f64; 6],
    },
    SetTransform {
        matrix: [f64; 6],
    },
    SetDrawParam {
        style: serde_json::Value,
    },
    SetLineWidth {
        width: f64,
    },
    SetFillColor {
        color: [u8; 3],
    },
    SetStrokeColor {
        color: [u8; 3],
    },
    SetGlobalAlpha {
        alpha: Option<f64>,
    },
}

impl Command {
    fn into_core(self) -> Result<CanvasCommand, String> {
        Ok(match self {
            Self::SetFont { font } => CanvasCommand::SetFont {
                font: font.into_core()?,
            },
            Self::FillText { text, point } => CanvasCommand::FillText { text, point },
            Self::DrawImage {
                data_base64,
                format,
                rect,
            } => CanvasCommand::DrawImage {
                data: STANDARD
                    .decode(data_base64)
                    .map_err(|e| format!("invalid canvas image base64: {e}"))?,
                format,
                rect,
            },
            Self::DrawImageAt {
                data_base64,
                format,
                point,
                pixels_per_mm,
            } => CanvasCommand::DrawImageAt {
                data: STANDARD
                    .decode(data_base64)
                    .map_err(|e| format!("invalid canvas image base64: {e}"))?,
                format,
                point,
                pixels_per_mm: pixels_per_mm.unwrap_or(3.78),
            },
            Self::DrawImageRegion {
                data_base64,
                format,
                source,
                rect,
                pixels_per_mm,
            } => CanvasCommand::DrawImageRegion {
                data: STANDARD
                    .decode(data_base64)
                    .map_err(|e| format!("invalid canvas image base64: {e}"))?,
                format,
                source,
                rect,
                pixels_per_mm: pixels_per_mm.unwrap_or(3.78),
            },
            Self::BeginPath {} => CanvasCommand::BeginPath,
            Self::ClosePath {} => CanvasCommand::ClosePath,
            Self::MoveTo { point } => CanvasCommand::MoveTo { point },
            Self::LineTo { point } => CanvasCommand::LineTo { point },
            Self::QuadraticCurveTo { control, end } => {
                CanvasCommand::QuadraticCurveTo { control, end }
            }
            Self::BezierCurveTo {
                control1,
                control2,
                end,
            } => CanvasCommand::BezierCurveTo {
                control1,
                control2,
                end,
            },
            Self::ArcTo {
                radii,
                angle,
                large,
                sweep,
                end,
            } => CanvasCommand::ArcTo {
                radii,
                angle,
                large,
                sweep,
                end,
            },
            Self::CircleArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep,
            } => CanvasCommand::CircleArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep,
            },
            Self::Rect { rect } => CanvasCommand::Rect { rect },
            Self::Stroke {} => CanvasCommand::Stroke,
            Self::Fill { even_odd } => CanvasCommand::Fill { even_odd },
            Self::FillRect { rect } => CanvasCommand::FillRect { rect },
            Self::StrokeRect { rect } => CanvasCommand::StrokeRect { rect },
            Self::Clip { even_odd } => CanvasCommand::Clip { even_odd },
            Self::Save {} => CanvasCommand::Save,
            Self::Restore {} => CanvasCommand::Restore,
            Self::Translate { offset } => CanvasCommand::Translate { offset },
            Self::Scale { factors } => CanvasCommand::Scale { factors },
            Self::Rotate { angle } => CanvasCommand::Rotate { angle },
            Self::Transform { matrix } => CanvasCommand::Transform { matrix },
            Self::SetTransform { matrix } => CanvasCommand::SetTransform { matrix },
            Self::SetDrawParam { style } => CanvasCommand::SetDrawParam {
                style: Box::new(parse_draw_param_style_json(&style.to_string())?),
            },
            Self::SetLineWidth { width } => CanvasCommand::SetLineWidth { width },
            Self::SetFillColor { color } => CanvasCommand::SetFillColor { color },
            Self::SetStrokeColor { color } => CanvasCommand::SetStrokeColor { color },
            Self::SetGlobalAlpha { alpha } => CanvasCommand::SetGlobalAlpha { alpha },
        })
    }
}

pub(super) fn parse(source: &str) -> Result<([f64; 4], Vec<CanvasCommand>), String> {
    if source.len() > 16 * 1024 * 1024 {
        return Err("canvas JSON exceeds 16 MiB".into());
    }
    let request: Request = serde_json::from_str(source).map_err(|e| e.to_string())?;
    if request.commands.len() > 10_000 {
        return Err("too many canvas commands".into());
    }
    let commands = request
        .commands
        .into_iter()
        .map(|value| {
            serde_json::from_value::<Command>(value)
                .map_err(|e| e.to_string())?
                .into_core()
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((request.boundary, commands))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(handle: *mut OfdDocument, value: &serde_json::Value) -> Result<Vec<String>, i32> {
        let input = CString::new(value.to_string()).unwrap();
        let mut buffer = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        let status = unsafe { ofd_draw_canvas_json(handle, 0, input.as_ptr(), &mut buffer) };
        if status != 0 {
            assert!(buffer.data.is_null());
            return Err(status);
        }
        let result =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) })
                .unwrap();
        unsafe { ofd_buffer_free(buffer) };
        Ok(result)
    }

    fn measure(
        handle: *mut OfdDocument,
        value: &serde_json::Value,
    ) -> Result<serde_json::Value, i32> {
        let input = CString::new(value.to_string()).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        let status =
            unsafe { ofd_measure_canvas_text_json(handle, 0, input.as_ptr(), &mut output) };
        if status != 0 {
            return Err(status);
        }
        let result =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        Ok(result)
    }

    fn measure_area(
        handle: *mut OfdDocument,
        value: &serde_json::Value,
    ) -> Result<serde_json::Value, i32> {
        let input = CString::new(value.to_string()).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        let status =
            unsafe { ofd_measure_canvas_text_area_json(handle, 0, input.as_ptr(), &mut output) };
        if status != 0 {
            return Err(status);
        }
        let result =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        Ok(result)
    }

    fn resolve_style(
        handle: *mut OfdDocument,
        request: &serde_json::Value,
    ) -> Result<serde_json::Value, i32> {
        let input = CString::new(request.to_string()).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        let status =
            unsafe { ofd_resolve_canvas_font_style_json(handle, 0, input.as_ptr(), &mut output) };
        if status != 0 {
            assert!(output.data.is_null());
            return Err(status);
        }
        let value =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(output.data, output.len) })
                .unwrap();
        unsafe { ofd_buffer_free(output) };
        Ok(value)
    }

    #[test]
    fn ffi_font_style_matches_java_cases_and_preserves_existing_resources() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../ofdrw/tests/fixtures/canvas-font-style-java.json"
        ))
        .unwrap();
        let mut doc = OfdDocument::new();
        doc.add_blank_page(40., 30.).unwrap();
        let font_data = include_bytes!("../../ofdrw/tests/fixtures/dejavu-sans.ttf");
        let latin = doc
            .add_embedded_font("CanvasTest", font_data)
            .unwrap()
            .to_string();
        let chinese = doc
            .add_embedded_font("宋体", font_data)
            .unwrap()
            .to_string();
        let before = doc.to_bytes().unwrap();
        let handle = register_document(doc).unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 986);
        for case in cases {
            let font = resolve_style(
                handle,
                &json!({
                    "style":case["input"], "pixels_per_mm":case["pixels_per_mm"],
                    "font":{"size":9., "weight":case["base_weight"], "italic":case["base_italic"],
                    "letter_spacing":0.25, "read_direction":90, "char_direction":270, "align":"end"}
                }),
            )
            .unwrap();
            let expected = &case["expected"];
            assert!(
                (font["size"].as_f64().unwrap() - expected["size"].as_f64().unwrap()).abs() < 1e-12,
                "{case}"
            );
            assert_eq!(font["weight"], expected["weight"], "{case}");
            assert_eq!(font["italic"], expected["italic"], "{case}");
            assert_eq!(
                font["font_id"],
                if expected["family"] == "宋体" {
                    chinese.as_str()
                } else {
                    latin.as_str()
                }
            );
            assert_eq!(font["letter_spacing"], 0.25);
            assert_eq!(font["read_direction"], 90);
            assert_eq!(font["char_direction"], 270);
            assert_eq!(font["align"], "end");
        }
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_font_style_rejects_bad_requests_before_mutating_resources() {
        let mut doc = OfdDocument::new();
        doc.add_blank_page(40., 30.).unwrap();
        let before = doc.to_bytes().unwrap();
        let handle = register_document(doc).unwrap();
        for request in [
            json!({"style":"bold 4mm NewFont", "pixels_per_mm":0}),
            json!({"style":"0mm NewFont"}),
            json!({"style":"4mm NewFont", "font":{"align":"invalid"}}),
            json!({"style":"4mm NewFont", "font":{"size":-1}}),
            json!({"style":"4mm NewFont", "extra":true}),
            json!({"style":"4mm NewFont", "font":{"extra":true}}),
            json!({"style":null}),
        ] {
            assert!(resolve_style(handle, &request).is_err(), "{request}");
        }
        let input = CString::new(r#"{"style":"4mm NewFont"}"#).unwrap();
        assert_ne!(
            unsafe {
                ofd_resolve_canvas_font_style_json(handle, 0, input.as_ptr(), ptr::null_mut())
            },
            0
        );
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(
            unsafe { ofd_resolve_canvas_font_style_json(handle, 1, input.as_ptr(), &mut output) },
            0
        );
        assert_ne!(
            unsafe { ofd_resolve_canvas_font_style_json(handle, 0, ptr::null(), &mut output) },
            0
        );
        let bad_utf8 = [255_u8, 0];
        assert_ne!(
            unsafe {
                ofd_resolve_canvas_font_style_json(handle, 0, bad_utf8.as_ptr().cast(), &mut output)
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let font = resolve_style(handle, &json!({"style":"bold 4mm NewFont"})).unwrap();
        assert_eq!(font["weight"], 700);
        assert_eq!(
            resolve_style(handle, &json!({"style":"bold 4mm NewFont"})).unwrap(),
            font
        );
        unsafe { ofd_close(handle) };
        assert!(resolve_style(handle, &json!({"style":"4mm NewFont"})).is_err());
    }

    #[test]
    fn ffi_canvas_area_returns_typed_json_and_preserves_document() {
        let mut doc = OfdDocument::new();
        doc.add_blank_page(40., 30.).unwrap();
        let id = doc
            .add_embedded_font(
                "CanvasTest",
                include_bytes!("../../ofdrw/tests/fixtures/dejavu-sans.ttf"),
            )
            .unwrap()
            .to_string();
        let before = doc.to_bytes().unwrap();
        let handle = register_document(doc).unwrap();
        let area = measure_area(
            handle,
            &json!({"text":"WiA", "font":{"font_id":id,"size":4,"letter_spacing":-1}}),
        )
        .unwrap();
        assert_eq!(
            area,
            json!({"width":7.802734375,"height":4.65625,"letter_spacing":0.,
            "char_areas":[[0.,0.,3.955078125,4.65625],[0.,0.,1.111328125,4.65625],[0.,0.,2.736328125,4.65625]]})
        );
        assert_eq!(
            measure_area(handle, &json!({"text":""})).unwrap(),
            json!({"width":0.,"height":0.,"letter_spacing":0.,"char_areas":[]})
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_canvas_area_rejects_invalid_requests_nulls_and_closed_handles() {
        let mut doc = OfdDocument::new();
        doc.add_blank_page(40., 30.).unwrap();
        let before = doc.to_bytes().unwrap();
        let handle = register_document(doc).unwrap();
        for request in [
            json!({"text":"x","extra":1}),
            json!({"text":12}),
            json!({"text":"x","font":{"size":0}}),
            json!({"text":"x","font":{"extra":1}}),
        ] {
            assert!(measure_area(handle, &request).is_err());
        }
        let input = CString::new(r#"{"text":"x"}"#).unwrap();
        let mut output = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        for (pointer, page, request, destination) in [
            (
                ptr::null(),
                0,
                input.as_ptr(),
                &mut output as *mut OfdBuffer,
            ),
            (handle as *const OfdDocument, 1, input.as_ptr(), &mut output),
            (handle as *const OfdDocument, 0, ptr::null(), &mut output),
            (
                handle as *const OfdDocument,
                0,
                input.as_ptr(),
                ptr::null_mut(),
            ),
        ] {
            assert_ne!(
                unsafe { ofd_measure_canvas_text_area_json(pointer, page, request, destination) },
                0
            );
            assert!(output.data.is_null());
        }
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
        assert!(measure_area(handle, &json!({"text":""})).is_err());
    }

    #[test]
    fn ffi_canvas_text_matches_all_java_metrics_and_reference_pixels() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../ofdrw/tests/fixtures/canvas-text-java");
        let requests: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("requests.json")).unwrap()).unwrap();
        let java = OfdDocument::open(root.join("directions.ofd")).unwrap();
        let mut doc = OfdDocument::new();
        doc.add_blank_page(90., 90.).unwrap();
        let area = java.pages().unwrap().remove(0);
        doc.set_page_area(
            0,
            PageArea {
                physical_box: area.physical_box.unwrap(),
                application_box: area.application_box,
                content_box: area.content_box,
                bleed_box: area.bleed_box,
            },
        )
        .unwrap();
        let font_id = doc
            .add_embedded_font(
                "CanvasTest",
                include_bytes!("../../ofdrw/tests/fixtures/dejavu-sans.ttf"),
            )
            .unwrap()
            .to_string();
        let before = doc.to_bytes().unwrap();
        let handle = register_document(doc).unwrap();
        let mut commands = vec![
            json!({"type":"translate","offset":[1,1]}),
            json!({"type":"set_global_alpha","alpha":0.5}),
        ];
        for case in requests["cases"].as_array().unwrap() {
            let mut font = case["font"].clone();
            font["font_id"] = json!(font_id);
            let actual = measure(handle, &json!({"text":requests["text"],"font":font})).unwrap();
            assert_eq!(actual, case["metrics"]);
            commands.push(json!({"type":"set_font","font":font}));
            commands
                .push(json!({"type":"fill_text","text":requests["text"],"point":case["point"]}));
        }
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let controls: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("controls.json")).unwrap()).unwrap();
        for case in controls["cases"].as_array().unwrap() {
            let text = format!(
                "A{}B",
                char::from_u32(case["code"].as_u64().unwrap() as u32).unwrap()
            );
            let actual = measure(
                handle,
                &json!({"text":text,"font":{"font_id":font_id,"size":4}}),
            )
            .unwrap();
            assert_eq!(actual, case["metrics"]);
        }
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let ids = call(
            handle,
            &json!({"boundary":requests["boundary"],"commands":commands}),
        )
        .unwrap();
        assert_eq!(ids.len(), 16);
        let doc = unsafe { document_ref(handle).unwrap() };
        for dpi in [25.4, 101.6] {
            assert_eq!(
                doc.page_png(0, dpi).unwrap(),
                java.page_png(0, dpi).unwrap()
            );
        }
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_canvas_text_rejects_invalid_settings_without_mutation() {
        let mut doc = OfdDocument::new();
        doc.add_blank_page(40., 30.).unwrap();
        let before = doc.to_bytes().unwrap();
        let handle = register_document(doc).unwrap();
        for font in [
            json!({"size":0}),
            json!({"weight":450}),
            json!({"align":"unknown"}),
            json!({"read_direction":1}),
            json!({"extra":1}),
        ] {
            assert!(measure(handle, &json!({"text":"text","font":font})).is_err());
            assert!(call(handle,&json!({"boundary":[0,0,40,30],"commands":[
                {"type":"fill_text","text":"first","point":[1,5]},
                {"type":"set_font","font":font},{"type":"fill_text","text":"last","point":[1,10]}
            ]})).is_err());
            assert_eq!(
                unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
                before
            );
        }
        assert!(measure(handle, &json!({"text":"x","extra":1})).is_err());
        let defaults = measure(handle, &json!({"text":""})).unwrap();
        assert_eq!(defaults["width"], json!(0.0));
        assert_eq!(defaults["offsets"], json!([]));
        assert_ne!(
            unsafe { ofd_measure_canvas_text_json(handle, 0, ptr::null(), ptr::null_mut()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn ffi_canvas_image_commands_match_java_pixels_and_reject_invalid_base64() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../ofdrw/tests/fixtures/canvas-images-java");
        let requests: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("requests.json")).unwrap()).unwrap();
        for case in requests["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let java = OfdDocument::open(root.join(format!("{name}.ofd"))).unwrap();
            let area = java.pages().unwrap().remove(0);
            let mut doc = OfdDocument::new();
            doc.add_blank_page(40., 30.).unwrap();
            doc.set_page_area(
                0,
                PageArea {
                    physical_box: area.physical_box.unwrap(),
                    application_box: area.application_box,
                    content_box: area.content_box,
                    bleed_box: area.bleed_box,
                },
            )
            .unwrap();
            let handle = register_document(doc).unwrap();
            let ids = call(
                handle,
                &json!({"boundary":requests["boundary"], "commands":case["commands"]}),
            )
            .unwrap();
            assert_eq!(ids.len(), 1);
            let doc = unsafe { document_ref(handle).unwrap() };
            for dpi in [25.4, 101.6] {
                assert_eq!(
                    doc.page_png(0, dpi).unwrap(),
                    java.page_png(0, dpi).unwrap(),
                    "{name} {dpi}"
                );
            }
            let before = doc.to_bytes().unwrap();
            assert!(call(
                handle,
                &json!({"boundary":[0,0,40,30],"commands":[
                    {"type":"draw_image","data_base64":"!invalid!","format":"png","rect":[0,0,1,1]}
                ]})
            )
            .is_err());
            assert_eq!(
                unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
                before
            );
            unsafe { ofd_close(handle) };
        }
        let (_, commands) = parse(
            &json!({"boundary":[0,0,40,30],"commands":[
                {"type":"draw_image_at","data_base64":"","format":"png","point":[0,0]}
            ]})
            .to_string(),
        )
        .unwrap();
        assert!(
            matches!(&commands[0], CanvasCommand::DrawImageAt { pixels_per_mm, .. } if *pixels_per_mm == 3.78)
        );
    }

    #[test]
    fn ffi_canvas_matches_java_reference_paths_styles_and_pixels() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../ofdrw/tests/fixtures/canvas-java");
        let requests: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("requests.json")).unwrap()).unwrap();
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("reference.json")).unwrap()).unwrap();
        for (case, reference) in requests["cases"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["cases"].as_array().unwrap())
        {
            assert_eq!(case["name"], reference["name"]);
            let name = case["name"].as_str().unwrap();
            let file = reference["render_reference"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{name}.ofd"));
            let java = OfdDocument::open(root.join(file)).unwrap();
            let area = java.pages().unwrap().remove(0);
            let mut native = OfdDocument::new();
            native.add_blank_page(40., 30.).unwrap();
            native
                .set_page_area(
                    0,
                    PageArea {
                        physical_box: area.physical_box.unwrap(),
                        application_box: area.application_box,
                        content_box: area.content_box,
                        bleed_box: area.bleed_box,
                    },
                )
                .unwrap();
            let handle = register_document(native).unwrap();
            let ids = call(
                handle,
                &json!({"boundary":requests["boundary"],"commands":case["commands"]}),
            )
            .unwrap();
            let doc = unsafe { document_ref(handle).unwrap() };
            assert_eq!(ids.len(), reference["objects"].as_array().unwrap().len());
            for (index, (path, style)) in doc
                .page_paths(0)
                .unwrap()
                .iter()
                .zip(doc.page_object_stroke_styles(0).unwrap())
                .enumerate()
            {
                assert_eq!(path.id.as_ref(), Some(&ids[index]));
                assert_eq!(
                    style.style.line_width,
                    reference["objects"][index]["line_width"].as_f64().unwrap()
                );
            }
            for dpi in [25.4, 101.6] {
                assert_eq!(
                    doc.page_png(0, dpi).unwrap(),
                    java.page_png(0, dpi).unwrap(),
                    "{name} at {dpi} DPI"
                );
            }
            unsafe { ofd_close(handle) };
        }
    }

    #[test]
    fn ffi_canvas_rejects_unknown_malformed_and_late_invalid_commands_atomically() {
        let mut handle = ptr::null_mut();
        assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
        assert_eq!(unsafe { ofd_add_blank_page(handle, 40., 30.) }, 0);
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        for bad in [
            json!({"type":"not_a_command"}),
            json!({"type":"stroke","extra":1}),
            json!({"type":"begin_path","point":[1,2]}),
            json!({"type":"move_to","point":[1]}),
            json!({"type":"move_to","point":[1,2],"extra":true}),
            json!({"type":"fill","even_odd":1}),
            json!({"type":"set_line_width","width":-1}),
            json!({"type":"set_line_width","width":null}),
            json!({"type":"set_global_alpha","alpha":1.1}),
            json!({"type":"set_fill_color","color":[0,256,0]}),
            json!({"type":"set_draw_param","style":{"line_width":0,"extra":1}}),
            json!({"type":"set_draw_param","style":{"dash_sequence":[0]}}),
            json!({"type":"arc_to","radii":[-1,2],"angle":0,"end":[1,2]}),
        ] {
            assert!(call(handle,&json!({"boundary":[4,3,24,20],"commands":[{"type":"rect","rect":[1,1,4,4]},{"type":"stroke"},bad]})).is_err());
            assert_eq!(
                unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
                before
            );
        }
        for request in [
            json!({"boundary":[0,0,0,20],"commands":[]}),
            json!({"boundary":[0,0,20,20],"commands":null}),
            json!({"boundary":[0,0,20,20],"commands":[],"extra":1}),
        ] {
            assert!(call(handle, &request).is_err());
        }
        assert!(call(handle, &json!({"boundary":[0,0,20,20],"commands":[]}))
            .unwrap()
            .is_empty());
        let input = CString::new(r#"{"boundary":[0,0,20,20],"commands":[]}"#).unwrap();
        assert_ne!(
            unsafe { ofd_draw_canvas_json(handle, 0, input.as_ptr(), ptr::null_mut()) },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        unsafe { ofd_close(handle) };
    }

    #[test]
    fn canvas_protocol_preserves_zeroes_clears_alpha_and_checks_request_limits() {
        let (_,commands)=parse(r#"{"boundary":[0,0,20,20],"commands":[{"type":"set_line_width","width":0},{"type":"rotate","angle":0},{"type":"set_global_alpha","alpha":0},{"type":"set_global_alpha","alpha":null},{"type":"set_draw_param","style":{"line_width":0,"stroke":{"color":[0,0,0]}}}]}"#).unwrap();
        assert!(matches!(
            commands[0],
            CanvasCommand::SetLineWidth { width: 0. }
        ));
        assert!(matches!(
            commands[2],
            CanvasCommand::SetGlobalAlpha { alpha: Some(0.) }
        ));
        assert!(matches!(
            commands[3],
            CanvasCommand::SetGlobalAlpha { alpha: None }
        ));
        let oversized = format!(
            "{}{}",
            " ".repeat(16 * 1024 * 1024),
            r#"{"boundary":[0,0,20,20],"commands":[]}"#
        );
        assert!(parse(&oversized).unwrap_err().contains("exceeds 16 MiB"));
        assert!(parse(
            &json!({"boundary":[0,0,20,20],"commands":vec![json!({"type":"save"});10_001]})
                .to_string()
        )
        .is_err());
    }
}
