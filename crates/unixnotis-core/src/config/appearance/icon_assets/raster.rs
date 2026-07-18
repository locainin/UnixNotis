//! Bounded raster signature checking, decoding, and resizing

use std::io::Cursor;
use std::path::Path;

use image::imageops::FilterType;
use image::{ImageFormat, ImageReader, Limits};

use super::decode::{decode_error, validate_dimensions};
use super::{AssetPolicy, IconAssetError, ResolvedIconAsset};

pub(super) fn decode_raster_icon(
    path: &Path,
    bytes: &[u8],
    policy: AssetPolicy,
    render_size: Option<u32>,
) -> Result<ResolvedIconAsset, IconAssetError> {
    // Signature guessing happens before decode so an extension cannot select the wrong codec
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| decode_error(path, error))?;
    let actual_format = reader
        .format()
        .ok_or_else(|| IconAssetError::InvalidFormat(path.to_path_buf()))?;
    let expected_format = ImageFormat::from_extension(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default(),
    )
    .ok_or_else(|| IconAssetError::InvalidFormat(path.to_path_buf()))?;
    if actual_format != expected_format {
        // Mismatched content is rejected instead of trusting either attacker-controlled signal
        return Err(IconAssetError::FormatMismatch {
            path: path.to_path_buf(),
            expected: format!("{expected_format:?}"),
            actual: format!("{actual_format:?}"),
        });
    }

    // Decoder-level limits stop allocation before the post-decode geometry check
    let mut limits = Limits::default();
    limits.max_image_width = Some(policy.max_width);
    limits.max_image_height = Some(policy.max_height);
    // Four output bytes per pixel plus a small decoder working allowance
    limits.max_alloc = Some(policy.max_pixels.saturating_mul(8));
    reader.limits(limits);
    let image = reader.decode().map_err(|error| decode_error(path, error))?;
    validate_dimensions(path, image.width(), image.height(), policy)?;
    let image = if let Some(render_size) = render_size {
        if render_size == 0 {
            return Err(IconAssetError::InvalidRenderSize);
        }
        // Preserve aspect ratio while fitting the configured square icon slot
        image.resize(render_size, render_size, FilterType::Lanczos3)
    } else {
        image
    };
    let rgba = image.into_rgba8();
    // Raster output uses straight alpha while resvg reports premultiplied pixels
    Ok(ResolvedIconAsset {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
        premultiplied_alpha: false,
    })
}

#[cfg(test)]
#[path = "tests/raster.rs"]
mod tests;
