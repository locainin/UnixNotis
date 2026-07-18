//! Safe PNG materialization for bytes decoded under an asset policy

use std::path::Path;

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};

use super::{decode_image_asset_contents, AssetPolicy, IconAssetError};

/// Decode an allowed image and serialize only its bounded RGBA pixels as PNG
///
/// # Errors
///
/// Returns an error when source validation, pixel conversion, or PNG encoding fails
pub fn materialize_bounded_image_as_png(
    path: &Path,
    bytes: &[u8],
    policy: AssetPolicy,
) -> Result<Vec<u8>, IconAssetError> {
    let decoded = decode_image_asset_contents(path, bytes, policy)?;
    let expected_len = u64::from(decoded.width)
        .checked_mul(u64::from(decoded.height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|length| usize::try_from(length).ok())
        .ok_or_else(|| invalid_pixel_buffer(path))?;
    if decoded.rgba.len() != expected_len {
        return Err(invalid_pixel_buffer(path));
    }

    let mut rgba = decoded.rgba;
    if decoded.premultiplied_alpha {
        // PNG stores straight alpha, so resvg output is converted before serialization
        for pixel in rgba.chunks_exact_mut(4) {
            let alpha = pixel[3];
            unpremultiply_channel(&mut pixel[0], alpha);
            unpremultiply_channel(&mut pixel[1], alpha);
            unpremultiply_channel(&mut pixel[2], alpha);
        }
    }

    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            &rgba,
            decoded.width,
            decoded.height,
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| IconAssetError::Decode {
            path: path.to_path_buf(),
            message: format!("could not encode validated PNG: {error}"),
        })?;
    Ok(png)
}

fn unpremultiply_channel(channel: &mut u8, alpha: u8) {
    if alpha == 0 {
        *channel = 0;
        return;
    }
    let straight = (u32::from(*channel) * 255 + u32::from(alpha) / 2) / u32::from(alpha);
    *channel = u8::try_from(straight.min(255)).unwrap_or(255);
}

fn invalid_pixel_buffer(path: &Path) -> IconAssetError {
    IconAssetError::Decode {
        path: path.to_path_buf(),
        message: "decoded pixel buffer has an invalid length".to_string(),
    }
}

#[cfg(test)]
#[path = "tests/materialize.rs"]
mod tests;
