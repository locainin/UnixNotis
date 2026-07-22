use std::io::Write;

use flate2::write::GzEncoder;
use flate2::Compression;

use super::super::pipeline::MAX_ICON_DIMENSION;
use super::super::svg::{
    decode_svg_bytes, decompress_svgz_with_limit, fitted_svg_dimensions, is_gzip_payload,
    validate_svg_dimensions,
};

#[test]
fn svg_decoder_renders_bounded_pixels_and_preserves_aspect_ratio() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><path d="M0 0h20v10H0z"/></svg>"#;

    let decoded = decode_svg_bytes(svg, 16).expect("decode bounded SVG");

    assert_eq!((decoded.width, decoded.height, decoded.stride), (16, 8, 64));
    assert_eq!(decoded.bytes.len(), 16 * 8 * 4);
    assert!(decoded.premultiplied_alpha);
}

#[test]
fn svg_decoder_uses_height_as_the_constraint_for_tall_images() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20"><path d="M0 0h10v20H0z"/></svg>"#;

    let decoded = decode_svg_bytes(svg, 16).expect("decode tall SVG");

    assert_eq!((decoded.width, decoded.height, decoded.stride), (8, 16, 32));
}

#[test]
fn svg_decoder_rejects_secondary_image_nodes() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><image href="file:///tmp/icon.png" width="16" height="16"/></svg>"#;

    let error = decode_svg_bytes(svg, 16).expect_err("secondary image must fail");

    assert!(error.contains("secondary images"));
}

#[test]
fn svgz_decompression_stops_at_the_configured_output_limit() {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(&vec![b' '; 1_025])
        .expect("compress SVGZ body");
    let compressed = encoder.finish().expect("finish SVGZ body");

    let error = decompress_svgz_with_limit(&compressed, 1_024)
        .expect_err("expanded SVGZ body must stay bounded");

    assert!(error.contains("decompressed SVG exceeds icon byte limit"));
}

#[test]
fn svgz_detection_requires_both_gzip_magic_bytes() {
    assert!(is_gzip_payload(&[0x1f, 0x8b, 0x00]));
    assert!(!is_gzip_payload(&[0x1f]));
    assert!(!is_gzip_payload(&[0x1f, 0x00]));
    assert!(!is_gzip_payload(b"<svg/>"));
}

#[test]
fn svgz_decoder_accepts_a_complete_compressed_document() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="4"><path d="M0 0h8v4H0z"/></svg>"#;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(svg).expect("compress SVG document");
    let compressed = encoder.finish().expect("finish SVG document");

    let decoded = decode_svg_bytes(&compressed, 16).expect("decode SVGZ document");

    assert_eq!((decoded.width, decoded.height), (16, 8));
}

#[test]
fn svgz_decompression_accepts_the_exact_output_limit() {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(&vec![b'x'; 1_024])
        .expect("compress exact SVGZ body");
    let compressed = encoder.finish().expect("finish exact SVGZ body");

    assert_eq!(
        decompress_svgz_with_limit(&compressed, 1_024).expect("exact output limit"),
        vec![b'x'; 1_024]
    );
}

#[test]
fn svg_source_limits_cover_zero_exact_and_oversized_boundaries() {
    assert!(validate_svg_dimensions(0, 1).is_err());
    assert!(validate_svg_dimensions(1, 0).is_err());
    assert!(validate_svg_dimensions(MAX_ICON_DIMENSION, MAX_ICON_DIMENSION).is_ok());
    assert!(validate_svg_dimensions(MAX_ICON_DIMENSION + 1, 1).is_err());
    assert!(validate_svg_dimensions(1, MAX_ICON_DIMENSION + 1).is_err());
}

#[test]
fn svg_scaling_rejects_non_finite_zero_and_oversized_inputs() {
    assert!(fitted_svg_dimensions(f32::NAN, 10.0, 16).is_err());
    assert!(fitted_svg_dimensions(10.0, f32::INFINITY, 16).is_err());
    assert!(fitted_svg_dimensions(0.0, 10.0, 16).is_err());
    assert!(fitted_svg_dimensions(10.0, 10.0, 0).is_err());
    assert!(fitted_svg_dimensions(10.0, 10.0, MAX_ICON_DIMENSION + 1).is_err());
}

#[test]
fn svg_scaling_returns_finite_bounded_geometry() {
    let (width, height, scale) =
        fitted_svg_dimensions(20.0, 10.0, 16).expect("fit finite geometry");

    assert_eq!((width, height), (16, 8));
    assert!(scale.is_finite());
    assert!(scale > 0.0);
}
