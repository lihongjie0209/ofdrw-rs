//! Stateful vector drawing using the Java OFDRW DrawContext path model.
use crate::{DrawParamStyle, OfdDocument, OfdError, PathClipArea, PathStyle, SolidPaintStyle};
use std::fmt::Write;

const IDENTITY: [f64; 6] = [1., 0., 0., 1., 0., 0.];
const MAX_COMMANDS: usize = 10_000;
const MAX_STACK: usize = 256;
const MAX_SAVED_PATH_BYTES: usize = 16 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 4 * 1024 * 1024;

/// One command in an atomic vector canvas drawing operation.
///
/// Coordinates and widths are millimeters; angles are degrees. Paths are
/// transformed when painted, rather than when each segment is added. Save and
/// restore include the current path, matching OFDRW's DrawContext model.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CanvasCommand {
    BeginPath,
    ClosePath,
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
    /// Endpoint-form elliptical arc. `sweep` selects the OFD sweep flag.
    ArcTo {
        radii: [f64; 2],
        angle: f64,
        large: bool,
        sweep: bool,
        end: [f64; 2],
    },
    /// Center-form circular arc, including the Java two-segment full circle.
    /// The `sweep` flag is passed directly to the OFD arc command.
    CircleArc {
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        sweep: bool,
    },
    Rect {
        rect: [f64; 4],
    },
    Stroke,
    /// Java fill also retains a zero-width stroke around the filled path.
    Fill {
        even_odd: bool,
    },
    FillRect {
        rect: [f64; 4],
    },
    StrokeRect {
        rect: [f64; 4],
    },
    /// Replace the clip with the current path, frozen under the current CTM.
    Clip {
        even_odd: bool,
    },
    Save,
    /// An empty stack is a no-op.
    Restore,
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
    /// Replace drawing parameters, including solid/gradient/mesh/pattern paints.
    SetDrawParam {
        style: Box<DrawParamStyle>,
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
    /// `None` clears the explicit global opacity. Values must be in 0..=1.
    SetGlobalAlpha {
        alpha: Option<f64>,
    },
}

#[derive(Clone)]
struct Clip {
    path: String,
    matrix: [f64; 6],
    even_odd: bool,
}

#[derive(Clone)]
struct State {
    path: Option<String>,
    matrix: [f64; 6],
    draw: DrawParamStyle,
    alpha: Option<f64>,
    clip: Option<Clip>,
}

impl Default for State {
    fn default() -> Self {
        let black = SolidPaintStyle {
            color: [0; 3],
            alpha: 255,
        };
        Self {
            path: None,
            matrix: IDENTITY,
            draw: DrawParamStyle {
                fill: Some(black),
                stroke: Some(black),
                ..Default::default()
            },
            alpha: None,
            clip: None,
        }
    }
}

fn finite(values: &[f64]) -> Result<(), OfdError> {
    if values.iter().any(|v| !v.is_finite()) {
        return Err(OfdError::Invalid(
            "canvas coordinates must be finite".into(),
        ));
    }
    Ok(())
}

fn append(path: &mut Option<String>, command: &str, values: &[f64]) -> Result<(), OfdError> {
    finite(values)?;
    let path = path.get_or_insert_with(String::new);
    if !path.is_empty() {
        path.push(' ');
    }
    path.push_str(command);
    for value in values {
        // Writing to a String is infallible.
        let _ = write!(path, " {value}");
    }
    if path.len() > MAX_PATH_BYTES {
        return Err(OfdError::Limit("canvas path is too large".into()));
    }
    Ok(())
}

fn rectangle(rect: [f64; 4]) -> Result<String, OfdError> {
    finite(&rect)?;
    let [x, y, w, h] = rect;
    let mut path = None;
    append(&mut path, "M", &[x, y])?;
    append(&mut path, "L", &[x + w, y])?;
    append(&mut path, "L", &[x + w, y + h])?;
    append(&mut path, "L", &[x, y + h])?;
    append(&mut path, "C", &[])?;
    Ok(path.unwrap_or_default())
}

