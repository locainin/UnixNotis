use std::path::Path;

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder, ImageReader};

use super::materialize_bounded_image_as_png;
use crate::config::{AssetPolicy, IconAssetError};

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let pixels = vec![128; width as usize * height as usize * 4];
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .expect("encode source PNG");
    bytes
}

#[test]
fn materializer_emits_a_decodable_png_with_the_same_dimensions() {
    let source = png_bytes(3, 2);

    let output = materialize_bounded_image_as_png(
        Path::new("assets/source.png"),
        &source,
        AssetPolicy::default(),
    )
    .expect("materialize bounded image");
    let decoded = ImageReader::new(std::io::Cursor::new(output))
        .with_guessed_format()
        .expect("guess output format")
        .decode()
        .expect("decode output PNG");

    assert_eq!(decoded.width(), 3);
    assert_eq!(decoded.height(), 2);
}

#[test]
fn materializer_rejects_source_dimensions_above_policy() {
    let source = png_bytes(3, 2);
    let policy = AssetPolicy {
        max_width: 2,
        max_height: 2,
        max_pixels: 4,
        ..AssetPolicy::default()
    };

    let error = materialize_bounded_image_as_png(Path::new("assets/source.png"), &source, policy)
        .expect_err("reject oversized source");

    assert!(matches!(
        error,
        IconAssetError::Dimensions { .. } | IconAssetError::Decode { .. }
    ));
}

#[test]
fn materializer_converts_svg_pixels_from_premultiplied_to_straight_alpha() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">
        <rect width="1" height="1" fill="#804020" fill-opacity="0.5"/>
    </svg>"##;

    let output = materialize_bounded_image_as_png(
        Path::new("assets/source.svg"),
        source,
        AssetPolicy::default(),
    )
    .expect("materialize SVG");
    let decoded = ImageReader::new(std::io::Cursor::new(output))
        .with_guessed_format()
        .expect("guess output format")
        .decode()
        .expect("decode output PNG")
        .into_rgba8();

    assert_eq!(decoded.get_pixel(0, 0).0, [128, 64, 32, 128]);
}
