use std::path::Path;

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};

use super::decode_raster_icon;
use crate::config::{AssetPolicy, IconAssetError};

fn png_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&[0, 0, 0, 0], 1, 1, ExtendedColorType::Rgba8)
        .expect("encode PNG");
    bytes
}

#[test]
fn raster_decoder_rejects_extension_signature_mismatch() {
    let error = decode_raster_icon(
        Path::new("icon.jpg"),
        &png_bytes(),
        AssetPolicy::default(),
        None,
    )
    .expect_err("PNG bytes must not pass as JPEG");

    assert!(matches!(error, IconAssetError::FormatMismatch { .. }));
}

#[test]
fn raster_decoder_rejects_zero_render_size() {
    let error = decode_raster_icon(
        Path::new("icon.png"),
        &png_bytes(),
        AssetPolicy::default(),
        Some(0),
    )
    .expect_err("zero render size must fail");

    assert!(matches!(error, IconAssetError::InvalidRenderSize));
}
