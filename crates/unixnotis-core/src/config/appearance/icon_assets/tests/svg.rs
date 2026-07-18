use std::path::Path;

use super::decode_svg_icon;
use crate::config::{AssetPolicy, IconAssetError};

#[test]
fn svg_decoder_rejects_namespaced_external_image_nodes() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg" width="16" height="16"><s:image href="/tmp/external.png" width="16" height="16"/></svg>"#;

    let error = decode_svg_icon(
        Path::new("assets/icon.svg"),
        svg,
        AssetPolicy::default(),
        None,
    )
    .expect_err("secondary image resolver must stay disabled");

    assert!(matches!(error, IconAssetError::EmbeddedSvgImage(_)));
}

#[test]
fn svg_decoder_preserves_aspect_ratio_at_requested_size() {
    let svg =
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><path d="M0 0h20v10H0z"/></svg>"#;

    let icon = decode_svg_icon(
        Path::new("assets/icon.svg"),
        svg,
        AssetPolicy::default(),
        Some(10),
    )
    .expect("decode bounded SVG");

    assert_eq!((icon.width, icon.height), (10, 5));
    assert!(icon.premultiplied_alpha);
}

#[test]
fn svg_decoder_preserves_portrait_aspect_ratio_at_requested_size() {
    let svg =
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20"><path d="M0 0h10v20H0z"/></svg>"#;

    let icon = decode_svg_icon(
        Path::new("assets/icon.svg"),
        svg,
        AssetPolicy::default(),
        Some(10),
    )
    .expect("decode bounded portrait SVG");

    assert_eq!((icon.width, icon.height), (5, 10));
}
