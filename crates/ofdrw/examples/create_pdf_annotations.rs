use std::env;

use ofdrw::{
    AnnotationGraphic, AnnotationPath, CompositeStyle, OfdDocument, PathStyle,
    ShapeAnnotationStyle, TextNoteStyle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args().nth(1).ok_or("missing output path")?;
    let mut document = OfdDocument::new();
    document.add_blank_page(80.0, 50.0)?;
    document.add_shape_annotation(
        0,
        "Square",
        [5.0, 5.0, 20.0, 15.0],
        &ShapeAnnotationStyle::default(),
    )?;
    document.add_shape_annotation(
        0,
        "Circle",
        [30.0, 5.0, 15.0, 15.0],
        &ShapeAnnotationStyle::default(),
    )?;
    document.add_text_note_annotation(
        0,
        "Review",
        [50.0, 5.0, 10.0, 10.0],
        &TextNoteStyle::default(),
    )?;
    document.add_highlight_annotation(0, [5.0, 25.0, 25.0, 5.0], [255, 255, 0], 96)?;
    let multi_id = document.add_multi_path_annotation(
        0,
        [40.0, 25.0, 25.0, 15.0],
        &[
            AnnotationPath {
                commands: "M 0 0 L 10 10".into(),
                boundary: [0.0, 0.0, 10.0, 10.0],
                style: PathStyle::default(),
            },
            AnnotationPath {
                commands: "M 0 10 L 10 0".into(),
                boundary: [10.0, 0.0, 10.0, 10.0],
                style: PathStyle::default(),
            },
        ],
    )?;
    let first_path_id = document.annotation_objects(0, &multi_id)?[0]
        .id
        .clone()
        .ok_or("multi-path annotation object has no ID")?;
    document.set_annotation_path_data(0, &multi_id, &first_path_id, "M 0 10 L 10 0")?;
    let png = document.page_png(0, 25.4)?;
    let image_id = document.add_multimedia_resource(0, "Image", "PNG", &png)?;
    let graphics = [
        AnnotationGraphic::Path(AnnotationPath {
            commands: "M 0 0 L 8 8".into(),
            boundary: [0.0, 0.0, 8.0, 8.0],
            style: PathStyle::default(),
        }),
        AnnotationGraphic::Text {
            text: "Mixed".into(),
            boundary: [8.0, 0.0, 15.0, 8.0],
            size: 4.0,
            font_id: None,
            color: Some([0, 0, 255]),
            opacity: 255,
        },
        AnnotationGraphic::Image {
            resource_id: image_id,
            boundary: [23.0, 0.0, 8.0, 8.0],
            opacity: 255,
        },
    ];
    let mixed_id = document.add_mixed_annotation(0, [5.0, 35.0, 35.0, 12.0], &graphics)?;
    let text_id = document.annotation_objects(0, &mixed_id)?[1]
        .id
        .clone()
        .ok_or("mixed annotation text has no ID")?;
    document.set_annotation_text_code(0, &mixed_id, &text_id, 0, "Updated & <Text>")?;
    document.add_mixed_stamp_annotation(0, [42.0, 35.0, 35.0, 12.0], &graphics)?;
    document.add_composite_path_object(
        0,
        "M 0 0 L 8 0 L 8 8 C",
        [0.0, 0.0, 8.0, 8.0],
        PathStyle::default(),
    )?;
    let resource_id = document.page_composite_objects(0)?[0].resource_id.clone();
    let composite = AnnotationGraphic::Composite {
        resource_id,
        boundary: [0.0, 0.0, 8.0, 8.0],
        style: CompositeStyle::default(),
    };
    let composite_id = document.add_mixed_stamp_annotation(
        0,
        [5.0, 5.0, 20.0, 12.0],
        std::slice::from_ref(&composite),
    )?;
    document.append_annotation_objects(0, &composite_id, &[composite])?;
    document.save(output)?;
    Ok(())
}
