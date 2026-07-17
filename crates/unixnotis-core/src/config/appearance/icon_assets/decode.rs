//! Format dispatch and shared decoded-dimension enforcement

use std::path::Path;

use super::path::{normalize_icon_asset_relative_path, validate_icon_asset_extension};
use super::raster::decode_raster_icon;
use super::svg::decode_svg_icon;
use super::{AssetPolicy, IconAssetError, ResolvedIconAsset};

pub(super) fn decode_icon_asset(
    path: &Path,
    bytes: &[u8],
    policy: AssetPolicy,
    render_size: Option<u32>,
) -> Result<ResolvedIconAsset, IconAssetError> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        return decode_svg_icon(path, bytes, policy, render_size);
    }
    decode_raster_icon(path, bytes, policy, render_size)
}

pub(super) fn validate_dimensions(
    path: &Path,
    width: u32,
    height: u32,
    policy: AssetPolicy,
) -> Result<(), IconAssetError> {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if width == 0
        || height == 0
        || width > policy.max_width
        || height > policy.max_height
        || pixels > policy.max_pixels
    {
        return Err(IconAssetError::Dimensions {
            path: path.to_path_buf(),
            width,
            height,
            max_width: policy.max_width,
            max_height: policy.max_height,
            max_pixels: policy.max_pixels,
        });
    }
    Ok(())
}

pub(super) fn decode_error(path: &Path, error: impl std::fmt::Display) -> IconAssetError {
    IconAssetError::Decode {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

/// Validate and decode icon bytes using the default asset policy
///
/// # Errors
///
/// Returns an error when the reference is invalid, the payload exceeds the size limit, or the
/// image format cannot be decoded safely
pub fn validate_icon_asset_contents(asset: &str, bytes: &[u8]) -> Result<(), IconAssetError> {
    let relative = normalize_icon_asset_relative_path(asset)?;
    let policy = AssetPolicy::default();
    validate_icon_asset_extension(&relative, policy)?;
    if bytes.len() as u64 > policy.max_bytes {
        return Err(IconAssetError::TooLarge {
            path: relative,
            size: bytes.len() as u64,
            max: policy.max_bytes,
        });
    }
    decode_icon_asset(&relative, bytes, policy, None).map(|_| ())
}

#[cfg(test)]
#[path = "tests/decode.rs"]
mod tests;