fn multiply(parent: [f64; 6], child: [f64; 6]) -> Result<[f64; 6], OfdError> {
    finite(&child)?;
    let [a, b, c, d, e, f] = parent;
    let [g, h, i, j, k, l] = child;
    let result = [
        a * g + c * h,
        b * g + d * h,
        a * i + c * j,
        b * i + d * j,
        a * k + c * l + e,
        b * k + d * l + f,
    ];
    finite(&result)?;
    Ok(result)
}

fn arc(
    path: &mut Option<String>,
    radii: [f64; 2],
    angle: f64,
    large: bool,
    sweep: bool,
    end: [f64; 2],
) -> Result<(), OfdError> {
    finite(&[radii[0], radii[1], angle, end[0], end[1]])?;
    if radii.iter().any(|v| *v < 0.) {
        return Err(OfdError::Invalid(
            "canvas arc radii must be nonnegative".into(),
        ));
    }
    append(
        path,
        "A",
        &[
            radii[0],
            radii[1],
            angle % 360.,
            u8::from(large).into(),
            u8::from(sweep).into(),
            end[0],
            end[1],
        ],
    )
}

impl OfdDocument {
    /// Draw a vector canvas on an existing global page, atomically.
    ///
    /// All painted objects use `boundary` as their canvas. Returns object IDs in
    /// paint order. State starts fresh for each call. Rect helpers preserve the
    /// current path; clipping replaces the prior clip and freezes its CTM.
    ///
    /// Unlike the pinned Java CanvasState clone bug, restore preserves drawing
    /// parameters as well as paths, transforms, colors, opacity and clipping.
    /// Text/image canvas commands and CSS font/color parsers are not included.
    ///
    /// ```
    /// use ofdrw::{CanvasCommand, OfdDocument};
    /// let mut doc = OfdDocument::new();
    /// doc.add_blank_page(40., 30.)?;
    /// let ids = doc.draw_canvas(0, [4., 3., 24., 20.], &[
    ///     CanvasCommand::SetStrokeColor { color: [0, 80, 200] },
    ///     CanvasCommand::Rect { rect: [1., 2., 12., 8.] },
    ///     CanvasCommand::Stroke,
    /// ])?;
    /// assert_eq!(ids.len(), 1);
    /// # Ok::<(), ofdrw::OfdError>(())
    /// ```
    pub fn draw_canvas(
        &mut self,
        page_index: usize,
        boundary: [f64; 4],
        commands: &[CanvasCommand],
    ) -> Result<Vec<String>, OfdError> {
        finite(&boundary)?;
        if boundary[2] <= 0. || boundary[3] <= 0. {
            return Err(OfdError::Invalid(
                "canvas boundary must have positive dimensions".into(),
            ));
        }
        if commands.len() > MAX_COMMANDS {
            return Err(OfdError::Limit("too many canvas commands".into()));
        }
        let (document_index, path) = self.page_location(page_index)?;
        self.ensure_unsigned_page_path(document_index, &path)?;
        let mut staged = self.clone();
        let mut state = State::default();
        let mut stack = Vec::new();
        let mut parameters: Vec<(DrawParamStyle, String)> = Vec::new();
        let mut saved_path_bytes = 0usize;
        let mut objects = Vec::new();
        for command in commands {
            match command {
                CanvasCommand::BeginPath => state.path = Some(String::new()),
                CanvasCommand::ClosePath => {
                    if state.path.as_ref().is_some_and(|p| !p.is_empty()) {
                        append(&mut state.path, "C", &[])?;
                    }
                }
                CanvasCommand::MoveTo { point } => append(&mut state.path, "M", point)?,
                CanvasCommand::LineTo { point } => {
                    finite(point)?;
                    if state.path.is_some() {
                        append(&mut state.path, "L", point)?;
                    }
                }
                CanvasCommand::QuadraticCurveTo { control, end } => append(
                    &mut state.path,
                    "Q",
                    &[control[0], control[1], end[0], end[1]],
                )?,
                CanvasCommand::BezierCurveTo {
                    control1,
                    control2,
                    end,
                } => append(
                    &mut state.path,
                    "B",
                    &[
                        control1[0],
                        control1[1],
                        control2[0],
                        control2[1],
                        end[0],
                        end[1],
                    ],
                )?,
                CanvasCommand::ArcTo {
                    radii,
                    angle,
                    large,
                    sweep,
                    end,
                } => arc(&mut state.path, *radii, *angle, *large, *sweep, *end)?,
                CanvasCommand::CircleArc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    sweep,
                } => {
                    finite(&[center[0], center[1], *radius, *start_angle, *end_angle])?;
                    if *radius < 0. {
                        return Err(OfdError::Invalid(
                            "canvas arc radius must be nonnegative".into(),
                        ));
                    }
                    let start = start_angle.to_radians();
                    let first = [
                        center[0] + radius * start.cos(),
                        center[1] + radius * start.sin(),
                    ];
                    append(&mut state.path, "M", &first)?;
                    let angle = end_angle - start_angle;
                    if angle == 360. {
                        arc(
                            &mut state.path,
                            [*radius; 2],
                            angle,
                            true,
                            *sweep,
                            [center[0] - radius, center[1]],
                        )?;
                        arc(&mut state.path, [*radius; 2], angle, true, *sweep, first)?;
                    } else {
                        let end = end_angle.to_radians();
                        arc(
                            &mut state.path,
                            [*radius; 2],
                            angle,
                            angle > 180.,
                            *sweep,
                            [
                                center[0] + radius * end.cos(),
                                center[1] + radius * end.sin(),
                            ],
                        )?;
                    }
                }
                CanvasCommand::Rect { rect } => {
                    let extra = rectangle(*rect)?;
                    let path = state.path.get_or_insert_with(String::new);
                    if path.len().saturating_add(extra.len()).saturating_add(1) > MAX_PATH_BYTES {
                        return Err(OfdError::Limit("canvas path is too large".into()));
                    }
                    if !path.is_empty() {
                        path.push(' ');
                    }
                    path.push_str(&extra);
                }
                CanvasCommand::Save => {
                    if stack.len() >= MAX_STACK {
                        return Err(OfdError::Limit("canvas state stack is too deep".into()));
                    }
                    let bytes = state.path.as_ref().map_or(0, String::len)
                        + state.clip.as_ref().map_or(0, |clip| clip.path.len());
                    saved_path_bytes = saved_path_bytes
                        .checked_add(bytes)
                        .filter(|total| *total <= MAX_SAVED_PATH_BYTES)
                        .ok_or_else(|| {
                            OfdError::Limit("saved canvas paths are too large".into())
                        })?;
                    stack.push(state.clone());
                }
                CanvasCommand::Restore => {
                    if let Some(saved) = stack.pop() {
                        saved_path_bytes -= saved.path.as_ref().map_or(0, String::len)
                            + saved.clip.as_ref().map_or(0, |clip| clip.path.len());
                        state = saved;
                    }
                }
                CanvasCommand::Translate { offset } => {
                    state.matrix = multiply(state.matrix, [1., 0., 0., 1., offset[0], offset[1]])?
                }
                CanvasCommand::Scale { factors } => {
                    state.matrix = multiply(state.matrix, [factors[0], 0., 0., factors[1], 0., 0.])?
                }
                CanvasCommand::Rotate { angle } => {
                    finite(&[*angle])?;
                    let radians = angle.to_radians();
                    state.matrix = multiply(
                        state.matrix,
                        [
                            radians.cos(),
                            radians.sin(),
                            -radians.sin(),
                            radians.cos(),
                            0.,
                            0.,
                        ],
                    )?;
                }
                CanvasCommand::Transform { matrix } => {
                    state.matrix = multiply(state.matrix, *matrix)?
                }
                CanvasCommand::SetTransform { matrix } => {
                    finite(matrix)?;
                    state.matrix = *matrix;
                }
                CanvasCommand::SetDrawParam { style } => {
                    style.validate()?;
                    state.draw = style.as_ref().clone();
                }
                CanvasCommand::SetLineWidth { width } => {
                    finite(&[*width])?;
                    if *width < 0. {
                        return Err(OfdError::Invalid(
                            "canvas line width must be nonnegative".into(),
                        ));
                    }
                    state.draw.line_width = Some(*width);
                }
                CanvasCommand::SetFillColor { color } => {
                    state.draw.fill = Some(SolidPaintStyle {
                        color: *color,
                        alpha: 255,
                    });
                    state.draw.fill_gradient = None;
                    state.draw.fill_gouraud = None;
                    state.draw.fill_pattern = None;
                }
                CanvasCommand::SetStrokeColor { color } => {
                    state.draw.stroke = Some(SolidPaintStyle {
                        color: *color,
                        alpha: 255,
                    });
                    state.draw.stroke_gradient = None;
                    state.draw.stroke_gouraud = None;
                    state.draw.stroke_pattern = None;
                }
                CanvasCommand::SetGlobalAlpha { alpha } => {
                    if alpha.is_some_and(|v| !v.is_finite() || !(0. ..=1.).contains(&v)) {
                        return Err(OfdError::Invalid("canvas alpha must be in 0..=1".into()));
                    }
                    state.alpha = *alpha;
                }
                CanvasCommand::Clip { even_odd } => {
                    if let Some(path) = state.path.as_ref().filter(|p| !p.is_empty()) {
                        crate::svg_path_data(path)?;
                        state.clip = Some(Clip {
                            path: path.clone(),
                            matrix: state.matrix,
                            even_odd: *even_odd,
                        });
                    }
                }
                CanvasCommand::Stroke
                | CanvasCommand::Fill { .. }
                | CanvasCommand::FillRect { .. }
                | CanvasCommand::StrokeRect { .. } => {
                    let temporary;
                    let path = match command {
                        CanvasCommand::FillRect { rect } | CanvasCommand::StrokeRect { rect } => {
                            temporary = rectangle(*rect)?;
                            &temporary
                        }
                        _ => {
                            let Some(path) = state.path.as_ref().filter(|p| !p.is_empty()) else {
                                continue;
                            };
                            path
                        }
                    };
                    let fill = matches!(
                        command,
                        CanvasCommand::Fill { .. } | CanvasCommand::FillRect { .. }
                    );
                    let even_odd = matches!(command, CanvasCommand::Fill { even_odd: true });
                    let hairline = matches!(command, CanvasCommand::Fill { .. });
                    let param = if let Some((_, id)) =
                        parameters.iter().find(|(style, _)| style == &state.draw)
                    {
                        id.clone()
                    } else {
                        let id = staged.add_document_draw_param(document_index, &state.draw)?;
                        parameters.push((state.draw.clone(), id.clone()));
                        id
                    };
                    let id = staged.add_path_object(
                        page_index,
                        path,
                        boundary,
                        PathStyle {
                            fill,
                            even_odd,
                            line_width: 0.,
                            alpha: state.alpha.map(|v| (255. * v) as u8),
                            ..Default::default()
                        },
                    )?;
                    if !hairline {
                        staged.set_page_object_attribute(page_index, &id, "LineWidth", None)?;
                    }
                    staged.set_page_object_draw_param(page_index, &id, Some(&param))?;
                    if state.matrix != IDENTITY {
                        staged.set_page_object_ctm(page_index, &id, Some(state.matrix))?;
                    }
                    if let Some(clip) = &state.clip {
                        staged.set_page_object_path_clips(
                            page_index,
                            &id,
                            &[vec![PathClipArea {
                                boundary: [0., 0., boundary[2], boundary[3]],
                                commands: clip.path.clone(),
                                area_ctm: None,
                                path_ctm: Some(clip.matrix),
                                even_odd: clip.even_odd,
                                fill: Some(true),
                                // Java CT_Path also strokes clip paths by default.
                                stroke: Some(true),
                                draw_param: None,
                            }]],
                            false,
                        )?;
                    }
                    objects.push(id);
                }
            }
        }
        *self = staged;
        Ok(objects)
    }
}
