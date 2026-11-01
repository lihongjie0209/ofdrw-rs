use crate::{tiny_skia, usvg};

fn raster(body: &str, size: f32) -> tiny_skia::Pixmap {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}">{body}</svg>"#
    );
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap();
    let mut pixmap = tiny_skia::Pixmap::new(size.ceil() as u32, size.ceil() as u32).unwrap();
    pixmap.fill(tiny_skia::Color::WHITE);
    crate::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap
}

#[test]
fn nested_layers_preserve_large_off_page_geometry() {
    let scene = |rect: &str| {
        format!(
            r##"<defs><clipPath id="page"><rect width="40" height="40"/></clipPath></defs><g clip-path="url(#page)"><g clip-path="url(#page)"><g opacity="0.5">{rect}</g></g></g>"##
        )
    };
    let large = raster(
        &scene(r#"<rect x="-2000" y="-2000" width="4000" height="4000" fill="red"/>"#),
        40.0,
    );
    let bounded = raster(&scene(r#"<rect width="40" height="40" fill="red"/>"#), 40.0);
    assert_eq!(large.data(), bounded.data());
    assert!(large.pixel(20, 20).unwrap().green() < 200);
}

#[test]
fn fractional_viewport_clip_preserves_edge_coverage() {
    let image = raster(
        r##"<defs><clipPath id="page"><rect width="10.7" height="10.7"/></clipPath></defs><g clip-path="url(#page)"><rect width="20" height="20" fill="red"/></g>"##,
        10.7,
    );
    let edge = image.pixel(10, 5).unwrap();
    assert_eq!(edge.red(), 255);
    assert!(edge.green() > 0 && edge.green() < 255);
    assert_eq!(edge.green(), edge.blue());
    assert_eq!(edge.alpha(), 255);
}

#[test]
fn masks_follow_the_offscreen_layer_origin() {
    let scene = |rect: &str| {
        format!(
            r##"<defs><clipPath id="page"><rect width="40" height="40"/></clipPath><mask id="mask" maskUnits="userSpaceOnUse" x="0" y="0" width="40" height="40"><g opacity="0.5"><rect width="40" height="40" fill="white"/></g></mask></defs><g clip-path="url(#page)"><g mask="url(#mask)">{rect}</g></g>"##
        )
    };
    assert_eq!(
        raster(
            &scene(r#"<rect x="-2000" y="-2000" width="4000" height="4000" fill="red"/>"#),
            40.0
        )
        .data(),
        raster(&scene(r#"<rect width="40" height="40" fill="red"/>"#), 40.0).data()
    );
}

#[test]
fn pattern_tiles_use_their_own_surface_budget() {
    let patterned = raster(
        r##"<defs><pattern id="p" patternUnits="userSpaceOnUse" width="150" height="60" patternTransform="translate(-80 0)"><g opacity="0.5"><rect x="130" width="20" height="60" fill="red"/></g></pattern></defs><g opacity="0.5"><rect x="50" width="10" height="60" fill="url(#p)"/></g>"##,
        60.0,
    );
    let solid = raster(
        r#"<rect x="50" width="10" height="60" fill="red" opacity="0.25"/>"#,
        60.0,
    );
    assert_eq!(patterned.data(), solid.data());
}
