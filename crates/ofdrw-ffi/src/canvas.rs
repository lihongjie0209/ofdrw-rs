use super::*;
use ofdrw::CanvasCommand;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    boundary: [f64; 4],
    commands: Vec<serde_json::Value>,
}

// Empty struct variants reject stray fields on commands without arguments.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
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
