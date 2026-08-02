use std::io::Write;
use std::os::unix::fs::PermissionsExt;

use flate2::write::GzEncoder;
use flate2::Compression;

use super::super::model::RasterImage;
use super::super::pipeline::{MAX_ICON_DIMENSION, MAX_ICON_PIXELS};
use super::super::svg::{
    checked_rgba_len, decode_svg_bytes_with_renderer, decompress_svgz_with_limit, is_gzip_payload,
    resolve_svg_renderer,
};

fn decode_svg_bytes(bytes: &[u8], target: u32) -> Result<RasterImage, String> {
    let renderer = resolve_svg_renderer()?;
    decode_svg_bytes_with_renderer(bytes, target, &renderer)
}

#[test]
fn svg_decoder_renders_bounded_pixels_and_preserves_aspect_ratio() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><path d="M0 0h20v10H0z"/></svg>"#;

    let decoded = decode_svg_bytes(svg, 16).expect("decode bounded SVG");

    assert_eq!((decoded.width, decoded.height, decoded.stride), (16, 8, 64));
    assert_eq!(decoded.bytes.len(), 16 * 8 * 4);
    assert!(decoded.premultiplied_alpha);
}

#[test]
fn svg_protocol_accepts_multiline_documents() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="4">
  <path d="M0 0h8v4H0z"/>
</svg>"#;

    let decoded = decode_svg_bytes(svg, 16).expect("multiline SVG should render");
    assert_eq!((decoded.width, decoded.height), (16, 8));
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
    let input_error = "SVG scaling inputs must be finite and bounded";
    assert_eq!(
        fitted_svg_dimensions(f32::NAN, 10.0, 16).expect_err("NaN width must fail"),
        input_error
    );
    assert_eq!(
        fitted_svg_dimensions(10.0, f32::INFINITY, 16).expect_err("infinite height must fail"),
        input_error
    );
    assert_eq!(
        fitted_svg_dimensions(0.0, 10.0, 16).expect_err("zero width must fail"),
        input_error
    );
    assert_eq!(
        fitted_svg_dimensions(10.0, 10.0, 0).expect_err("zero target must fail"),
        input_error
    );
    assert_eq!(
        fitted_svg_dimensions(10.0, 10.0, MAX_ICON_DIMENSION + 1)
            .expect_err("oversized target must fail"),
        input_error
    );
    assert_eq!(
        fitted_svg_dimensions(f32::MIN_POSITIVE, f32::MIN_POSITIVE, 16)
            .expect_err("infinite scale must fail"),
        "SVG scaling result must be finite and positive"
    );
}

#[test]
fn svg_scaling_returns_finite_bounded_geometry() {
    let (width, height, scale) =
        fitted_svg_dimensions(20.0, 10.0, 16).expect("fit finite geometry");

    assert_eq!((width, height), (16, 8));
    assert!(scale.is_finite());
    assert!(scale > 0.0);

    let (width, height, _scale) =
        fitted_svg_dimensions(1.0, 1.0, MAX_ICON_DIMENSION).expect("fit exact target limit");
    assert_eq!((width, height), (MAX_ICON_DIMENSION, MAX_ICON_DIMENSION));
}

#[test]
fn renderer_output_dimensions_are_checked_before_allocation() {
    assert!(checked_rgba_len(0, 1).is_err());
    assert!(checked_rgba_len(MAX_ICON_DIMENSION + 1, 1).is_err());
    assert!(checked_rgba_len(1, MAX_ICON_DIMENSION + 1).is_err());
    assert!(checked_rgba_len(u32::MAX, u32::MAX).is_err());
    assert_eq!(
        checked_rgba_len(MAX_ICON_DIMENSION, MAX_ICON_DIMENSION).expect("bounded output"),
        usize::try_from(MAX_ICON_PIXELS).expect("usize pixels") * 4
    );
}

#[test]
fn malformed_renderer_dimensions_are_rejected_without_large_allocation() {
    let directory = tempfile::tempdir().expect("create renderer fixture directory");
    let renderer = directory.path().join("bad-renderer");
    std::fs::write(
        &renderer,
        "#!/bin/sh\n# Consume the complete request before returning malformed dimensions\ndd bs=1 count=8 iflag=fullblock of=/dev/null 2>/dev/null || exit 1\ncat >/dev/null\nprintf '\\377\\377\\377\\377\\377\\377\\377\\377'\n",
    )
    .expect("write renderer fixture");
    std::fs::set_permissions(&renderer, std::fs::Permissions::from_mode(0o755))
        .expect("make renderer executable");

    let error = decode_svg_bytes_with_renderer(b"<svg/>", 16, &renderer)
        .expect_err("oversized child dimensions must fail");
    assert!(
        error.contains("renderer returned oversized image"),
        "unexpected error: {error}"
    );
}

