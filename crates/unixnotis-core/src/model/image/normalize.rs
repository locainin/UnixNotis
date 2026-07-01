//! Image-data validation and layout normalization

use super::{ImageData, NotificationImage, MAX_IMAGE_BYTES, MAX_IMAGE_DIMENSION};

impl NotificationImage {
    pub(super) fn is_image_data_usable(data: &ImageData) -> bool {
        // Hard dimension caps keep texture creation and D-Bus payloads predictable
        if data.width > MAX_IMAGE_DIMENSION || data.height > MAX_IMAGE_DIMENSION {
            return false;
        }
        if data.bits_per_sample != 8 || data.channels != 4 {
            return false;
        }
        // Reject invalid rowstride/data lengths early to protect downstream consumers
        Self::normalized_rowstride(
            data.width,
            data.height,
            data.rowstride,
            data.bits_per_sample,
            data.channels,
            data.data.len(),
        )
        .is_some()
    }

    pub(super) fn normalize_image_data(image: ImageData) -> Option<ImageData> {
        if image.bits_per_sample != 8 {
            return None;
        }
        // Normalize rowstride to a safe, non-zero value and reject invalid layouts
        let rowstride = Self::normalized_rowstride(
            image.width,
            image.height,
            image.rowstride,
            image.bits_per_sample,
            image.channels,
            image.data.len(),
        )?;
        let rowstride = i32::try_from(rowstride).ok()?;
        let image = ImageData { rowstride, ..image };
        match image.channels {
            4 => Some(image),
            3 => Self::expand_rgb_to_rgba(&image),
            _ => None,
        }
    }

    pub(super) fn normalized_rowstride(
        width: i32,
        height: i32,
        rowstride: i32,
        bits_per_sample: i32,
        channels: i32,
        data_len: usize,
    ) -> Option<usize> {
        if data_len == 0 || data_len > MAX_IMAGE_BYTES {
            return None;
        }
        // Negative rowstride is invalid for memory buffers
        if rowstride < 0 {
            return None;
        }
        let width = usize::try_from(width).ok()?;
        let height = usize::try_from(height).ok()?;
        if width == 0 || height == 0 {
            return None;
        }
        let bytes_per_pixel = Self::bytes_per_pixel(bits_per_sample, channels)?;
        let min_rowstride = width.checked_mul(bytes_per_pixel)?;
        let stride = if rowstride > 0 {
            usize::try_from(rowstride).ok()?
        } else {
            min_rowstride
        };
        if stride < min_rowstride {
            return None;
        }
        let required = stride.checked_mul(height)?;
        if data_len < required {
            return None;
        }
        Some(stride)
    }

    pub(super) fn bytes_per_pixel(bits_per_sample: i32, channels: i32) -> Option<usize> {
        // Require a whole number of bytes per pixel to avoid fractional layouts
        if bits_per_sample <= 0 || channels <= 0 {
            return None;
        }
        let bits_per_pixel = bits_per_sample.checked_mul(channels)?;
        if bits_per_pixel % 8 != 0 {
            return None;
        }
        usize::try_from(bits_per_pixel / 8).ok()
    }
}
