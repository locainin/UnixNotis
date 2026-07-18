//! Content-based routing across the bounded raster and SVG decoders

use std::path::Path;

use super::file::read_icon_file;
use super::model::{IconResult, RasterImage};
use super::raster::decode_raster_bytes;
use super::svg::{decode_svg_bytes, is_gzip_payload};

pub(super) const MAX_ICON_DIMENSION: u32 = 2_048;
pub(super) const MAX_ICON_PIXELS: u64 = 4_194_304;

pub(super) fn decode_icon_file(path: &Path, size: i32, scale: i32) -> IconResult {
    // One descriptor-backed read captures the complete source before format routing
    let bytes = match read_icon_file(path) {
        Ok(bytes) => bytes,
        Err(error) => return IconResult::Failed(error),
    };
    match decode_icon_bytes(path, &bytes, decode_target(size, scale)) {
        Ok(image) => IconResult::Raster(image),
        Err(error) => IconResult::Failed(error),
    }
}

pub(super) fn decode_icon_bytes(
    path: &Path,
    bytes: &[u8],
    target: u32,
) -> Result<RasterImage, String> {
    // Content signatures take precedence when compression changes the outer filename
    if is_gzip_payload(bytes) {
        // Gzip is accepted only as bounded SVGZ data
        return decode_svg_bytes(bytes, target);
    }

    if path_suggests_svg(path) {
        // The suffix selects an efficient first attempt, never an unrestricted decoder
        return decode_svg_bytes(bytes, target).or_else(|svg_error| {
            decode_raster_bytes(bytes, target).map_err(|raster_error| {
                format!("SVG decode failed ({svg_error}); raster decode failed ({raster_error})")
            })
        });
    }

    decode_raster_bytes(bytes, target).or_else(|raster_error| {
        // Extension-free SVG paths remain supported through the same bounded renderer
        decode_svg_bytes(bytes, target).map_err(|svg_error| {
            format!("raster decode failed ({raster_error}); SVG decode failed ({svg_error})")
        })
    })
}

pub(super) fn decode_target(size: i32, scale: i32) -> u32 {
    // Invalid widget values become one pixel while large requests stay within decode limits
    let logical_size = i64::from(size.max(1));
    let output_scale = i64::from(scale.max(1));
    u32::try_from(
        logical_size
            .saturating_mul(output_scale)
            .clamp(1, i64::from(MAX_ICON_DIMENSION)),
    )
    .unwrap_or(MAX_ICON_DIMENSION)
}

pub(super) fn path_suggests_svg(path: &Path) -> bool {
    // The suffix changes attempt order only and never bypasses either decoder limit
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}
