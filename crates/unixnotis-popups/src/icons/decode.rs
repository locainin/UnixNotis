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
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{ImageBuffer, ImageFormat, Rgba};

    use super::{decode_icon_file, MAX_ICON_SOURCE_DIMENSION};

    fn test_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "unixnotis-popups-{name}-{}-{nonce}.png",
            std::process::id()
        ))
    }

    #[test]
    fn decode_icon_file_rejects_large_dimensions_before_full_decode() {
        let path = test_path("oversized");
        let image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(
            MAX_ICON_SOURCE_DIMENSION + 1,
            8,
            Rgba([0, 0, 0, 255]),
        );
        image
            .save_with_format(&path, ImageFormat::Png)
            .expect("save image");

        let err = match decode_icon_file(&path, 20) {
            Ok(_) => panic!("oversized image should fail"),
            Err(err) => err,
        };
        assert!(err.contains("decode limit"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn decode_icon_file_scales_to_requested_size() {
        let path = test_path("scale");
        let image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(64, 32, Rgba([1, 2, 3, 255]));
        image
            .save_with_format(&path, ImageFormat::Png)
            .expect("save image");

        let decoded = decode_icon_file(&path, 20).expect("decode icon");

        assert!(decoded.width <= 20);
        assert!(decoded.height <= 20);
        assert_eq!(decoded.stride, decoded.width.saturating_mul(4));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn decode_icon_file_rejects_non_files() {
        let path = test_path("not-file");
        fs::create_dir(&path).expect("create temp dir");

        let err = match decode_icon_file(&path, 20) {
            Ok(_) => panic!("directory should fail"),
            Err(err) => err,
        };
        assert_eq!(err, "icon path is not a regular file");
        let _ = fs::remove_dir(&path);
    }
}
