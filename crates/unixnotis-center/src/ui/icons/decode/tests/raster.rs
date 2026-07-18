use super::super::pipeline::{MAX_ICON_DIMENSION, MAX_ICON_PIXELS};
use super::super::raster::{
    decode_raster_bytes, validate_source_dimensions, MAX_ICON_DECODE_ALLOC_BYTES,
};
use super::support::png_bytes;

#[test]
fn raster_decoder_preflights_then_resizes_valid_pixels() {
    let decoded = decode_raster_bytes(&png_bytes(2, 1), 8).expect("decode bounded PNG");

    assert_eq!((decoded.width, decoded.height, decoded.stride), (8, 8, 32));
    assert_eq!(decoded.bytes.len(), 8 * 8 * 4);
    assert!(!decoded.premultiplied_alpha);
}

#[test]
fn raster_decoder_keeps_exact_square_pixels_without_resizing() {
    let decoded = decode_raster_bytes(&png_bytes(1, 1), 1).expect("decode exact PNG");

    assert_eq!((decoded.width, decoded.height, decoded.stride), (1, 1, 4));
    assert_eq!(decoded.bytes, vec![0x7f; 4]);
}

#[test]
fn raster_decoder_resizes_when_only_one_dimension_matches_the_target() {
    let wide = decode_raster_bytes(&png_bytes(8, 4), 8).expect("decode wide PNG");
    let tall = decode_raster_bytes(&png_bytes(4, 8), 8).expect("decode tall PNG");

    assert_eq!((wide.width, wide.height), (8, 8));
    assert_eq!((tall.width, tall.height), (8, 8));
}

#[test]
fn raster_source_limits_cover_zero_exact_and_oversized_boundaries() {
    assert_eq!(MAX_ICON_DECODE_ALLOC_BYTES, MAX_ICON_PIXELS * 8);
    assert!(validate_source_dimensions(0, 1).is_err());
    assert!(validate_source_dimensions(1, 0).is_err());
    assert!(validate_source_dimensions(MAX_ICON_DIMENSION, MAX_ICON_DIMENSION).is_ok());
    assert!(validate_source_dimensions(MAX_ICON_DIMENSION + 1, 1).is_err());
    assert!(validate_source_dimensions(1, MAX_ICON_DIMENSION + 1).is_err());
}

#[test]
fn raster_decoder_rejects_huge_dimensions_from_the_header() {
    let header = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x01, 0x86, 0xa0, 0x00, 0x01, 0x86, 0xa0, 0x08, 0x06, 0x00, 0x00, 0x00, 0xa8,
        0x52, 0x0b, 0xc8, 0x00, 0x00, 0x00, 0x00, 0x49, 0x44, 0x41, 0x54, 0x35, 0xaf, 0x06, 0x1e,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    let error = decode_raster_bytes(&header, 32).expect_err("huge dimensions must fail early");

    assert!(
        error.contains("dimensions exceed center decode limit"),
        "{error}"
    );
    assert!(error.contains("100000x100000"));
}

#[test]
fn raster_decoder_rejects_zero_dimensions_from_the_header() {
    let header = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x08, 0x06, 0x00, 0x00, 0x00, 0x34,
        0xd3, 0x76, 0x7e, 0x00, 0x00, 0x00, 0x00, 0x49, 0x44, 0x41, 0x54, 0x35, 0xaf, 0x06, 0x1e,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    let error = decode_raster_bytes(&header, 32).expect_err("zero width must fail early");

    assert!(error.contains("Invalid image dimensions"), "{error}");
}
