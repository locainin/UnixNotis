//! Preflighted raster decoding and thumbnail resizing

use std::io::Cursor;

use fast_image_resize as fir;
use image::{ImageReader, Limits};

use super::model::RasterImage;
use super::pipeline::{MAX_ICON_DIMENSION, MAX_ICON_PIXELS};

// Four output bytes per pixel plus bounded decoder working space
pub(super) const MAX_ICON_DECODE_ALLOC_BYTES: u64 = MAX_ICON_PIXELS * 8;

pub(super) fn decode_raster_bytes(bytes: &[u8], target: u32) -> Result<RasterImage, String> {
    // Header-only probing rejects excessive geometry before full pixel allocation
    let probe = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let (width, height) = probe.into_dimensions().map_err(|error| error.to_string())?;
    validate_source_dimensions(width, height)?;

    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_ICON_DIMENSION);
    limits.max_image_height = Some(MAX_ICON_DIMENSION);
    limits.max_alloc = Some(MAX_ICON_DECODE_ALLOC_BYTES);
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    // Decoder-level limits apply before a compressed source can allocate its full output
    reader.limits(limits);
    let rgba = reader
        .decode()
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    // Exact target images avoid a second allocation and resample pass
    if width == target && height == target {
        return raster_image(rgba.into_raw(), width, height);
    }

    let source =
        fir::images::Image::from_vec_u8(width, height, rgba.into_raw(), fir::PixelType::U8x4)
            .map_err(|error| error.to_string())?;
    let mut destination = fir::images::Image::new(target, target, fir::PixelType::U8x4);
    let options = fir::ResizeOptions::new()
        .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::CatmullRom));
    let mut resizer = fir::Resizer::new();
    // File-backed icons retain the established square output contract
    resizer
        .resize(&source, &mut destination, Some(&options))
        .map_err(|error| error.to_string())?;
    raster_image(destination.into_vec(), target, target)
}

pub(super) fn validate_source_dimensions(width: u32, height: u32) -> Result<(), String> {
    // Multiplication stays widened so malformed dimensions cannot wrap the pixel count
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if width == 0
        || height == 0
        || width > MAX_ICON_DIMENSION
        || height > MAX_ICON_DIMENSION
        || pixels > MAX_ICON_PIXELS
    {
        return Err(format!(
            "icon dimensions exceed center decode limit ({width}x{height})"
        ));
    }
    Ok(())
}

fn raster_image(bytes: Vec<u8>, width: u32, height: u32) -> Result<RasterImage, String> {
    let width = i32::try_from(width).map_err(|error| error.to_string())?;
    let height = i32::try_from(height).map_err(|error| error.to_string())?;
    let stride = width
        .checked_mul(4)
        .ok_or_else(|| "icon row stride exceeds supported size".to_string())?;
    Ok(RasterImage {
        bytes,
        width,
        height,
        stride,
        premultiplied_alpha: false,
    })
}
