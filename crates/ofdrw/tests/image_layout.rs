use ofdrw::{FlowBlock, FlowDocumentStyle, FlowTextAlign, OfdDocument, TableCellImage};
use std::io::Cursor;

fn encoded(format: image::ImageFormat) -> Vec<u8> {
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        50,
        25,
        image::Rgb([220, 30, 10]),
    ));
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, format).unwrap();
    output.into_inner()
}

#[test]
fn intrinsic_sizes_match_java_pixel_conversion_across_raster_formats() {
    for (format, name, canonical) in [
        (image::ImageFormat::Png, "png", "PNG"),
        (image::ImageFormat::Jpeg, "jpg", "JPEG"),
        (image::ImageFormat::Gif, "gif", "GIF"),
        (image::ImageFormat::Bmp, "bmp", "BMP"),
        (image::ImageFormat::Tiff, "tif", "TIFF"),
    ] {
        let data = encoded(format);
        let image = TableCellImage::from_intrinsic(data.clone(), name).unwrap();
        assert_eq!([image.width, image.height], [10., 5.], "{name}");
        assert_eq!(image.format, canonical);
        assert_eq!(image.data, data);
    }
}

#[test]
fn fitting_preserves_aspect_ratio_and_does_not_enlarge_images() {
    for (bounds, expected) in [
        ([4., 8.], [4., 2.]),
        ([8., 1.], [2., 1.]),
        ([20., 20.], [10., 5.]),
        ([10., 5.], [10., 5.]),
    ] {
        let image = TableCellImage::from_intrinsic(encoded(image::ImageFormat::Png), "png")
            .unwrap()
            .fit_within(bounds)
            .unwrap();
        assert_eq!([image.width, image.height], expected);
    }
}

#[test]
fn invalid_image_formats_bytes_bounds_and_dimensions_are_rejected() {
    for (data, format) in [
        (Vec::new(), "png"),
        (encoded(image::ImageFormat::Png), "jpeg"),
        (encoded(image::ImageFormat::Png), "unknown"),
        (encoded(image::ImageFormat::Png)[..24].to_vec(), "png"),
    ] {
        assert!(TableCellImage::from_intrinsic(data, format).is_err());
    }
    for bad in [0., -1., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let original =
            TableCellImage::from_intrinsic(encoded(image::ImageFormat::Png), "png").unwrap();
        for bounds in [[bad, 10.], [10., bad]] {
            assert!(original.clone().fit_within(bounds).is_err());
        }
        let mut invalid = original;
        invalid.width = bad;
        assert!(invalid.fit_within([10., 10.]).is_err());
    }
    let mut extreme =
        TableCellImage::from_intrinsic(encoded(image::ImageFormat::Png), "png").unwrap();
    extreme.width = f64::MAX;
    assert!(extreme.fit_within([f64::from_bits(1), 1.]).is_err());
}

#[test]
fn intrinsically_sized_image_is_drawn_by_flow_and_survives_round_trip() {
    let image = TableCellImage::from_intrinsic(encoded(image::ImageFormat::Png), "png")
        .unwrap()
        .fit_within([4., 4.])
        .unwrap();
    let mut document = OfdDocument::new();
    let result = document
        .add_flow_blocks(
            0,
            &[FlowBlock::Image {
                image,
                align: FlowTextAlign::Left,
            }],
            FlowDocumentStyle::default(),
        )
        .unwrap();
    assert_eq!(result.pages, vec![0]);
    let bytes = document.to_bytes().unwrap();
    let reopened = OfdDocument::from_bytes(&bytes).unwrap();
    assert_eq!(reopened.pages().unwrap().len(), 1);
    assert_eq!(
        reopened.page_images(0).unwrap()[0].boundary,
        Some([31.7, 25.4, 4., 2.])
    );
    assert!(!reopened.page_png(0, 30.).unwrap().is_empty());
    let svg = reopened.page_svg(0).unwrap();
    assert!(svg.contains("image"));
}

#[test]
fn jpeg2000_intrinsic_dimensions_use_the_decoded_image() {
    for (data, format) in [
        (include_bytes!("fixtures/red-32x32.jp2").as_slice(), "jp2"),
        (include_bytes!("fixtures/blue-32x32.j2k").as_slice(), "j2k"),
    ] {
        let image = TableCellImage::from_intrinsic(data.to_vec(), format).unwrap();
        assert_eq!([image.width, image.height], [6.4, 6.4]);
        assert_eq!(image.format, "JPEG2000");
        assert_eq!(image.data, data);
    }
}
