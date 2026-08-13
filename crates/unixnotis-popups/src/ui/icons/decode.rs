//! File-backed popup icon decoding
//!
//! Keeps image decoding and size limits away from GTK widget code

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use image::imageops::FilterType;
use image::{ImageReader, Limits};
use rustix::fs::{open, Mode, OFlags};

#[derive(Clone)]
pub struct RasterIcon {
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

pub fn decode_icon_file(path: &Path, target_size: i32) -> Result<RasterIcon, String> {
    // Single descriptor-backed read captures the complete source before decode.
    // O_NOFOLLOW rejects last-component symlinks; O_NONBLOCK avoids blocking on
    // FIFOs or device files. This closes the TOCTOU window where a regular file
    // could be swapped for a FIFO between metadata and decode calls.
    let bytes = read_icon_file_bounded(path)?;

    // Probe format from content, not extension, so disguised files are caught
    let _format =
        image::guess_format(&bytes).map_err(|err| format!("icon format probe failed: {err}"))?;

    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_ICON_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_ICON_SOURCE_DIMENSION);
    limits.max_alloc = Some(MAX_ICON_DECODE_ALLOC_BYTES);

    let mut reader = ImageReader::new(io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    reader.limits(limits);
    let mut image = reader.decode().map_err(|err| err.to_string())?;

    let width = image.width();
    let height = image.height();
    if width > MAX_ICON_SOURCE_DIMENSION || height > MAX_ICON_SOURCE_DIMENSION {
        // Header checks reject very large rasters before a full pixel decode happens
        return Err(format!(
            "icon dimensions exceed popup decode limit ({width}x{height})"
        ));
    }

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

fn read_icon_file_bounded(path: &Path) -> Result<Vec<u8>, String> {
    // Open with NOFOLLOW to reject last-component symlinks and NONBLOCK to
    // avoid hanging on FIFOs or device files
    let descriptor = open(
        path,
        OFlags::CLOEXEC
            .union(OFlags::NOFOLLOW)
            .union(OFlags::NONBLOCK),
        Mode::empty(),
    )
    .map_err(|err| err.to_string())?;
    let file = File::from(descriptor);

    // Metadata and content come from the same descriptor even if the path changes later
    let metadata = file.metadata().map_err(|err| err.to_string())?;
    if !metadata.is_file() {
        return Err("icon path is not a regular file".to_string());
    }
    if metadata.len() > MAX_ICON_BYTES {
        return Err(format!("icon file too large ({} bytes)", metadata.len()));
    }

    let capacity = usize::try_from(metadata.len()).unwrap_or(0);
    let max_capacity = usize::try_from(MAX_ICON_BYTES).unwrap_or(usize::MAX);
    let mut bytes = Vec::with_capacity(capacity.min(max_capacity));

    // One extra byte detects a regular file that grew after the metadata snapshot
    file.take(MAX_ICON_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|err| err.to_string())?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > MAX_ICON_BYTES {
        return Err(format!("icon file too large ({observed} bytes)"));
    }

    Ok(bytes)
}

#[cfg(test)]
#[path = "tests/decode.rs"]
mod tests;
