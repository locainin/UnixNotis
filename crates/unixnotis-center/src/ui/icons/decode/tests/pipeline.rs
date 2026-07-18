use std::path::Path;

use super::super::pipeline::{decode_icon_bytes, decode_target, path_suggests_svg};
use super::support::png_bytes;

#[test]
fn content_routing_decodes_raster_bytes_with_an_svg_suffix() {
    let decoded = decode_icon_bytes(Path::new("disguised.svg"), &png_bytes(1, 1), 12)
        .expect("bounded raster fallback");

    assert_eq!((decoded.width, decoded.height), (12, 12));
    assert!(!decoded.premultiplied_alpha);
}

#[test]
fn content_routing_rejects_incomplete_png_data_with_an_svg_suffix() {
    let header_only_png = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40, 0x08, 0x06, 0x00, 0x00, 0x00, 0xaa,
        0x69, 0x71, 0xde, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    decode_icon_bytes(Path::new("raster-disguised-as.svg"), &header_only_png, 12)
        .expect_err("incomplete raster data must not reach a fallback decoder");
}

#[test]
fn content_routing_decodes_extensionless_svg_with_resvg() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="4"><path d="M0 0h8v4H0z"/></svg>"#;

    let decoded = decode_icon_bytes(Path::new("icon"), svg, 16).expect("bounded SVG fallback");

    assert_eq!((decoded.width, decoded.height), (16, 8));
    assert!(decoded.premultiplied_alpha);
}

#[test]
fn decode_target_clamps_invalid_and_oversized_requests() {
    assert_eq!(decode_target(24, 2), 48);
    assert_eq!(decode_target(0, 0), 1);
    assert_eq!(decode_target(i32::MAX, i32::MAX), 2_048);
}

#[test]
fn svg_path_hint_matches_only_the_svg_extension_without_case_sensitivity() {
    assert!(path_suggests_svg(Path::new("icon.svg")));
    assert!(path_suggests_svg(Path::new("icon.SVG")));
    assert!(!path_suggests_svg(Path::new("icon.svgz")));
    assert!(!path_suggests_svg(Path::new("icon.png")));
    assert!(!path_suggests_svg(Path::new("icon")));
}