#[test]
fn renderer_deadline_terminates_a_slow_child() {
    let directory = tempfile::tempdir().expect("create renderer fixture directory");
    let renderer = directory.path().join("slow-renderer");
    std::fs::write(
        &renderer,
        "#!/bin/sh\n# Consume the request so the parent can finish writing before the timeout\ncat >/dev/null\nsleep 2\n",
    )
    .expect("write renderer fixture");
    std::fs::set_permissions(&renderer, std::fs::Permissions::from_mode(0o755))
        .expect("make renderer executable");

    let error = decode_svg_bytes_with_renderer(b"<svg/>", 16, &renderer)
        .expect_err("slow renderer must be stopped");
    assert!(error.contains("timed out"));
}

#[test]
fn renderer_stderr_is_drained_while_stdout_is_decoded() {
    let directory = tempfile::tempdir().expect("create renderer fixture directory");
    let renderer = directory.path().join("chatty-renderer");
    std::fs::write(
        &renderer,
        "#!/bin/sh\nhead -c 1048576 /dev/zero >&2\nprintf '\\001\\000\\000\\000\\001\\000\\000\\000\\000\\000\\000\\377'\n",
    )
    .expect("write renderer fixture");
    std::fs::set_permissions(&renderer, std::fs::Permissions::from_mode(0o755))
        .expect("make renderer executable");

    let decoded = decode_svg_bytes_with_renderer(b"<svg/>", 16, &renderer)
        .expect("chatty renderer should not deadlock");
    assert_eq!((decoded.width, decoded.height), (1, 1));
}

#[test]
fn renderer_stderr_is_bounded_before_error_reporting() {
    let directory = tempfile::tempdir().expect("create renderer fixture directory");
    let renderer = directory.path().join("noisy-failing-renderer");
    std::fs::write(
        &renderer,
        "#!/bin/sh\nyes X | head -c 1048576 >&2\nexit 1\n",
    )
    .expect("write renderer fixture");
    std::fs::set_permissions(&renderer, std::fs::Permissions::from_mode(0o755))
        .expect("make renderer executable");

    let error = decode_svg_bytes_with_renderer(b"<svg/>", 16, &renderer)
        .expect_err("failing renderer should return an error");
    assert!(error.len() <= 17_000, "stderr exceeded diagnostic cap");
}

#[test]
fn missing_sibling_renderer_is_reported() {
    let error = decode_svg_bytes_with_renderer(
        b"<svg/>",
        16,
        std::path::Path::new("/nonexistent/unixnotis-svg-renderer"),
    )
    .expect_err("missing renderer must fail closed");
    assert!(error.contains("failed to spawn SVG renderer"));
}

fn fitted_svg_dimensions(
    source_width: f32,
    source_height: f32,
    target: u32,
) -> Result<(u32, u32, f32), String> {
    if !source_width.is_finite()
        || !source_height.is_finite()
        || source_width <= 0.0
        || source_height <= 0.0
        || target == 0
        || target > MAX_ICON_DIMENSION
    {
        return Err("SVG scaling inputs must be finite and bounded".to_string());
    }

    let target = target as f32;
    let scale = (target / source_width).min(target / source_height);
    if !scale.is_finite() || scale <= 0.0 {
        return Err("SVG scaling result must be finite and positive".to_string());
    }
    let scaled_width = (source_width * scale).round().max(1.0);
    let scaled_height = (source_height * scale).round().max(1.0);

    let width = scaled_width as u32;
    let height = scaled_height as u32;
    validate_svg_dimensions(width, height)?;
    Ok((width, height, scale))
}

fn validate_svg_dimensions(width: u32, height: u32) -> Result<(), String> {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if width == 0
        || height == 0
        || width > MAX_ICON_DIMENSION
        || height > MAX_ICON_DIMENSION
        || pixels > MAX_ICON_PIXELS
    {
        return Err(format!(
            "SVG dimensions exceed center decode limit ({width}x{height})"
        ));
    }
    Ok(())
}
