//! File-backed popup icon decoding
//!
//! Keeps image decoding and size limits away from GTK widget code

use std::fs;
use std::path::Path;

use image::imageops::FilterType;
use image::{ImageReader, Limits};

#[derive(Clone)]
pub(crate) struct RasterIcon {
    pub(crate) bytes: Vec<u8>,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) stride: i32,
}

// Keep icon loads lightweight; popups only render small thumbnails
const MAX_ICON_BYTES: u64 = 16 * 1024 * 1024;
// Reject raster sources that are too large to decode cheaply for popup thumbnails
const MAX_ICON_SOURCE_DIMENSION: u32 = 2048;
// Cap decoder allocation so compressed inputs cannot explode into very large buffers
const MAX_ICON_DECODE_ALLOC_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) fn decode_icon_file(path: &Path, target_size: i32) -> Result<RasterIcon, String> {
    // Decode on a worker thread; keep I/O and CPU-bound work off the GTK main loop
    let metadata = fs::metadata(path).map_err(|err| err.to_string())?;
    if !metadata.is_file() {
        // Directories and special files are rejected before image parsing starts
        return Err("icon path is not a regular file".to_string());
    }
    if metadata.len() > MAX_ICON_BYTES {
        // Oversized files are rejected early to cap decode memory use
        return Err(format!("icon file too large ({} bytes)", metadata.len()));
    }

    let (width, height) = image::image_dimensions(path).map_err(|err| err.to_string())?;
    if width > MAX_ICON_SOURCE_DIMENSION || height > MAX_ICON_SOURCE_DIMENSION {
        // Header checks reject very large rasters before a full pixel decode happens
        return Err(format!(
            "icon dimensions exceed popup decode limit ({}x{})",
            width, height
        ));
    }

    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_ICON_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_ICON_SOURCE_DIMENSION);
    limits.max_alloc = Some(MAX_ICON_DECODE_ALLOC_BYTES);

    let mut reader = ImageReader::open(path).map_err(|err| err.to_string())?;
    if reader.format().is_none() {
        // Extension-free temp paths still need content sniffing before decode
        reader = reader
            .with_guessed_format()
            .map_err(|err| err.to_string())?;
    }
    reader.limits(limits);
    let mut image = reader.decode().map_err(|err| err.to_string())?;

    let target = target_size.max(1) as u32;
    // Normalize to the popup icon target so file-backed icons match themed icon sizing
    image = image.resize(target, target, FilterType::Lanczos3);

    // Popup textures always move forward as RGBA bytes
    let rgba = image.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    if width > i32::MAX as u32 || height > i32::MAX as u32 {
        // GTK texture sizes still need to fit in signed 32-bit dimensions
        return Err("icon exceeds supported dimensions".to_string());
    }
    let width = width as i32;
    let height = height as i32;
    let stride = width.saturating_mul(4);

    Ok(RasterIcon {
        bytes: rgba.into_raw(),
        width,
        height,
        stride,
    })
}

#[cfg(test)]
#[path = "tests/decode.rs"]
mod tests;
