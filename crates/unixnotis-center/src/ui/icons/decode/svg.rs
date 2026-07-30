//! Bounded SVG and SVGZ parsing with secondary image loading disabled

use std::borrow::Cow;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use flate2::read::GzDecoder;

use super::file::MAX_ICON_BYTES;
use super::model::RasterImage;
use super::pipeline::{MAX_ICON_DIMENSION, MAX_ICON_PIXELS};

// Hard wall-clock deadline for SVG parsing and rendering
const SVG_RENDER_DEADLINE: Duration = Duration::from_millis(500);

pub(super) const fn is_gzip_payload(bytes: &[u8]) -> bool {
    // SVGZ uses the normal gzip signature regardless of its filename suffix
    matches!(bytes, [0x1f, 0x8b, ..])
}

pub(super) fn decode_svg_bytes(bytes: &[u8], target: u32) -> Result<RasterImage, String> {
    // Compressed documents are expanded under the same source byte ceiling
    let document = if is_gzip_payload(bytes) {
        Cow::Owned(decompress_svgz_with_limit(bytes, MAX_ICON_BYTES)?)
    } else {
        Cow::Borrowed(bytes)
    };

    let secondary_image = Arc::new(AtomicBool::new(false));
    // Both resolver callbacks share one flag so attempted nested images fail the document
    let data_image = Arc::clone(&secondary_image);
    let path_image = Arc::clone(&secondary_image);
    let options = resvg::usvg::Options {
        // SVG image nodes stay disabled so parsing cannot open files or nested image decoders
        image_href_resolver: resvg::usvg::ImageHrefResolver {
            resolve_data: Box::new(move |_mime, _data, _options| {
                data_image.store(true, Ordering::Relaxed);
                None
            }),
            resolve_string: Box::new(move |_href, _options| {
                path_image.store(true, Ordering::Relaxed);
                None
            }),
        },
        ..resvg::usvg::Options::default()
    };
    let tree =
        resvg::usvg::Tree::from_data(&document, &options).map_err(|error| error.to_string())?;
    // Parsing may call a resolver even though that resolver returns no image
    if secondary_image.load(Ordering::Relaxed) {
        return Err("SVG icons must not contain secondary images".to_string());
    }

    let source_width_float = tree.size().width();
    let source_height_float = tree.size().height();
    // Fit validation rejects invalid floating-point geometry before integer conversion
    let (width, height, scale) =
        fitted_svg_dimensions(source_width_float, source_height_float, target)?;
    let source_width = source_width_float.ceil() as u32;
    let source_height = source_height_float.ceil() as u32;
    validate_svg_dimensions(source_width, source_height)?;

    // Output allocation follows the fitted dimensions rather than the source canvas
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| "could not allocate bounded SVG surface".to_string())?;

    // Enforce a wall-clock deadline on rendering to prevent CPU exhaustion (UNX-4-005)
    let render_start = Instant::now();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    if render_start.elapsed() > SVG_RENDER_DEADLINE {
        return Err("SVG render exceeded time limit".to_string());
    }

    let width = i32::try_from(width).map_err(|error| error.to_string())?;
    let height = i32::try_from(height).map_err(|error| error.to_string())?;
    let stride = width
        .checked_mul(4)
        .ok_or_else(|| "SVG row stride exceeds supported size".to_string())?;
    Ok(RasterImage {
        bytes: pixmap.take(),
        width,
        height,
        stride,
        premultiplied_alpha: true,
    })
}

pub(super) fn fitted_svg_dimensions(
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
    // A finite minimum ratio keeps both products no larger than the target
    let scaled_width = (source_width * scale).round().max(1.0);
    let scaled_height = (source_height * scale).round().max(1.0);

    let width = scaled_width as u32;
    let height = scaled_height as u32;
    validate_svg_dimensions(width, height)?;
    Ok((width, height, scale))
}

pub(super) fn decompress_svgz_with_limit(bytes: &[u8], max_bytes: u64) -> Result<Vec<u8>, String> {
    let mut decoder = GzDecoder::new(bytes);
    let mut document = Vec::new();
    // One extra byte distinguishes an exact-limit document from an oversized stream
    decoder
        .by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut document)
        .map_err(|error| error.to_string())?;
    if u64::try_from(document.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err("decompressed SVG exceeds icon byte limit".to_string());
    }
    Ok(document)
}

pub(super) fn validate_svg_dimensions(width: u32, height: u32) -> Result<(), String> {
    // Source geometry is checked separately from the smaller fitted output surface
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
